//! Export the overlay built in protongen's MangoHud builder into the real,
//! system-wide `~/.config/MangoHud/MangoHud.conf`, so it becomes the default
//! for *every* MangoHud-enabled program on the system, not just the launch
//! command you copy out of this app.
//!
//! **A deliberate, narrow exception to the read-only-by-contract invariant**
//! (see `design.md` §11) — the third, after `heroic::inject` and
//! `optiscaler_upgrade::fetch_and_extract`. Same shape as `heroic::inject`: back
//! up first, preserve every line it doesn't own, write atomically, never gated
//! in this module itself (the frontend only ever calls `write_system_config`
//! from its confirm dialog's Apply handler). Unlike Heroic's structured JSON,
//! MangoHud.conf is a flat `key` / `key=value` text file, so "preserve what we
//! don't own" means preserving every *line* whose key isn't one this app's
//! overlay builder can express — a hand-tuned font, a toggle keybind, an app
//! blacklist, colors the builder doesn't model, and any comments — while the
//! managed lines are replaced wholesale to match the current build exactly,
//! including dropping a managed key the user has since unchecked. That's the
//! same "this becomes the default" semantics as Heroic's showMangohud/
//! useGameMode booleans, which write `false` rather than leaving a stale `true`.

use std::path::PathBuf;

/// `$XDG_CONFIG_HOME/MangoHud` (or `~/.config/MangoHud`). Mirrors
/// [`crate::params::config_dir`] but for MangoHud's config tree, not protongen's.
fn config_dir() -> Option<PathBuf> {
    crate::fsutil::config_home().map(|d| d.join("MangoHud"))
}

/// Every key protongen's overlay builder (`src/lib/mangohud.ts`'s `METRICS` /
/// `COLOR_DEFS` / positional options) can express, as the bare token
/// (`gpu_stats`) or the part before `=` (`font_size`). Anything else found in an
/// existing `MangoHud.conf` — font, keybinds, blacklist, unmodeled colors,
/// logging, comments — is left untouched because it isn't in this list.
const MANAGED_KEYS: &[&str] = &[
    "fps",
    "frame_timing",
    "cpu_stats",
    "gpu_stats",
    "cpu_temp",
    "gpu_temp",
    "ram",
    "vram",
    "gpu_name",
    "horizontal",
    "horizontal_stretch",
    "hud_compact",
    "position",
    "font_size",
    "round_corners",
    "background_alpha",
    "alpha",
    "fps_limit",
    "text_color",
    "gpu_color",
    "cpu_color",
    "background_color",
    "gpu_list",
];

/// Keys the builder can *set* but must never *clear*: the in-game hotkeys. A
/// hand-written `toggle_hud=` in an existing file is the user's own keybind, so
/// a config that doesn't mention it leaves it alone (unlike `MANAGED_KEYS`,
/// where absence means "turned off"); one that does replaces it in place.
const SET_ONLY_KEYS: &[&str] = &["toggle_hud", "toggle_fps_limit", "toggle_logging"];

/// Turn a `MANGOHUD_CONFIG`-style comma-separated string (e.g.
/// `"fps,frame_timing,font_size=14"`, the same shape `buildConfig()` in
/// `mangohud.ts` produces) into one `MangoHud.conf` line per token.
///
/// A value can itself hold commas (`gpu_list=0,1`), so a bare numeric token
/// right after a `key=value` one is a continuation of that value, not a
/// token of its own — no MangoHud bare token is a number. Same rule as
/// `splitTokens()` in `mangohud.ts`.
fn config_to_lines(config: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in config.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let continues = t.bytes().all(|b| b.is_ascii_digit()) && out.last().is_some_and(|l| l.contains('='));
        match out.last_mut() {
            Some(prev) if continues => {
                prev.push(',');
                prev.push_str(t);
            }
            _ => out.push(t.to_string()),
        }
    }
    out
}

pub use crate::conf_merge::{ExportResult, MergeOutcome};

/// Merge `new_config` (a `MANGOHUD_CONFIG` string) into `existing` (the
/// current file's text, empty if there is none): every [`MANAGED_KEYS`] line
/// is replaced, a [`SET_ONLY_KEYS`] line only when the new config sets it
/// again; everything else stays, in place (see [`crate::conf_merge::merge`]).
pub fn merge(existing: &str, new_config: &str) -> MergeOutcome {
    crate::conf_merge::merge(existing, &config_to_lines(new_config), MANAGED_KEYS, |k| SET_ONLY_KEYS.contains(&k))
}

/// Merge `config` into the real `MangoHud.conf` — backed up first, written
/// atomically. Only ever called from the confirm dialog's Apply.
pub fn write_system_config(config: &str) -> Result<ExportResult, String> {
    let dir = config_dir().ok_or_else(|| "no config directory available (is $HOME set?)".to_string())?;
    crate::conf_merge::write_merged(&dir.join("MangoHud.conf"), |existing| merge(existing, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every config key the TS builder can emit: `token: "<key>"` entries,
    /// plus string/template literals in its build function that start with a
    /// word followed by one of `ends` (`"` for a bare MangoHud token, `=` or
    /// ` =` for an assignment). A tiny scanner rather than a regex dependency.
    fn ts_emitted_keys(ts: &str, build_fn: &str, ends: &[&str]) -> Vec<String> {
        let mut keys = Vec::new();
        let mut rest = ts;
        while let Some(i) = rest.find("token: \"") {
            rest = &rest[i + 8..];
            if let Some(end) = rest.find('"') {
                keys.push(rest[..end].to_string());
            }
        }
        let start = ts.find(&format!("export function {build_fn}(")).expect("build fn");
        let body = &ts[start..start + ts[start..].find("\n}\n").expect("fn end")];
        let word = |s: &str| s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect::<String>();
        for (i, c) in body.char_indices() {
            if c == '"' || c == '`' {
                let w = word(&body[i + 1..]);
                let after = &body[i + 1 + w.len()..];
                let is_key = w.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    && ends.iter().any(|e| after.starts_with(e));
                if is_key {
                    keys.push(w);
                }
            }
        }
        keys.sort();
        keys.dedup();
        keys
    }

    /// `MANAGED_KEYS` repeats what `src/lib/mangohud.ts` can emit. A metric
    /// added there but not here would never be cleared by the system-wide
    /// export when the user unchecks it.
    #[test]
    fn managed_keys_cover_everything_the_ts_builder_emits() {
        let ts = include_str!("../../src/lib/mangohud.ts");
        let keys = ts_emitted_keys(ts, "buildConfig", &["\"", "="]);
        assert!(keys.len() > 15, "{keys:?}");
        for k in &keys {
            assert!(
                MANAGED_KEYS.contains(&k.as_str()) || SET_ONLY_KEYS.contains(&k.as_str()),
                "mangohud.ts emits `{k}`, which mangohud_export.rs doesn't own"
            );
        }
    }

    #[test]
    fn gpu_list_stays_one_line_and_is_replaced() {
        let out = merge("gpu_list=0,1\nfont_size=20\n", "fps,gpu_list=0,1,font_size=14").text;
        assert_eq!(out.matches("gpu_list").count(), 1, "{out}");
        assert!(out.contains("gpu_list=0,1\n"), "{out}");
        assert!(!out.lines().any(|l| l.trim() == "1"), "{out}");
        assert_eq!(merge(&out, "fps,gpu_list=0,1,font_size=14").text, out);
    }

    #[test]
    fn preserves_unknown_lines() {
        let existing = "\
################### File Generated by Goverlay 1.9.0 stable ###################
legacy_layout=0
font_file=/usr/share/fonts/Foo.ttf
toggle_hud=Shift_R+F12
gpu_stats
vram
blacklist=zenity,protonplus
";
        let out = merge(existing, "fps").text;
        assert!(out.contains("font_file=/usr/share/fonts/Foo.ttf"));
        assert!(out.contains("toggle_hud=Shift_R+F12"));
        assert!(out.contains("blacklist=zenity,protonplus"));
        assert!(out.contains("legacy_layout=0"));
        assert!(out.contains(
            "################### File Generated by Goverlay 1.9.0 stable ###################"
        ));
    }

    #[test]
    fn replaces_existing_managed_keys() {
        let existing = "gpu_color=AD64C1\ncpu_color=2E97CB\nfont_file=/foo.ttf\n";
        let m = merge(existing, "gpu_color=2e9762,cpu_color=2e97cb");
        assert!(m.text.contains("gpu_color=2e9762"));
        assert!(m.text.contains("cpu_color=2e97cb"));
        assert!(!m.text.contains("AD64C1"));
        assert!(m.text.contains("font_file=/foo.ttf"));
        assert_eq!(m.changed_keys, vec!["gpu_color", "cpu_color"]);
        assert!(m.cleared_keys.is_empty());
    }

    #[test]
    fn reports_managed_keys_no_longer_selected_as_cleared() {
        let existing = "fps\nframe_timing\ncpu_stats\nfont_file=/foo.ttf\n";
        let m = merge(existing, "fps");
        assert!(m.text.contains("fps"));
        assert!(!m.text.contains("frame_timing"));
        assert!(!m.text.contains("cpu_stats"));
        assert!(m.text.contains("font_file=/foo.ttf"));
        assert_eq!(m.changed_keys, vec!["fps"]);
        assert_eq!(m.cleared_keys, vec!["frame_timing", "cpu_stats"]);
    }

    #[test]
    fn appends_when_no_managed_keys_existed() {
        let existing = "font_file=/foo.ttf\ntoggle_hud=F12\n";
        let out = merge(existing, "fps,gpu_stats").text;
        assert!(out.contains("font_file=/foo.ttf"));
        assert!(out.contains("toggle_hud=F12"));
        assert!(out.contains("fps"));
        assert!(out.contains("gpu_stats"));
    }

    #[test]
    fn hotkeys_replace_in_place_but_are_never_cleared() {
        let existing = "toggle_hud=F12\nfont_file=/foo.ttf\ntoggle_logging=Shift_L+F2\n";
        let m = merge(existing, "fps,toggle_hud=Shift_R+Home");
        assert!(m.text.contains("toggle_hud=Shift_R+Home"), "{}", m.text);
        assert!(!m.text.contains("toggle_hud=F12"), "{}", m.text);
        assert_eq!(m.text.matches("toggle_hud").count(), 1, "{}", m.text);
        // Not in the new config: the user's own keybind survives, unreported.
        assert!(m.text.contains("toggle_logging=Shift_L+F2"), "{}", m.text);
        assert!(m.cleared_keys.is_empty());
        assert_eq!(merge(&m.text, "fps,toggle_hud=Shift_R+Home").text, m.text);
    }

    #[test]
    fn creates_fresh_file_when_absent() {
        let m = merge("", "fps,gpu_stats,font_size=14");
        assert_eq!(m.text, "fps\ngpu_stats\nfont_size=14\n");
        assert_eq!(m.changed_keys, vec!["fps", "gpu_stats", "font_size"]);
        assert!(m.cleared_keys.is_empty());
    }

    #[test]
    fn is_idempotent() {
        let once = merge("font_file=/foo.ttf\n", "fps,gpu_stats,font_size=14").text;
        let twice = merge(&once, "fps,gpu_stats,font_size=14").text;
        assert_eq!(once, twice);
    }

    #[test]
    fn ignores_comment_lines() {
        let existing = "# fps is disabled below\nfont_file=/foo.ttf\n";
        let out = merge(existing, "fps").text;
        assert!(out.contains("# fps is disabled below"));
        assert!(out.contains("font_file=/foo.ttf"));
        assert!(out.contains("fps"));
    }
}
