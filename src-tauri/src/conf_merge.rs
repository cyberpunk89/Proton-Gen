//! The shared core of the MangoHud.conf and vkBasalt.conf exports: merge a
//! builder's lines into a flat `key` / `key=value` config file the user may
//! also hand-edit, and write the result (backup first, atomically).
//!
//! The two exports differ only in which keys they own and how their builder's
//! output becomes lines; everything else — what "preserve what we don't own"
//! means, where new lines go, what counts as cleared — is this module.

use std::path::Path;

/// The key a config line sets, or `None` for a blank line or `#` comment.
pub fn line_key(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    Some(t.split('=').next().unwrap_or(t).trim())
}

/// The result of merging new lines into an existing file's text.
/// **Pure** — the tested core.
pub struct MergeOutcome {
    pub text: String,
    /// Keys the new config sets (added or updated).
    pub changed_keys: Vec<String>,
    /// Managed keys present in `existing` but absent from the new config —
    /// i.e. settings this write clears because they're no longer selected.
    pub cleared_keys: Vec<String>,
}

/// Merge `new_lines` into `existing` (the current file's text, empty if the
/// file doesn't exist yet).
///
/// A line of `existing` is replaced when its key is in `managed` (the
/// builder owns it: absent from the new config means "turned off", so it's
/// dropped and reported cleared), or when `replace_if_set(key)` holds *and*
/// the new config sets that key (a key the builder can set but must never
/// clear, like a user's own keybind). Every other line — comments, blanks,
/// settings the builder doesn't model — is kept verbatim, in place. The new
/// lines are spliced in where the first replaced line was, or appended after
/// a separating blank line when there wasn't one.
pub fn merge(
    existing: &str,
    new_lines: &[String],
    managed: &[&str],
    replace_if_set: impl Fn(&str) -> bool,
) -> MergeOutcome {
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
        let replaced = |k: &&str| managed.contains(k) || (replace_if_set(k) && new_keys.contains(k));
        if let Some(k) = line_key(line).filter(replaced) {
            if managed.contains(&k) && !new_keys.contains(&k) && !cleared.iter().any(|c| c == k) {
                cleared.push(k.to_string());
            }
            if !spliced {
                out.extend(new_lines.iter().cloned());
                spliced = true;
            }
            continue; // drop the old line either way
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

/// What a system-config export reports back to the confirm dialog.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ExportResult {
    pub config_path: String,
    /// Where the previous file was backed up; `None` when there wasn't one.
    pub backup_path: Option<String>,
    pub changed_keys: Vec<String>,
    pub cleared_keys: Vec<String>,
}

/// Read `path` (or nothing, if it doesn't exist), back it up, merge with
/// `merge_fn`, and write the result atomically.
pub fn write_merged(path: &Path, merge_fn: impl FnOnce(&str) -> MergeOutcome) -> Result<ExportResult, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    let existing = crate::fsutil::read_existing(path)?;
    let backup_path = match &existing {
        Some(text) => Some(crate::fsutil::write_backup(path, "protongen", text.as_bytes())?.display().to_string()),
        None => None,
    };
    let merged = merge_fn(&existing.unwrap_or_default());
    crate::fsutil::write_atomic(path, merged.text.as_bytes())?;
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

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn replace_if_set_keys_are_replaced_only_when_set() {
        let existing = "toggle_hud=Shift_R+F12\nfps\n";
        let keep = merge(existing, &lines(&["fps"]), &["fps"], |k| k == "toggle_hud");
        assert!(keep.text.contains("toggle_hud=Shift_R+F12"));
        let swap = merge(existing, &lines(&["fps", "toggle_hud=F1"]), &["fps"], |k| k == "toggle_hud");
        assert!(!swap.text.contains("Shift_R+F12") && swap.text.contains("toggle_hud=F1"));
        assert!(swap.cleared_keys.is_empty(), "a set-only key is never reported cleared");
    }

    #[test]
    fn write_merged_backs_up_and_writes() {
        let dir = std::env::temp_dir().join(format!("protongen-confmerge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sub/x.conf");
        let r = write_merged(&path, |e| merge(e, &lines(&["a=1"]), &["a"], |_| false)).unwrap();
        assert!(r.backup_path.is_none(), "nothing to back up the first time");
        let r = write_merged(&path, |e| merge(e, &lines(&["a=2"]), &["a"], |_| false)).unwrap();
        assert!(r.backup_path.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a=2\n");
        std::fs::remove_dir_all(&dir).ok();
    }
}
