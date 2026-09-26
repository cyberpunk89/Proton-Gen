//! Filesystem helpers shared by every write path (the store and the
//! sanctioned MangoHud/vkBasalt exports).

use std::io::Write;
use std::path::Path;

/// Replace `path` with `bytes` without ever leaving a half-written file: write
/// a temp file in the same directory (so the rename can't cross filesystems),
/// fsync it, then rename over the target. A crash mid-write leaves the old
/// file intact. Creates the parent directory. Every error names the path.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;

    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    written.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("couldn't write {}: {e}", path.display())
    })
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
