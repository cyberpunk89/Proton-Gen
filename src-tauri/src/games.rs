//! Enumerate installed Steam games and non-Steam shortcuts (read-only).

use std::collections::HashSet;

use steamlocate::app::StateFlag;
use steamlocate::SteamDir;

use crate::params::ConfigWarning;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSource {
    Steam,
    NonSteam,
    /// A sideloaded game discovered from Heroic (see [`crate::heroic`]).
    Heroic,
}

impl GameSource {
    pub fn label(&self) -> &'static str {
        match self {
            GameSource::Steam => "steam",
            GameSource::NonSteam => "non-steam",
            GameSource::Heroic => "heroic",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub app_id: u32,
    pub name: String,
    pub source: GameSource,
    /// Target executable (non-Steam shortcuts + Heroic games) — prefills umu mode.
    pub executable: Option<String>,
    /// Whether the launcher reports the app as installed. Non-Steam shortcuts
    /// point at a path Steam does not manage, so they are always `true`.
    pub installed: bool,
    /// Heroic's per-game id (base62 `app_name`), the key to its `GamesConfig`
    /// file. `Some` only for [`GameSource::Heroic`]; the inject command needs it.
    pub heroic_id: Option<String>,
}

/// Well-known non-game app IDs (runtimes / redistributables) to hide.
const HIDDEN_APP_IDS: &[u32] = &[
    228980,  // Steamworks Common Redistributables
    1070560, // Steam Linux Runtime 1.0 (scout)
    1391110, // Steam Linux Runtime 2.0 (soldier)
    1628350, // Steam Linux Runtime 3.0 (sniper)
    1493710, // Proton Experimental (the tool app)
];

/// True if an app is a Proton/runtime tool rather than a real game.
fn is_tool(app_id: u32, name: &str) -> bool {
    if HIDDEN_APP_IDS.contains(&app_id) {
        return true;
    }
    let n = name.to_lowercase();
    n.starts_with("proton")
        || n.contains("steam linux runtime")
        || n.contains("steamworks common")
        || n.contains("proton experimental")
}

/// Drop duplicate app ids, then sort by name.
///
/// Dedup must run **before** the sort: this used to be a `dedup_by_key` on the
/// name-sorted vec, which only collapses *adjacent* equal keys. The same appid
/// registered in two Steam libraries sorts to two entries with the same name but
/// is not guaranteed adjacent (and even when it is, relying on that is luck), so
/// duplicates survived. Cosmetic today; a duplicate key in a keyed Svelte
/// `{#each}` is a crash.
pub(crate) fn dedup_and_sort(mut games: Vec<Game>) -> Vec<Game> {
    let mut seen = HashSet::new();
    games.retain(|g| seen.insert(g.app_id));
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games
}

/// A deterministic synthetic `app_id` for a Heroic game, hashed from its base62
/// `app_name`.
///
/// Determinism is load-bearing: `app_id` is the persistence key for
/// `game_memory`, `favorites`, and `last_game_appid`, so a per-process-seeded
/// hasher (`DefaultHasher`/`RandomState`) would rotate the id every launch and
/// orphan the user's saved tuning. FNV-1a-32 is stable across runs.
///
/// The high bit is forced on, parking Heroic ids above every real Steam appid
/// (all well under 2³¹) — so a hash can't collide with a Steam game and trip
/// `dedup_and_sort`'s silent drop or crash a keyed Svelte `{#each}`.
fn heroic_app_id(app_name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in app_name.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash | 0x8000_0000
}

/// Sideloaded Heroic games as [`Game`]s (source [`GameSource::Heroic`]).
/// Independent of any Steam install; empty when Heroic isn't present.
pub fn list_heroic_games() -> Vec<Game> {
    crate::heroic::list_sideloaded()
        .into_iter()
        .map(|h| Game {
            app_id: heroic_app_id(&h.app_name),
            name: h.title,
            source: GameSource::Heroic,
            executable: h.executable,
            installed: h.installed,
            heroic_id: Some(h.app_name),
        })
        .collect()
}

/// Collect the apps of one library into `out`, skipping runtimes/tools.
/// Returns how many it added, so a configured library that yields nothing can
/// say so.
fn push_library_apps(library: &steamlocate::Library, out: &mut Vec<Game>) -> usize {
    let before = out.len();
    for app in library.apps().flatten() {
        let name = app.name.clone().unwrap_or_else(|| app.install_dir.clone());
        if is_tool(app.app_id, &name) {
            continue;
        }
        // An appmanifest with no state flags tells us nothing; the manifest
        // existing at all is the better guess, so assume installed rather than
        // hiding a game that is really there.
        let installed = app
            .state_flags
            .map_or(true, |f| f.flags().any(|s| s == StateFlag::FullyInstalled));
        out.push(Game {
            app_id: app.app_id,
            name,
            source: GameSource::Steam,
            executable: None,
            installed,
            heroic_id: None,
        });
    }
    out.len() - before
}

/// List installed Steam games plus non-Steam shortcuts, sorted by name with
/// runtime/tool apps filtered out.
///
/// `extra_libraries` (from Settings) are additional library folders, for the
/// case where `libraryfolders.vdf` doesn't mention a drive. A folder already
/// declared there costs nothing to list twice: `dedup_and_sort` keys on appid.
pub fn list_games(
    dir: &SteamDir,
    extra_libraries: &[String],
    warn: &mut Vec<ConfigWarning>,
) -> Vec<Game> {
    let mut games = Vec::new();

    // Installed Steam games across all libraries.
    if let Ok(libraries) = dir.libraries() {
        for library in libraries.flatten() {
            push_library_apps(&library, &mut games);
        }
    }

    for raw in crate::store::Paths::clean(extra_libraries) {
        match steamlocate::Library::from_dir(std::path::Path::new(raw)) {
            Ok(library) => {
                if push_library_apps(&library, &mut games) == 0 {
                    warn.push(ConfigWarning::path(
                        "Steam library",
                        raw,
                        "no installed games here — expected a folder containing steamapps/",
                    ));
                }
            }
            Err(e) => warn.push(ConfigWarning::path("Steam library", raw, e.to_string())),
        }
    }

    // Non-Steam game shortcuts (shortcuts.vdf).
    if let Ok(shortcuts) = dir.shortcuts() {
        for sc in shortcuts.flatten() {
            let exe = sc.executable.trim().trim_matches('"').to_string();
            games.push(Game {
                app_id: sc.app_id,
                name: sc.app_name.clone(),
                source: GameSource::NonSteam,
                executable: if exe.is_empty() { None } else { Some(exe) },
                installed: true,
                heroic_id: None,
            });
        }
    }

    // Sideloaded Heroic games — independent of Steam, but folded in here so
    // `--list`/`dump()` shows them and they go through the same dedup + sort.
    games.extend(list_heroic_games());

    dedup_and_sort(games)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steam(app_id: u32, name: &str) -> Game {
        Game {
            app_id,
            name: name.to_string(),
            source: GameSource::Steam,
            executable: None,
            installed: true,
            heroic_id: None,
        }
    }

    #[test]
    fn heroic_app_id_is_deterministic_and_high() {
        // Same input -> same id across calls (and, since FNV isn't seeded, across
        // process runs), so saved per-game config survives a restart.
        assert_eq!(heroic_app_id("7Hm5qmyaYmaSZ45Mqo3u4s"), heroic_app_id("7Hm5qmyaYmaSZ45Mqo3u4s"));
        assert_ne!(heroic_app_id("abc"), heroic_app_id("abd"));
        // High bit set -> above every real Steam appid.
        assert!(heroic_app_id("anything") >= 0x8000_0000);
    }

    #[test]
    fn dedups_the_same_appid_in_two_libraries() {
        // Two libraries both claim 553850; a third game sorts between the two
        // copies by name, so they are NOT adjacent after the name sort — which
        // is exactly what the old sort-then-`dedup_by_key` missed.
        let games = dedup_and_sort(vec![
            steam(553850, "HELLDIVERS 2"),
            steam(1245620, "ELDEN RING"),
            steam(553850, "HELLDIVERS 2"),
        ]);
        assert_eq!(
            games.iter().map(|g| g.app_id).collect::<Vec<_>>(),
            vec![1245620, 553850]
        );
    }

    #[test]
    fn sorts_by_name_case_insensitively() {
        let games = dedup_and_sort(vec![
            steam(3, "zed"),
            steam(1, "Alpha"),
            steam(2, "beta"),
        ]);
        assert_eq!(
            games.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "beta", "zed"]
        );
    }
}
