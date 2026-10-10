//! Opt-in ProtonDB lookup for a Steam app id.
//!
//! Uses the official summary endpoint, which returns only compatibility stats
//! (tier / confidence / score / report count) — no launch commands. Fetched
//! synchronously via `ehttp::fetch_blocking`; the Tauri command wraps it in a
//! blocking task so it never stalls the UI.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Tier {
    pub tier: String,
    pub total: u64,
    pub confidence: String,
    /// Rendered beside the overall tier when they disagree with it — a
    /// trending tier below `tier` is a recent regression, which is exactly the
    /// thing worth knowing before you tune launch options.
    pub trending: String,
    pub best: String,
}

/// `https://www.protondb.com/app/<appid>` — the human page with community tips.
pub fn page_url(appid: u32) -> String {
    format!("https://www.protondb.com/app/{appid}")
}

/// Tiers move slowly — a week-old summary is still the right answer to "how
/// does this run", and the library looks one up per visible tile.
const TTL: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);
const CACHE: &str = "protondb.json";
/// Serializes the cache file's read-modify-write: tile lookups run
/// concurrently, and two interleaved writers would drop each other's entries.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Cached {
    fetched: u64,
    tier: Tier,
}

/// A game's ProtonDB summary, from the on-disk cache when it is younger than
/// [`TTL`], else fetched (and then cached). Errors are never cached, so a
/// network blip is retried on the next look.
pub fn fetch_blocking(appid: u32) -> Result<Tier, String> {
    let now = crate::fsutil::unix_ts();
    let key = appid.to_string();
    {
        let _g = CACHE_LOCK.lock();
        let map = read_cache();
        if let Some(c) = map.get(&key) {
            if c.fetched <= now && now - c.fetched < TTL.as_secs() {
                return Ok(c.tier.clone());
            }
        }
    }
    let tier = fetch_network(appid)?;
    let _g = CACHE_LOCK.lock();
    let mut map = read_cache();
    // Drop expired entries while here, so the file can't grow forever.
    map.retain(|_, c| c.fetched <= now && now - c.fetched < TTL.as_secs());
    map.insert(key, Cached { fetched: now, tier: tier.clone() });
    // Whole-map TTL is irrelevant here (entries carry their own stamps), so it
    // goes through `disk_cache` with a TTL that never expires the map itself.
    crate::disk_cache::put(CACHE, &map);
    Ok(tier)
}

fn read_cache() -> HashMap<String, Cached> {
    crate::disk_cache::get(CACHE, std::time::Duration::from_secs(u64::MAX / 4)).unwrap_or_default()
}

fn fetch_network(appid: u32) -> Result<Tier, String> {
    let url = format!("https://www.protondb.com/api/v1/reports/summaries/{appid}.json");
    let request = ehttp::Request::get(url);
    match ehttp::fetch_blocking(&request) {
        Ok(resp) if resp.ok => parse(resp.text().unwrap_or("")),
        Ok(resp) => Err(format!("HTTP {} {}", resp.status, resp.status_text)),
        Err(e) => Err(e),
    }
}

fn parse(body: &str) -> Result<Tier, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("unknown").to_string();
    if v.get("tier").is_none() {
        return Err("no ProtonDB reports for this game".to_string());
    }
    Ok(Tier {
        tier: s("tier"),
        trending: s("trendingTier"),
        best: s("bestReportedTier"),
        confidence: s("confidence"),
        total: v.get("total").and_then(|x| x.as_u64()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_summary() {
        let t = parse(
            r#"{"tier":"platinum","total":1234,"confidence":"strong",
                "trendingTier":"gold","bestReportedTier":"platinum"}"#,
        )
        .expect("valid payload");
        assert_eq!(t.tier, "platinum");
        assert_eq!(t.total, 1234);
        assert_eq!(t.confidence, "strong");
        assert_eq!(t.trending, "gold");
        assert_eq!(t.best, "platinum");
    }

    #[test]
    fn missing_tier_is_error() {
        // The API returns a bare object for games with no reports.
        assert!(parse(r#"{"total":0}"#).is_err());
    }

    #[test]
    fn optional_fields_default_to_unknown() {
        let t = parse(r#"{"tier":"gold"}"#).expect("tier alone is enough");
        assert_eq!(t.tier, "gold");
        assert_eq!(t.trending, "unknown");
        assert_eq!(t.best, "unknown");
        assert_eq!(t.confidence, "unknown");
        assert_eq!(t.total, 0);
    }

    #[test]
    fn malformed_json_is_error() {
        assert!(parse("<html>404</html>").is_err());
    }
}
