//! Where a game's logs are, and reading their tails for the diagnostics
//! viewer. Read-only.
//!
//! This used to be `$HOME/steam-<appid>.log` and nothing else, which missed:
//! - a `PROTON_LOG_DIR` set in the game's own config;
//! - games run through umu, where Proton names the log after umu's game id
//!   (`steam-0.log` for the default `GAMEID=umu-0`), not the Steam appid;
//! - Nexus's own launch log for its games (`<slug>-last-launch.log`, the
//!   game's stdout/stderr — Wine's `err:` lines land there even with no
//!   `PROTON_LOG`);
//! - DXVK and VKD3D-Proton's own logs.
//!
//! [`candidates`] lists them for one game and its current config; the viewer
//! shows the newest one present and lets the user switch. Paths only ever come
//! from that list, never from the webview.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::ipc::GameDto;
use crate::store::Config;

/// Bytes of log tail to return. Proton logs can reach hundreds of MB over a long
/// session; the viewer only ever needs the end, and the head is stale by then.
pub const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// Cap on surfaced error lines, so a log that is nothing but warnings can't
/// balloon the payload the frontend has to render.
const LOG_ERROR_LINES: usize = 200;

/// One place a log for this game may be.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LogSource {
    /// Stable within one game + config: what the viewer sends back to pick it.
    pub id: String,
    pub label: String,
    pub path: String,
    pub present: bool,
    pub size: u64,
    /// Unix seconds of the last write, when present.
    pub modified: Option<u64>,
}

/// One game's log, read for the diagnostics viewer.
///
/// A read-only view of one [`LogSource`]; a missing file is a normal
/// `present: false` result, not an error, so the viewer can say "no log yet —
/// enable logging and relaunch" rather than showing a failure.
#[derive(Clone, Serialize)]
pub struct ProtonLog {
    /// Whether the log file exists.
    pub present: bool,
    /// The path we looked at, shown even when absent so the user knows where the
    /// log will appear.
    pub path: String,
    /// The tail of the log (last [`LOG_TAIL_BYTES`]), or empty when absent.
    pub tail: String,
    /// Total size in bytes.
    pub size: u64,
    /// True when the file was larger than the tail we returned (the head was cut).
    pub truncated: bool,
    /// Lines from the tail matching common error/warning markers, surfaced first
    /// so the likely-relevant bits are one glance away.
    pub error_lines: Vec<String>,
    /// Which source this is.
    pub source_id: String,
    /// Every place a log for this game may be, present ones newest first.
    pub sources: Vec<LogSource>,
}

/// True when a log line looks like something worth reading first. Case-folded
/// substring match on a small marker set — deliberately conservative so the
/// "problems" list stays short enough to scan.
pub fn is_error_line(line: &str) -> bool {
    const MARKERS: [&str; 8] =
        ["err", "fail", "crash", "fixme", "abort", "assert", "unsupported", "not found"];
    let lower = line.to_ascii_lowercase();
    MARKERS.iter().any(|m| lower.contains(m))
}

/// The value `key` gets from `cfg`: the custom-env field wins over a catalog
/// row, as on the command line (it comes later).
fn env_value(cfg: &Config, key: &str) -> Option<String> {
    crate::compose::parse_extra_env(&cfg.extra_env)
        .into_iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .or_else(|| cfg.env.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.clone()))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// `~/x`, `$HOME/x` and `${HOME}/x` against `home` — the only expansions a
/// log path in a launch line realistically uses.
fn expand_home(path: &str, home: &Path) -> PathBuf {
    for prefix in ["~/", "$HOME/", "${HOME}/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// The number Proton names an umu-run log after: umu passes the digits of
/// `GAMEID` (`umu-1245620` → `1245620`), and `0` for anything else, the
/// default `umu-0` included.
fn umu_log_id(gameid: &str) -> String {
    let id = gameid.trim().strip_prefix("umu-").unwrap_or(gameid.trim());
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
        id.to_string()
    } else {
        "0".to_string()
    }
}

/// Every log location for `game` (or a bare `app_id`) under `cfg`, in a
/// fixed order: Proton, Nexus's launch log, DXVK, VKD3D. Not stat'ed.
pub fn candidates(home: &Path, app_id: u32, game: Option<&GameDto>, cfg: &Config) -> Vec<(String, String, PathBuf)> {
    let mut out: Vec<(String, String, PathBuf)> = Vec::new();
    let mut push = |id: String, label: String, path: PathBuf| {
        if !out.iter().any(|(_, _, p)| *p == path) {
            out.push((id, label, path));
        }
    };

    let nexus = game.filter(|g| g.source == "nexus");
    let log_dir = env_value(cfg, "PROTON_LOG_DIR").map(|d| expand_home(&d, home));
    let proton_dir = log_dir.as_deref().unwrap_or(home);

    // Proton's own log (PROTON_LOG=1). Steam names it after the appid; umu
    // after its game id. Nexus launches through umu whatever this config's mode.
    if cfg.umu || nexus.is_some() {
        let id = umu_log_id(&cfg.umu_gameid);
        push("proton-umu".into(), format!("Proton (umu, steam-{id}.log)"), proton_dir.join(format!("steam-{id}.log")));
    }
    if !cfg.umu || nexus.is_some() {
        push("proton".into(), "Proton".into(), proton_dir.join(format!("steam-{app_id}.log")));
    }

    // Nexus records each launch's output per game.
    if let Some(slug) = nexus.and_then(|g| g.nexus_slug.as_deref()) {
        if let Some(dir) = crate::nexus::data_dir() {
            push(
                "nexus".into(),
                "Nexus launch".into(),
                dir.join("logs").join(format!("{slug}-last-launch.log")),
            );
        }
    }

    // DXVK: `<exe>_<api>.log` in DXVK_LOG_PATH, else next to the exe (the
    // game's working directory, which is where Proton starts it).
    let exe = game
        .and_then(|g| g.executable.clone())
        .or_else(|| (!cfg.umu_exe.trim().is_empty()).then(|| cfg.umu_exe.clone()));
    if let Some(exe) = exe {
        let exe = Path::new(&exe);
        let stem = exe.file_stem().map(|s| s.to_string_lossy().into_owned());
        let dir = env_value(cfg, "DXVK_LOG_PATH")
            .map(|d| expand_home(&d, home))
            .or_else(|| exe.parent().map(Path::to_path_buf));
        if let (Some(stem), Some(dir)) = (stem, dir) {
            for api in ["d3d11", "dxgi", "d3d9", "d3d10core", "d3d8"] {
                push(format!("dxvk-{api}"), format!("DXVK ({api})"), dir.join(format!("{stem}_{api}.log")));
            }
        }
    }

    // VKD3D-Proton only writes a file when told where.
    if let Some(f) = env_value(cfg, "VKD3D_LOG_FILE") {
        push("vkd3d".into(), "VKD3D-Proton".into(), expand_home(&f, home));
    }
    out
}

/// Stat each candidate. Present ones come first, newest first; absent ones
/// keep their order (the first is where a fresh Proton log will appear).
pub fn sources(cands: &[(String, String, PathBuf)]) -> Vec<LogSource> {
    let mut v: Vec<LogSource> = cands
        .iter()
        .map(|(id, label, path)| {
            let meta = std::fs::metadata(path).ok().filter(|m| m.is_file());
            LogSource {
                id: id.clone(),
                label: label.clone(),
                path: path.display().to_string(),
                present: meta.is_some(),
                size: meta.as_ref().map_or(0, |m| m.len()),
                modified: meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            }
        })
        .collect();
    // Stable sort: absent entries (modified None) keep their relative order.
    v.sort_by_key(|s| std::cmp::Reverse((s.present, s.modified)));
    v
}

/// Read the last [`LOG_TAIL_BYTES`] of `path`, dropping a partial first line
/// when the head was cut, and pick out the error-looking lines.
pub fn read_tail(path: &Path) -> ProtonLog {
    use std::io::{Read, Seek, SeekFrom};

    let path_str = path.display().to_string();
    let absent = || ProtonLog {
        present: false,
        path: path_str.clone(),
        tail: String::new(),
        size: 0,
        truncated: false,
        error_lines: Vec::new(),
        source_id: String::new(),
        sources: Vec::new(),
    };

    let Ok(meta) = std::fs::metadata(path) else {
        return absent();
    };
    let size = meta.len();
    let truncated = size > LOG_TAIL_BYTES;

    let Ok(mut file) = std::fs::File::open(path) else {
        return absent();
    };
    if truncated {
        // Seek to the last window; a failed seek just means we read from 0.
        let _ = file.seek(SeekFrom::Start(size - LOG_TAIL_BYTES));
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return absent();
    }

    let mut tail = String::from_utf8_lossy(&buf).into_owned();
    // When we seeked mid-file we almost certainly landed inside a line; drop that
    // partial fragment so the first shown line is whole.
    if truncated {
        if let Some(nl) = tail.find('\n') {
            tail = tail[nl + 1..].to_string();
        }
    }

    let error_lines = tail
        .lines()
        .filter(|l| is_error_line(l))
        .take(LOG_ERROR_LINES)
        .map(|l| l.trim().to_string())
        .collect();

    ProtonLog { present: true, tail, size, truncated, error_lines, ..absent() }
}

/// The log the viewer asked for (`source_id`), else the newest one present,
/// else where the Proton log will appear — with the full source list.
pub fn read(home: &Path, app_id: u32, game: Option<&GameDto>, cfg: &Config, source_id: Option<&str>) -> ProtonLog {
    let cands = candidates(home, app_id, game, cfg);
    let sources = sources(&cands);
    let pick = source_id
        .and_then(|id| sources.iter().find(|s| s.id == id))
        .or_else(|| sources.first());
    let Some(pick) = pick else {
        return ProtonLog {
            sources,
            ..read_tail(&home.join(format!("steam-{app_id}.log")))
        };
    };
    let id = pick.id.clone();
    let log = read_tail(Path::new(&pick.path));
    ProtonLog { source_id: id, sources, ..log }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("protongen-logs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ids(c: &[(String, String, PathBuf)]) -> Vec<&str> {
        c.iter().map(|(id, _, _)| id.as_str()).collect()
    }

    #[test]
    fn error_lines_match_the_common_markers_case_insensitively() {
        assert!(is_error_line("wine: FIXME:module stub"));
        assert!(is_error_line("err:  vulkan device lost"));
        assert!(is_error_line("Assertion failed"));
        assert!(is_error_line("file not found"));
        assert!(!is_error_line("info: loaded 42 shaders"));
        assert!(!is_error_line("frame time 16ms"));
    }

    #[test]
    fn steam_mode_uses_the_appid_and_respects_proton_log_dir() {
        let home = Path::new("/home/u");
        let cfg = Config::default();
        let c = candidates(home, 1245620, None, &cfg);
        assert_eq!(c[0].2, Path::new("/home/u/steam-1245620.log"));

        let mut cfg = Config::default();
        cfg.env = vec![("PROTON_LOG_DIR".into(), "$HOME/logs".into())];
        assert_eq!(candidates(home, 7, None, &cfg)[0].2, Path::new("/home/u/logs/steam-7.log"));
        cfg.extra_env = "PROTON_LOG_DIR=~/other".into();
        assert_eq!(candidates(home, 7, None, &cfg)[0].2, Path::new("/home/u/other/steam-7.log"), "custom env wins");
    }

    #[test]
    fn umu_mode_names_the_log_after_the_game_id() {
        assert_eq!(umu_log_id(""), "0");
        assert_eq!(umu_log_id("umu-0"), "0");
        assert_eq!(umu_log_id("umu-1245620"), "1245620");
        assert_eq!(umu_log_id("umu-default"), "0");
        let mut cfg = Config::default();
        cfg.umu = true;
        cfg.umu_exe = "/g/Game.exe".into();
        let c = candidates(Path::new("/h"), 9, None, &cfg);
        assert_eq!(ids(&c)[0], "proton-umu");
        assert_eq!(c[0].2, Path::new("/h/steam-0.log"));
        assert!(!ids(&c).contains(&"proton"));
        assert!(c.iter().any(|(_, _, p)| p == Path::new("/g/Game_d3d11.log")));
    }

    #[test]
    fn vkd3d_only_when_a_file_is_named() {
        let mut cfg = Config::default();
        assert!(!ids(&candidates(Path::new("/h"), 1, None, &cfg)).contains(&"vkd3d"));
        cfg.env = vec![("VKD3D_LOG_FILE".into(), "~/vkd3d.log".into())];
        let c = candidates(Path::new("/h"), 1, None, &cfg);
        assert!(c.iter().any(|(id, _, p)| id == "vkd3d" && p == Path::new("/h/vkd3d.log")));
    }

    #[test]
    fn read_picks_the_newest_present_log_and_honours_a_choice() {
        let home = tmp("pick");
        let exe_dir = home.join("game");
        std::fs::create_dir_all(&exe_dir).unwrap();
        let mut cfg = Config::default();
        cfg.umu_exe = exe_dir.join("G.exe").display().to_string();
        std::fs::write(home.join("steam-5.log"), "info: ok\nerr: device lost\n").unwrap();
        // Make the DXVK log strictly newer.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(exe_dir.join("G_d3d11.log"), "warn: something\n").unwrap();

        let auto = read(&home, 5, None, &cfg, None);
        assert_eq!(auto.source_id, "dxvk-d3d11");
        assert!(auto.sources[0].present && auto.sources[1].present);

        let chosen = read(&home, 5, None, &cfg, Some("proton"));
        assert_eq!(chosen.source_id, "proton");
        assert_eq!(chosen.error_lines, vec!["err: device lost"]);

        // An id that isn't a candidate can't redirect the read anywhere.
        assert_eq!(read(&home, 5, None, &cfg, Some("../../etc/passwd")).source_id, "dxvk-d3d11");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn read_tail_keeps_only_whole_lines_of_the_last_window() {
        let dir = tmp("tail");
        let p = dir.join("big.log");
        let mut text = String::new();
        while text.len() < (LOG_TAIL_BYTES as usize) + 5000 {
            text.push_str("info: filler line\n");
        }
        text.push_str("err: the end\n");
        std::fs::write(&p, &text).unwrap();
        let log = read_tail(&p);
        assert!(log.present && log.truncated);
        assert_eq!(log.size, text.len() as u64);
        assert!(log.tail.starts_with("info: filler line\n"), "no partial first line");
        assert_eq!(log.error_lines, vec!["err: the end"]);
        assert!(!read_tail(&dir.join("missing.log")).present);
        std::fs::remove_dir_all(&dir).ok();
    }
}
