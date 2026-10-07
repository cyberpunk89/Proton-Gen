//! Export the effect chain built in protongen's vkBasalt builder into the
//! real, system-wide `~/.config/vkBasalt/vkBasalt.conf`, so it becomes the
//! default for *every* vkBasalt-enabled program on the system, not just the
//! launch command you copy out of this app.
//!
//! **A deliberate, narrow exception to the read-only-by-contract invariant**
//! (see `design.md` §11) — the fourth, after `heroic::inject`,
//! `optiscaler_upgrade::fetch_and_extract` and `mangohud_export::write_system_config`.
//! Same shape as those: back up first, preserve every line it doesn't own,
//! write atomically, never gated in this module itself (the frontend only
//! ever calls [`write_system_config`] from its confirm dialog's Apply
//! handler). Unlike MangoHud.conf's bare-token-or-`key=value` mix,
//! `vkBasalt.conf` is always `key = value` (spaces around `=`, no bare
//! tokens), so "preserve what we don't own" means preserving every *line*
//! whose key isn't one this app's effect-chain builder can express — a
//! custom ReShade shader path, `lutFile`, `deband*` tuning, and any comments —
//! while the managed lines are replaced wholesale to match the current build
//! exactly, including dropping a managed key the user has since unchecked.

use std::path::PathBuf;

/// `$XDG_CONFIG_HOME/vkBasalt` (or `~/.config/vkBasalt`). Mirrors
/// [`crate::mangohud_export::config_dir`]'s shape but for vkBasalt's own
/// config tree.
fn config_dir() -> Option<PathBuf> {
    crate::fsutil::config_home().map(|d| d.join("vkBasalt"))
}

/// Every key protongen's effect-chain builder (`src/lib/vkbasalt.ts`) can
/// express. Anything else found in an existing `vkBasalt.conf` — `lutFile`,
/// `deband*` tuning, `reshadeTexturePath`/`reshadeIncludePath`, a custom
/// ReShade effect's own shader-path line, comments — is left untouched
/// because it isn't in this list.
const MANAGED_KEYS: &[&str] = &[
    "effects",
    "casSharpness",
    "dlsSharpness",
    "dlsDenoise",
    "fxaaQualitySubpix",
    "fxaaQualityEdgeThreshold",
    "fxaaQualityEdgeThresholdMin",
    "smaaEdgeDetection",
    "smaaThreshold",
    "smaaMaxSearchSteps",
    "smaaMaxSearchStepsDiag",
    "smaaCornerRounding",
    "toggleKey",
    "enableOnLaunch",
];

/// Turn the builder's newline-delimited `key = value` block (the shape
/// `buildVkBasalt()` in `vkbasalt.ts` produces) into one line per entry.
fn config_to_lines(config: &str) -> Vec<String> {
    config
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

pub use crate::conf_merge::{ExportResult, MergeOutcome};

/// Merge `new_config` (the builder's `key = value` lines) into `existing`:
/// every [`MANAGED_KEYS`] line is replaced, and so is any other line whose key
/// the new config sets (an effect's own shader-path line); everything else
/// stays, in place (see [`crate::conf_merge::merge`]).
pub fn merge(existing: &str, new_config: &str) -> MergeOutcome {
    crate::conf_merge::merge(existing, &config_to_lines(new_config), MANAGED_KEYS, |_| true)
}

/// Read the real `vkBasalt.conf`'s current text, so the builder dialog can
/// seed itself from it and never clobber a hand-tuned file. Empty string if
/// the file doesn't exist yet — that's a normal "nothing set up" state, not
/// an error.
pub fn read_current_config() -> Result<String, String> {
    let dir = config_dir().ok_or_else(|| "no config directory available (is $HOME set?)".to_string())?;
    let path = dir.join("vkBasalt.conf");
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("Could not read {}: {e}", path.display())),
    }
}

/// Merge `config` into the real `vkBasalt.conf` — backed up first, written
/// atomically. Only ever called from the confirm dialog's Apply.
pub fn write_system_config(config: &str) -> Result<ExportResult, String> {
    let dir = config_dir().ok_or_else(|| "no config directory available (is $HOME set?)".to_string())?;
    crate::conf_merge::write_merged(&dir.join("vkBasalt.conf"), |existing| merge(existing, config))
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

    /// Same guard as MangoHud's: every key `src/lib/vkbasalt.ts` writes must
    /// be one this export owns, or unchecking it would never clear it.
    #[test]
    fn managed_keys_cover_everything_the_ts_builder_emits() {
        let ts = include_str!("../../src/lib/vkbasalt.ts");
        let keys = ts_emitted_keys(ts, "buildVkBasalt", &[" ="]);
        assert!(keys.len() > 10, "{keys:?}");
        for k in &keys {
            assert!(MANAGED_KEYS.contains(&k.as_str()), "vkbasalt.ts emits `{k}`, which vkbasalt_export.rs doesn't own");
        }
    }

    #[test]
    fn preserves_unknown_lines() {
        let existing = "\
# vkBasalt config
effects = cas
casSharpness = 0.6
lutFile = /home/user/luts/warm.cube
reshadeTexturePath = /home/user/reshade-shaders/Textures
";
        let out = merge(existing, "effects = fxaa\nfxaaQualitySubpix = 0.75").text;
        assert!(out.contains("# vkBasalt config"));
        assert!(out.contains("lutFile = /home/user/luts/warm.cube"));
        assert!(out.contains("reshadeTexturePath = /home/user/reshade-shaders/Textures"));
    }

    #[test]
    fn replaces_existing_managed_keys() {
        let existing = "effects = cas\ncasSharpness = 0.6\nlutFile = /foo.cube\n";
        let m = merge(existing, "effects = cas\ncasSharpness = 0.4");
        assert!(m.text.contains("casSharpness = 0.4"));
        assert!(!m.text.contains("0.6"));
        assert!(m.text.contains("lutFile = /foo.cube"));
        assert_eq!(m.changed_keys, vec!["effects", "casSharpness"]);
        assert!(m.cleared_keys.is_empty());
    }

    #[test]
    fn reports_managed_keys_no_longer_selected_as_cleared() {
        let existing = "effects = cas:fxaa\ncasSharpness = 0.4\nfxaaQualitySubpix = 0.75\nlutFile = /foo.cube\n";
        let m = merge(existing, "effects = cas\ncasSharpness = 0.4");
        assert!(m.text.contains("effects = cas\n") || m.text.contains("effects = cas\r\n"));
        assert!(!m.text.contains("fxaaQualitySubpix"));
        assert!(m.text.contains("lutFile = /foo.cube"));
        assert_eq!(m.cleared_keys, vec!["fxaaQualitySubpix"]);
    }

    #[test]
    fn appends_when_no_managed_keys_existed() {
        let existing = "lutFile = /foo.cube\ndebandRange = 16.0\n";
        let out = merge(existing, "effects = cas\ncasSharpness = 0.4").text;
        assert!(out.contains("lutFile = /foo.cube"));
        assert!(out.contains("debandRange = 16.0"));
        assert!(out.contains("effects = cas"));
        assert!(out.contains("casSharpness = 0.4"));
    }

    #[test]
    fn creates_fresh_file_when_absent() {
        let m = merge("", "effects = cas\ncasSharpness = 0.4");
        assert_eq!(m.text, "effects = cas\ncasSharpness = 0.4\n");
        assert_eq!(m.changed_keys, vec!["effects", "casSharpness"]);
        assert!(m.cleared_keys.is_empty());
    }

    #[test]
    fn passthrough_re_emitted_by_the_builder_is_not_duplicated() {
        // The builder seeds `passthrough` from this file and re-emits it, so
        // the new block carries `lutFile` too — it must replace, not repeat.
        let existing = "effects = cas\nlutFile = /a.cube\nmyfx = /x.fx\n";
        let new = "effects = cas\nlutFile = /a.cube\nmyfx = /x.fx";
        let once = merge(existing, new).text;
        assert_eq!(once.matches("lutFile").count(), 1, "{once}");
        assert_eq!(once.matches("myfx").count(), 1, "{once}");
        assert_eq!(merge(&once, new).text, once);
    }

    #[test]
    fn is_idempotent() {
        let once = merge("lutFile = /foo.cube\n", "effects = cas\ncasSharpness = 0.4").text;
        let twice = merge(&once, "effects = cas\ncasSharpness = 0.4").text;
        assert_eq!(once, twice);
    }

    #[test]
    fn ignores_comment_lines() {
        let existing = "# cas is disabled below\nlutFile = /foo.cube\n";
        let out = merge(existing, "effects = cas").text;
        assert!(out.contains("# cas is disabled below"));
        assert!(out.contains("lutFile = /foo.cube"));
        assert!(out.contains("effects = cas"));
    }
}
