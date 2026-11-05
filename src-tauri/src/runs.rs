//! Runs: which config a game was launched with, and how that went — inferred
//! from its logs, because protongen never launches anything (Nexus, Steam or
//! Heroic do).
//!
//! The loop: when the user copies the command or applies it to Nexus/Heroic,
//! the frontend *arms* that config for the game. The next time a log for the
//! game is written after that moment, the armed config becomes a [`Run`],
//! with a verdict read off the log. Further writes to the log update the same
//! run until something is armed again — so one run is one tested config, which
//! is the unit a "what changed since it last worked?" question needs.
//!
//! Kept in its own file (`$XDG_CONFIG_HOME/protongen/runs.toml`), owned by the
//! backend alone: `save_store` overwrites `state.toml` wholesale with whatever
//! the frontend holds (#43), so anything the backend appended there would be
//! lost on the next save.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::store::Config;

/// Runs kept per game, newest first.
pub const MAX_RUNS: usize = 12;

/// Lines that mean the game went down, not merely complained. Case-folded
/// substring match, like `logs::is_error_line`.
const CRASH_MARKERS: [&str; 9] = [
    "unhandled page fault",
    "unhandled exception",
    "segmentation fault",
    "sigsegv",
    "vk_error_device_lost",
    "device lost",
    "core dumped",
    "backtrace:",
    "fatal error",
];

/// How a run went, as far as its log says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// A crash marker is in the log.
    Crash,
    /// Error lines, but nothing that says it went down.
    Errors,
    /// Nothing worth reading.
    Clean,
}

/// One launch of one config, inferred from its log.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Run {
    /// When the config was armed (unix seconds). Unique per game: one run per arm.
    pub id: u64,
    /// The last log write seen for this run (unix seconds).
    pub log_at: u64,
    /// Which log it came from ("Proton", "Nexus launch", "DXVK", …).
    pub source: String,
    pub verdict: Verdict,
    /// Error lines in the log tail, `fixme` noise excluded.
    pub error_count: u32,
    /// The first line that decided the verdict, for the list.
    pub first_error: Option<String>,
    /// Names of fix recipes whose `log_patterns` matched this log.
    pub matched_recipes: Vec<String>,
    /// The user's own verdict, which outranks ours: true = worked, false = broke.
    pub mark: Option<bool>,
    /// The config that ran.
    pub config: Config,
}

/// A config waiting for its run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Armed {
    pub at: u64,
    pub config: Config,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GameRuns {
    #[serde(default)]
    pub armed: Option<Armed>,
    /// Newest first.
    #[serde(default)]
    pub runs: Vec<Run>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunsFile {
    #[serde(default)]
    pub games: BTreeMap<String, GameRuns>,
}

/// What a log looked like when polled.
pub struct Observation<'a> {
    pub log_at: u64,
    pub source: &'a str,
    pub tail: &'a str,
    pub error_lines: &'a [String],
}

/// Arm `config` for the next launch. Re-arming the same config before any log
/// arrived keeps the earlier time, so copying twice doesn't move the window.
pub fn arm(g: &mut GameRuns, now: u64, config: Config) {
    let waiting_same = g.armed.as_ref().is_some_and(|a| {
        same_config(&a.config, &config) && g.runs.first().is_none_or(|r| r.id != a.at)
    });
    if !waiting_same {
        // Strictly after the last arm and run: the time is the run's id, and
        // two arms inside one second (Copy, toggle, Copy) must not share one.
        let floor = g.armed.as_ref().map(|a| a.at + 1).into_iter().chain(g.runs.first().map(|r| r.id + 1)).max();
        let at = floor.map_or(now, |f| now.max(f));
        g.armed = Some(Armed { at, config });
    }
}

/// Fold a log observation in. Returns whether anything changed.
pub fn observe(g: &mut GameRuns, obs: &Observation, recipes: &[(String, Vec<String>)]) -> bool {
    let Some(armed) = &g.armed else { return false };
    if obs.log_at < armed.at {
        return false;
    }
    let (verdict, error_count, first_error) = classify(obs.tail, obs.error_lines);
    let matched_recipes = match_recipes(obs.tail, recipes);

    if let Some(run) = g.runs.first_mut().filter(|r| r.id == armed.at) {
        if run.log_at == obs.log_at {
            return false;
        }
        run.log_at = obs.log_at;
        run.config = armed.config.clone();
        run.source = obs.source.to_string();
        run.verdict = verdict;
        run.error_count = error_count;
        run.first_error = first_error;
        run.matched_recipes = matched_recipes;
        return true;
    }
    g.runs.insert(
        0,
        Run {
            id: armed.at,
            log_at: obs.log_at,
            source: obs.source.to_string(),
            verdict,
            error_count,
            first_error,
            matched_recipes,
            mark: None,
            config: armed.config.clone(),
        },
    );
    g.runs.truncate(MAX_RUNS);
    true
}

/// Set (or clear) the user's verdict on run `id`. Returns whether it existed.
pub fn mark(g: &mut GameRuns, id: u64, mark: Option<bool>) -> bool {
    match g.runs.iter_mut().find(|r| r.id == id) {
        Some(r) => {
            r.mark = mark;
            true
        }
        None => false,
    }
}

fn classify(tail: &str, error_lines: &[String]) -> (Verdict, u32, Option<String>) {
    if let Some(line) = tail.lines().find(|l| {
        let lower = l.to_ascii_lowercase();
        CRASH_MARKERS.iter().any(|m| lower.contains(m))
    }) {
        let count = real_errors(error_lines).count() as u32;
        return (Verdict::Crash, count.max(1), Some(line.trim().to_string()));
    }
    let mut errors = real_errors(error_lines);
    let first = errors.next().map(|l| l.trim().to_string());
    let count = first.is_some() as u32 + errors.count() as u32;
    if count == 0 { (Verdict::Clean, 0, None) } else { (Verdict::Errors, count, first) }
}

fn real_errors(lines: &[String]) -> impl Iterator<Item = &String> {
    lines.iter().filter(|l| !l.to_ascii_lowercase().contains("fixme"))
}

/// Recipes (name, `log_patterns`) with any pattern in `tail`, case-folded.
pub fn match_recipes(tail: &str, recipes: &[(String, Vec<String>)]) -> Vec<String> {
    let lower = tail.to_ascii_lowercase();
    recipes
        .iter()
        .filter(|(_, pats)| pats.iter().any(|p| !p.trim().is_empty() && lower.contains(&p.to_ascii_lowercase())))
        .map(|(name, _)| name.clone())
        .collect()
}

/// Configs equal as the command sees them (serde form, so field order and
/// every field count).
fn same_config(a: &Config, b: &Config) -> bool {
    serde_json::to_string(a).ok() == serde_json::to_string(b).ok()
}

// ------------------------------------ I/O ------------------------------------

fn path() -> Option<std::path::PathBuf> {
    Some(crate::fsutil::config_home()?.join("protongen").join("runs.toml"))
}

/// Serialises read-modify-write of runs.toml across concurrent commands.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Read runs.toml, apply `f` to one game's entry, write back if `f` says it
/// changed. A missing or unreadable file starts empty (runs are disposable).
pub fn with_game<R>(app_id: u32, f: impl FnOnce(&mut GameRuns) -> (bool, R)) -> Result<R, String> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(path) = path() else { return Err("no config directory".into()) };
    let mut file: RunsFile = crate::fsutil::read_existing(&path)
        .ok()
        .flatten()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();
    let key = app_id.to_string();
    let entry = file.games.entry(key).or_default();
    let (changed, out) = f(entry);
    if changed {
        let text = toml::to_string_pretty(&file).map_err(|e| format!("couldn't serialize runs: {e}"))?;
        crate::fsutil::write_atomic(&path, text.as_bytes())?;
    }
    Ok(out)
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(env: &[(&str, &str)]) -> Config {
        Config {
            env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ..Config::default()
        }
    }

    fn obs<'a>(at: u64, tail: &'a str, errs: &'a [String]) -> Observation<'a> {
        Observation { log_at: at, source: "Proton", tail, error_lines: errs }
    }

    #[test]
    fn nothing_armed_means_no_run() {
        let mut g = GameRuns::default();
        assert!(!observe(&mut g, &obs(100, "", &[]), &[]));
        assert!(g.runs.is_empty());
    }

    #[test]
    fn a_log_older_than_the_arm_is_not_this_config() {
        let mut g = GameRuns::default();
        arm(&mut g, 200, cfg(&[("DXVK_ASYNC", "1")]));
        assert!(!observe(&mut g, &obs(150, "", &[]), &[]));
        assert!(g.runs.is_empty());
    }

    #[test]
    fn one_arm_is_one_run_updated_as_the_log_grows() {
        let mut g = GameRuns::default();
        arm(&mut g, 100, cfg(&[("DXVK_ASYNC", "1")]));
        assert!(observe(&mut g, &obs(110, "started", &[]), &[]));
        assert_eq!(g.runs.len(), 1);
        assert_eq!(g.runs[0].verdict, Verdict::Clean);

        let errs = vec!["err: vulkan device lost".to_string()];
        assert!(observe(&mut g, &obs(140, "wine: Unhandled page fault on read", &errs), &[]));
        assert_eq!(g.runs.len(), 1, "same arm → same run");
        assert_eq!(g.runs[0].verdict, Verdict::Crash);
        assert_eq!(g.runs[0].log_at, 140);
        // Same observation again: no write.
        assert!(!observe(&mut g, &obs(140, "x", &errs), &[]));

        arm(&mut g, 200, cfg(&[]));
        assert!(observe(&mut g, &obs(210, "", &[]), &[]));
        assert_eq!(g.runs.len(), 2);
        assert_eq!(g.runs[0].id, 200, "newest first");
        assert_eq!(g.runs[1].config.env[0].0, "DXVK_ASYNC");
    }

    #[test]
    fn rearming_the_same_config_before_a_launch_keeps_the_window() {
        let mut g = GameRuns::default();
        arm(&mut g, 100, cfg(&[("A", "1")]));
        arm(&mut g, 150, cfg(&[("A", "1")]));
        assert_eq!(g.armed.as_ref().unwrap().at, 100);
        arm(&mut g, 160, cfg(&[("A", "2")]));
        assert_eq!(g.armed.as_ref().unwrap().at, 160);
    }

    #[test]
    fn two_arms_in_one_second_are_two_runs() {
        let mut g = GameRuns::default();
        arm(&mut g, 100, cfg(&[("A", "1")]));
        observe(&mut g, &obs(100, "", &[]), &[]);
        arm(&mut g, 100, cfg(&[("A", "2")]));
        assert_eq!(g.armed.as_ref().unwrap().at, 101);
        observe(&mut g, &obs(101, "", &[]), &[]);
        assert_eq!(g.runs.len(), 2);
        assert_eq!(g.runs[0].config.env[0].1, "2");
    }

    #[test]
    fn fixme_lines_are_not_errors() {
        let errs = vec!["fixme:d3d stub".to_string(), "err: bad thing".to_string()];
        let (v, n, first) = classify("", &errs);
        assert_eq!(v, Verdict::Errors);
        assert_eq!(n, 1);
        assert_eq!(first.as_deref(), Some("err: bad thing"));
        assert_eq!(classify("", &["fixme: only".to_string()]).0, Verdict::Clean);
    }

    #[test]
    fn log_patterns_match_case_insensitively() {
        let recipes = vec![
            ("EAC fix".to_string(), vec!["EasyAntiCheat".to_string()]),
            ("Other".to_string(), vec!["nope".to_string(), "".to_string()]),
        ];
        assert_eq!(match_recipes("easyanticheat: untrusted file", &recipes), vec!["EAC fix"]);
    }

    #[test]
    fn runs_are_capped() {
        let mut g = GameRuns::default();
        for i in 0..(MAX_RUNS as u64 + 5) {
            arm(&mut g, i * 10, cfg(&[("N", &i.to_string())]));
            observe(&mut g, &obs(i * 10 + 1, "", &[]), &[]);
        }
        assert_eq!(g.runs.len(), MAX_RUNS);
    }

    #[test]
    fn marks_stick_to_their_run() {
        let mut g = GameRuns::default();
        arm(&mut g, 10, cfg(&[]));
        observe(&mut g, &obs(11, "", &[]), &[]);
        assert!(mark(&mut g, 10, Some(true)));
        assert_eq!(g.runs[0].mark, Some(true));
        assert!(!mark(&mut g, 99, Some(false)));
    }
}
