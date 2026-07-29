//! Enumerate installed Steam games and non-Steam shortcuts (read-only).

use std::collections::HashSet;

use steamlocate::app::StateFlag;
use steamlocate::SteamDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSource {
    Steam,
    NonSteam,
}

impl GameSource {
    pub fn label(&self) -> &'static str {
        match self {
            GameSource::Steam => "steam",
            GameSource::NonSteam => "non-steam",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub app_id: u32,
    pub name: String,
    pub source: GameSource,
    /// Target executable (non-Steam shortcuts only) — used to prefill umu mode.
    pub executable: Option<String>,
    /// Whether Steam reports the app as fully installed. Shortcuts point at a
    /// path Steam does not manage, so they are always `true`.
    pub installed: bool,
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
fn dedup_and_sort(mut games: Vec<Game>) -> Vec<Game> {
    let mut seen = HashSet::new();
    games.retain(|g| seen.insert(g.app_id));
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games
}

/// List installed Steam games plus non-Steam shortcuts, sorted by name with
/// runtime/tool apps filtered out.
pub fn list_games(dir: &SteamDir) -> Vec<Game> {
    let mut games = Vec::new();

    // Installed Steam games across all libraries.
    if let Ok(libraries) = dir.libraries() {
        for library in libraries.flatten() {
            for app in library.apps().flatten() {
                let name = app.name.clone().unwrap_or_else(|| app.install_dir.clone());
                if is_tool(app.app_id, &name) {
                    continue;
                }
                // An appmanifest with no state flags tells us nothing; the
                // manifest existing at all is the better guess, so assume
                // installed rather than hiding a game that is really there.
                let installed = app.state_flags.map_or(true, |f| {
                    f.flags().any(|s| s == StateFlag::FullyInstalled)
                });
                games.push(Game {
                    app_id: app.app_id,
                    name,
                    source: GameSource::Steam,
                    executable: None,
                    installed,
                });
            }
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
            });
        }
    }

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
        }
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
