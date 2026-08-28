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
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("MangoHud"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/MangoHud"))
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
];

/// The key a `MangoHud.conf` line sets, or `None` for a blank line or `#` comment.
fn line_key(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    Some(t.split('=').next().unwrap_or(t).trim())
}

/// Turn a `MANGOHUD_CONFIG`-style comma-separated string (e.g.
/// `"fps,frame_timing,font_size=14"`, the same shape `buildConfig()` in
/// `mangohud.ts` produces) into one `MangoHud.conf` line per token.
fn config_to_lines(config: &str) -> Vec<String> {
    config
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// The result of merging a new overlay config into an existing file's text.
/// **Pure** — the tested core.
pub struct MergeOutcome {
    pub text: String,
    /// Managed keys the new config sets (added or updated).
    pub changed_keys: Vec<String>,
    /// Managed keys present in `existing` but absent from the new config —
    /// i.e. settings this write clears because they're no longer selected.
    pub cleared_keys: Vec<String>,
}

/// Merge `new_config` into `existing` (the current file's text, empty if the
/// file doesn't exist yet). Every managed line `existing` has is dropped; every
/// line this module doesn't recognize (comments, blanks, a custom font/keybind/
/// blacklist/unmanaged color) is preserved verbatim, in place. The new managed
/// lines are spliced in at the position of the first managed line found there
/// (or appended, after a separating blank line, if there wasn't one).
pub fn merge(existing: &str, new_config: &str) -> MergeOutcome {
    let new_lines = config_to_lines(new_config);
    let new_keys: Vec<&str> = new_lines.iter().filter_map(|l| line_key(l)).collect();

    if existing.trim().is_empty() {
        return MergeOutcome {
            text: format!("{}\n", new_lines.join("\n")),
            changed_keys: new_keys.into_iter().map(str::to_string).collect(),
            cleared_keys: Vec::new(),
        };
    }

    let mut out: Vec<String> = Vec::new();
    let mut cleared: Vec<String> = Vec::new();
    let mut spliced = false;
    for line in existing.lines() {
        if let Some(k) = line_key(line).filter(|k| MANAGED_KEYS.contains(k)) {
            if !new_keys.contains(&k) && !cleared.iter().any(|c| c == k) {
                cleared.push(k.to_string());
            }
            if !spliced {
                out.extend(new_lines.iter().cloned());
                spliced = true;
            }
            continue; // drop the old managed line either way
        }
        out.push(line.to_string());
    }
    if !spliced {
        if out.last().is_some_and(|l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.extend(new_lines.iter().cloned());
    }

    MergeOutcome {
        text: format!("{}\n", out.join("\n")),
        changed_keys: new_keys.into_iter().map(str::to_string).collect(),
        cleared_keys: cleared,
    }
}

/// What a successful [`write_system_config`] wrote, for the confirm dialog / toast.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ExportResult {
    pub config_path: String,
    /// `None` when there was no pre-existing file to back up.
    pub backup_path: Option<String>,
    pub changed_keys: Vec<String>,
    pub cleared_keys: Vec<String>,
}

/// Write `config` (a `MANGOHUD_CONFIG`-style string) into the real, system-wide
/// `MangoHud.conf`, merging with whatever's already there. Backs the file up
/// first if it existed, then writes atomically. Impure; thin.
pub fn write_system_config(config: &str) -> Result<ExportResult, String> {
    let dir = config_dir().ok_or_else(|| "no config directory available (is $HOME set?)".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = dir.join("MangoHud.conf");

    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let backup_path = if path.exists() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = path.with_file_name(format!("MangoHud.conf.protongen-{ts}.bak"));
        std::fs::write(&backup, &existing)
            .map_err(|e| format!("Could not write backup {}: {e}", backup.display()))?;
        Some(backup.display().to_string())
    } else {
        None
    };

    let merged = merge(&existing, config);

    // Atomic replace: temp in the same dir, then rename over the original.
    let tmp = path.with_file_name("MangoHud.conf.protongen-tmp");
    std::fs::write(&tmp, merged.text.as_bytes())
        .map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Could not replace {}: {e}", path.display()))?;

    Ok(ExportResult {
        config_path: path.display().to_string(),
        backup_path,
        changed_keys: merged.changed_keys,
        cleared_keys: merged.cleared_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
