//! Minimal `which`: is a binary on `$PATH`?

use std::path::Path;

/// True if `bin` is found as an executable on `$PATH`.
pub fn is_installed(bin: &str) -> bool {
    // Absolute / explicit paths: check directly.
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(bin).is_file())
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
