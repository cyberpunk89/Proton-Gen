//! Enumerate installed Steam games and non-Steam shortcuts (read-only).

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
                games.push(Game {
                    app_id: app.app_id,
                    name,
                    source: GameSource::Steam,
                    executable: None,
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
            });
        }
    }

    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    games.dedup_by_key(|g| g.app_id);
    games
}
