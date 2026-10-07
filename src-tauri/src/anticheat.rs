//! Does this game's anti-cheat work on Linux? From AreWeAntiCheatYet's
//! community list (`games.json` in its GitHub repo), opt-in like ProtonDB.
//!
//! One ~500 KB download covers every game, so it is cached on disk for a few
//! days and parsed once per process; a failed refresh falls back to the stale
//! copy rather than to nothing. Steam games match by appid; everything else
//! (Heroic, Nexus repacks, shortcuts) by normalized title.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

const URL: &str =
    "https://raw.githubusercontent.com/AreWeAntiCheatYet/AreWeAntiCheatYet/HEAD/games.json";
/// How long a downloaded list is used before asking again.
const TTL_SECS: u64 = 3 * 24 * 60 * 60;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
/// Far above the real list's size; anything bigger isn't it.
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// One game's entry, as the frontend shows it.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct AntiCheat {
    pub name: String,
    /// `Supported` (works, enabled by the developer), `Running` (works),
    /// `Planned`, `Broken`, or `Denied` (the developer blocks Linux).
    pub status: String,
    /// e.g. `Easy Anti-Cheat`, `BattlEye`.
    pub anticheats: Vec<String>,
    /// The game's page on areweanticheatyet.com.
    pub url: String,
    /// When the entry last changed (ISO date), for "as of".
    pub updated: Option<String>,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    anticheats: Vec<String>,
    #[serde(default, rename = "storeIds")]
    store_ids: HashMap<String, serde_json::Value>,
    #[serde(default)]
    slug: String,
    #[serde(default, rename = "dateChanged")]
    date_changed: Option<String>,
}

#[derive(Default)]
pub struct Db {
    by_steam: HashMap<u32, AntiCheat>,
    by_name: HashMap<String, AntiCheat>,
}

/// Lowercased, ASCII-alphanumeric words — the same loose match the library
/// uses (`normalizeGameName`), minus diacritic folding, which titles in this
/// list don't need.
pub fn normalize(name: &str) -> String {
    name.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse(text: &str) -> Result<Db, String> {
    let raws: Vec<Raw> = serde_json::from_str(text).map_err(|e| format!("anti-cheat list: {e}"))?;
    let mut db = Db::default();
    for r in raws {
        if r.name.trim().is_empty() || r.anticheats.is_empty() {
            continue;
        }
        let entry = AntiCheat {
            url: if r.slug.is_empty() {
                "https://areweanticheatyet.com".into()
            } else {
                format!("https://areweanticheatyet.com/game/{}", r.slug)
            },
            name: r.name.clone(),
            status: r.status,
            anticheats: r.anticheats,
            updated: r.date_changed.map(|d| d.chars().take(10).collect()),
        };
        let steam = r.store_ids.get("steam").and_then(|v| match v {
            serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
            serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
            _ => None,
        });
        if let Some(id) = steam {
            db.by_steam.insert(id, entry.clone());
        }
        db.by_name.entry(normalize(&r.name)).or_insert(entry);
    }
    Ok(db)
}

impl Db {
    /// `app_id` only counts for a real Steam game; anything else matches by
    /// title, since its id is protongen's own.
    pub fn lookup(&self, source: &str, app_id: u32, name: &str) -> Option<AntiCheat> {
        if source == "steam" {
            if let Some(e) = self.by_steam.get(&app_id) {
                return Some(e.clone());
            }
        }
        self.by_name.get(&normalize(name)).cloned()
    }
}

fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("protongen/anticheat.json"))
}

fn fetch() -> Result<String, String> {
    let req = ehttp::Request::get(URL).with_timeout(Some(TIMEOUT));
    let resp = ehttp::fetch_blocking(&req)?;
    if !resp.ok {
        return Err(format!("HTTP {} {}", resp.status, resp.status_text));
    }
    if resp.bytes.len() > MAX_BYTES {
        return Err("anti-cheat list is unexpectedly large".into());
    }
    String::from_utf8(resp.bytes).map_err(|e| e.to_string())
}

static LOADED: Mutex<Option<(u64, Arc<Db>)>> = Mutex::new(None);

/// The list: in memory if fresh, else the disk cache if fresh, else
/// downloaded (and cached). A failed download falls back to any cached copy.
pub fn db() -> Result<Arc<Db>, String> {
    let now = crate::fsutil::unix_ts();
    if let Some((at, db)) = LOADED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() {
        if now.saturating_sub(*at) < TTL_SECS {
            return Ok(Arc::clone(db));
        }
    }
    let cache = cache_path();
    let cached_at = cache
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    let fresh_on_disk = cached_at.is_some_and(|at| now.saturating_sub(at) < TTL_SECS);

    let text = if fresh_on_disk {
        cache.as_ref().and_then(|p| std::fs::read_to_string(p).ok())
    } else {
        None
    };
    let (text, at) = match text {
        Some(t) => (t, cached_at.unwrap_or(now)),
        None => match fetch() {
            Ok(t) => {
                // Only cache what parses, so a garbage response can't stick.
                if parse(&t).is_ok() {
                    if let Some(p) = &cache {
                        let _ = crate::fsutil::write_atomic(p, t.as_bytes());
                    }
                }
                (t, now)
            }
            Err(e) => match cache.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
                Some(stale) => (stale, cached_at.unwrap_or(now)),
                None => return Err(e),
            },
        },
    };
    let db = Arc::new(parse(&text)?);
    *LOADED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((at, Arc::clone(&db)));
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"name": "Halo: The Master Chief Collection", "status": "Supported",
       "anticheats": ["Easy Anti-Cheat"], "storeIds": {"steam": "976730"},
       "slug": "halo-the-master-chief-collection", "dateChanged": "2024-01-19T04:40:56.000Z"},
      {"name": "Fortnite", "status": "Denied", "anticheats": ["Easy Anti-Cheat", "BattlEye"],
       "storeIds": {"epic": "fn"}, "slug": "fortnite"},
      {"name": "No AC Here", "status": "Running", "anticheats": [], "storeIds": {"steam": "1"}},
      {"name": "Numeric Id", "status": "Running", "anticheats": ["BattlEye"], "storeIds": {"steam": 42}}
    ]"#;

    #[test]
    fn steam_games_match_by_appid_others_by_title() {
        let db = parse(SAMPLE).unwrap();
        let halo = db.lookup("steam", 976730, "whatever").unwrap();
        assert_eq!(halo.status, "Supported");
        assert_eq!(halo.url, "https://areweanticheatyet.com/game/halo-the-master-chief-collection");
        assert_eq!(halo.updated.as_deref(), Some("2024-01-19"));

        let fn_ = db.lookup("heroic", 0x8000_0001, "FORTNITE").unwrap();
        assert_eq!(fn_.status, "Denied");
        assert_eq!(fn_.anticheats, vec!["Easy Anti-Cheat", "BattlEye"]);
        // A Steam game whose appid isn't listed can still match by title.
        assert!(db.lookup("steam", 5, "Fortnite").is_some());
        assert_eq!(db.lookup("steam", 42, "").unwrap().name, "Numeric Id");
        // An entry with no anti-cheat isn't an anti-cheat entry.
        assert!(db.lookup("steam", 1, "No AC Here").is_none());
        assert!(db.lookup("nexus", 9, "Some Repack").is_none());
    }

    #[test]
    fn normalize_is_loose() {
        assert_eq!(normalize("Halo: The Master Chief Collection"), "halo the master chief collection");
        assert_eq!(normalize("  ELDEN RING™ "), "elden ring");
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(parse("{}").is_err());
        assert!(parse("").is_err());
    }

    /// Hits the network; run with `cargo test -- --ignored live_list`.
    #[test]
    #[ignore]
    fn live_list_parses_and_knows_a_famous_game() {
        let text = fetch().expect("download");
        let db = parse(&text).expect("parse");
        assert!(db.by_steam.len() > 300, "{} steam entries", db.by_steam.len());
        assert!(db.lookup("steam", 976730, "").is_some(), "Halo MCC");
    }
}
