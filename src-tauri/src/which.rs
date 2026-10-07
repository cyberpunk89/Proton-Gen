//! Minimal `which`: is a binary on `$PATH`?

use std::path::{Path, PathBuf};

/// True if `bin` is found as an executable on `$PATH`.
pub fn is_installed(bin: &str) -> bool {
    find(bin).is_some()
}

/// Where `bin` resolves: itself when it is a path, else the first `$PATH`
/// entry holding it.
pub fn find(bin: &str) -> Option<PathBuf> {
    // Absolute / explicit paths: check directly.
    if bin.contains('/') {
        return Path::new(bin).is_file().then(|| PathBuf::from(bin));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(bin)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_sh() {
        // `sh` exists on essentially every Unix.
        assert!(is_installed("sh"));
    }

    #[test]
    fn rejects_nonsense() {
        assert!(!is_installed("definitely-not-a-real-binary-xyzzy"));
    }
}
