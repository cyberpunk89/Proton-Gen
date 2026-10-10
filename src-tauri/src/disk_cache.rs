//! A tiny time-boxed JSON cache under `$XDG_CACHE_HOME/protongen/` for the
//! opt-in network lookups, so a launch doesn't re-ask GitHub / ProtonDB for
//! answers that change daily at most. (Unauthenticated GitHub API calls are
//! rate-limited to 60/hour per IP — every launch re-checking two or three
//! repos used to spend that on nothing.)
//!
//! Best-effort throughout: a missing, corrupt or expired entry reads as a
//! miss, and a failed write is ignored. Only successes are ever stored, so a
//! failure is retried next time instead of being remembered.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::fsutil;

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    /// Unix seconds when `value` was fetched.
    fetched: u64,
    value: T,
}

fn path(name: &str) -> Option<std::path::PathBuf> {
    Some(fsutil::cache_dir()?.join(name))
}

/// `name`'s value if it was stored less than `ttl` ago.
pub fn get<T: DeserializeOwned>(name: &str, ttl: Duration) -> Option<T> {
    let text = std::fs::read_to_string(path(name)?).ok()?;
    fresh(&text, ttl, fsutil::unix_ts())
}

fn fresh<T: DeserializeOwned>(text: &str, ttl: Duration, now: u64) -> Option<T> {
    let e: Entry<T> = serde_json::from_str(text).ok()?;
    // A clock that went backwards reads as expired rather than eternal.
    (e.fetched <= now && now - e.fetched < ttl.as_secs()).then_some(e.value)
}

/// Store `value` under `name`, stamped now.
pub fn put<T: Serialize>(name: &str, value: &T) {
    let Some(p) = path(name) else { return };
    let Ok(json) = serde_json::to_string(&Entry { fetched: fsutil::unix_ts(), value }) else { return };
    let _ = fsutil::write_atomic(&p, json.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honours_the_ttl_and_a_backwards_clock() {
        let text = serde_json::to_string(&Entry { fetched: 1000, value: "v" }).unwrap();
        let ttl = Duration::from_secs(60);
        assert_eq!(fresh::<String>(&text, ttl, 1030).as_deref(), Some("v"));
        assert_eq!(fresh::<String>(&text, ttl, 1060), None);
        assert_eq!(fresh::<String>(&text, ttl, 900), None);
        assert_eq!(fresh::<String>("garbage", ttl, 1030), None);
    }
}
