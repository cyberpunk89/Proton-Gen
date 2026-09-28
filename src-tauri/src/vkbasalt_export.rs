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
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("vkBasalt"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/vkBasalt"))
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

/// The key a `vkBasalt.conf` line sets, or `None` for a blank line or `#`
/// comment. vkBasalt keys are case-sensitive camelCase, unlike MangoHud's
/// lowercase-only keys, so callers must match `MANAGED_KEYS` exactly rather
/// than case-folding.
fn line_key(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    Some(t.split('=').next().unwrap_or(t).trim())
}

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

/// The result of merging a new effect chain into an existing file's text.
/// **Pure** — the tested core. Identical shape to
/// [`crate::mangohud_export::MergeOutcome`].
pub struct MergeOutcome {
    pub text: String,
    /// Managed keys the new config sets (added or updated).
    pub changed_keys: Vec<String>,
    /// Managed keys present in `existing` but absent from the new config —
    /// i.e. settings this write clears because they're no longer selected.
    pub cleared_keys: Vec<String>,
}

/// Merge `new_config` into `existing` (the current file's text, empty if the
/// file doesn't exist yet). Every managed line `existing` has is dropped;
/// every line this module doesn't recognize (comments, blanks, `lutFile`,
/// `deband*`, ReShade paths, a hand-added custom-effect shader path) is
/// preserved verbatim, in place. The new managed lines are spliced in at the
/// position of the first managed line found there (or appended, after a
/// separating blank line, if there wasn't one).
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
        // The new block wins for every key it sets, managed or not: the
        // builder seeds its passthrough from this very file and re-emits it,
        // so keeping the old copy too would duplicate it on every Apply.
        if let Some(k) = line_key(line).filter(|k| MANAGED_KEYS.contains(k) || new_keys.contains(k)) {
            if MANAGED_KEYS.contains(&k) && !new_keys.contains(&k) && !cleared.iter().any(|c| c == k) {
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

/// What a successful [`write_system_config`] wrote, for the confirm dialog /
/// toast. Identical shape to [`crate::mangohud_export::ExportResult`].
#[derive(Clone, Debug, serde::Serialize)]
pub struct ExportResult {
    pub config_path: String,
    /// `None` when there was no pre-existing file to back up.
    pub backup_path: Option<String>,
    pub changed_keys: Vec<String>,
    pub cleared_keys: Vec<String>,
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

/// Write `config` (a newline-delimited `key = value` block) into the real,
/// system-wide `vkBasalt.conf`, merging with whatever's already there. Backs
/// the file up first if it existed, then writes atomically. Impure; thin.
pub fn write_system_config(config: &str) -> Result<ExportResult, String> {
    let dir = config_dir().ok_or_else(|| "no config directory available (is $HOME set?)".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = dir.join("vkBasalt.conf");

    let existing = crate::fsutil::read_existing(&path)?;
    let backup_path = match &existing {
        Some(text) => Some(crate::fsutil::write_backup(&path, "protongen", text.as_bytes())?.display().to_string()),
        None => None,
    };
    let existing = existing.unwrap_or_default();

    let merged = merge(&existing, config);
    crate::fsutil::write_atomic(&path, merged.text.as_bytes())?;

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
