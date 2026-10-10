//! Filesystem helpers shared by the write paths: the store, and the sanctioned
//! Heroic inject and MangoHud/vkBasalt exports.

use std::io::Write;
use std::path::Path;

/// `$XDG_CONFIG_HOME` (when set and non-empty), else `~/.config` — the root
/// every config path here hangs off (protongen's own, Heroic's, MangoHud's,
/// vkBasalt's).
pub fn config_home() -> Option<std::path::PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(std::path::PathBuf::from(xdg));
    }
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
}

/// protongen's own cache directory: `$XDG_CACHE_HOME/protongen` (when set and
/// non-empty), else `~/.cache/protongen`. Everything in it is disposable.
pub fn cache_dir() -> Option<std::path::PathBuf> {
    let base = match std::env::var_os("XDG_CACHE_HOME").filter(|s| !s.is_empty()) {
        Some(xdg) => std::path::PathBuf::from(xdg),
        None => std::path::PathBuf::from(std::env::var_os("HOME")?).join(".cache"),
    };
    Some(base.join("protongen"))
}

/// Replace `path` with `bytes` without ever leaving a half-written file: write
/// a temp file in the same directory (so the rename can't cross filesystems),
/// fsync it, then rename over the target. A crash mid-write leaves the old
/// file intact. Creates the parent directory. Every error names the path.
///
/// A symlinked target (a dotfile manager's `MangoHud.conf -> ~/dotfiles/…`) is
/// written through: the link stays a link and its target is replaced. An
/// existing file's permissions carry over to the new one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let resolved;
    let path = if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        resolved = std::fs::canonicalize(path)
            .map_err(|e| format!("couldn't resolve symlink {}: {e}", path.display()))?;
        resolved.as_path()
    } else {
        path
    };
    let perms = std::fs::metadata(path).ok().map(|m| m.permissions());
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;

    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        if let Some(p) = &perms {
            f.set_permissions(p.clone())?;
        }
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    written.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("couldn't write {}: {e}", path.display())
    })
}

/// Save `contents` as a backup next to `path`, named `<file>.<tag>-<ts>.bak`.
/// Never overwrites: a second backup in the same second (two Applies in a
/// row) gets a `-1`, `-2`… suffix instead of replacing the pristine copy
/// with already-modified content.
pub fn write_backup(path: &Path, tag: &str, contents: &[u8]) -> Result<std::path::PathBuf, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = format!("{name}.{tag}-{}", unix_ts());
    for n in 0..1000 {
        let backup = path.with_file_name(if n == 0 { format!("{stem}.bak") } else { format!("{stem}-{n}.bak") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&backup) {
            Ok(mut f) => {
                return f
                    .write_all(contents)
                    .map(|()| backup.clone())
                    .map_err(|e| format!("Could not write backup {}: {e}", backup.display()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Could not write backup {}: {e}", backup.display())),
        }
    }
    Err(format!("Could not pick a free backup name for {}", path.display()))
}

/// Read a config file we're about to merge into. Missing is a normal empty
/// start; any other failure is an error — treating an unreadable file as
/// empty would overwrite it with only our own lines.
pub fn read_existing(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Could not read {}: {e}", path.display())),
    }
}

/// Seconds since the Unix epoch, for backup-file suffixes.
pub fn unix_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_in_the_same_second_never_overwrite_each_other() {
        let dir = scratch("backup");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("f.conf");
        let a = write_backup(&path, "protongen", b"pristine").unwrap();
        let b = write_backup(&path, "protongen", b"modified").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read(&a).unwrap(), b"pristine");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_writes_through_a_symlink_and_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("symlink");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.conf");
        std::fs::write(&real, "old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("link.conf");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_atomic(&link, b"new").unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read(&real).unwrap(), b"new");
        assert_eq!(std::fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("protongen-fsutil-{name}-{}", std::process::id()))
    }

    #[test]
    fn write_atomic_replaces_the_file_and_leaves_no_temp_behind() {
        let dir = scratch("replace");
        let path = dir.join("nested").join("f.conf");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");

        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "f.conf")
            .collect();
        assert!(leftovers.is_empty(), "stray files: {leftovers:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_atomic_names_the_path_when_the_parent_cannot_be_created() {
        let blocker = scratch("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let err = write_atomic(&blocker.join("f.conf"), b"x").unwrap_err();
        assert!(err.contains(&blocker.display().to_string()), "{err}");
        std::fs::remove_file(&blocker).ok();
    }
}
