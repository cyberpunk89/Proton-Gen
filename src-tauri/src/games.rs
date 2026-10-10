//! Enumerate installed Steam games and non-Steam shortcuts (read-only).

use std::collections::HashSet;
use std::path::PathBuf;

use steamlocate::app::StateFlag;
use steamlocate::SteamDir;

use crate::params::ConfigWarning;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameSource {
    Steam,
    NonSteam,
    /// A game discovered from Heroic (see [`crate::heroic`]) — sideloaded, or
    /// installed through one of Heroic's native stores (Epic, GOG, Amazon).
    Heroic,
    /// A game in Nexus's library (see [`crate::nexus`]), standing in for the
    /// Steam shortcut and Heroic sideload Nexus mirrors it into.
    Nexus,
}

impl GameSource {
    pub fn label(&self) -> &'static str {
        match self {
            GameSource::Steam => "steam",
            GameSource::NonSteam => "non-steam",
            GameSource::Heroic => "heroic",
            GameSource::Nexus => "nexus",
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
    /// Whether the game is really there: the launcher reports it installed
    /// *and* its folder / executable still exists on disk (see [`on_disk`]).
    /// `false` puts it on the library's "Not installed" shelf, never hides it.
    pub installed: bool,
    /// Heroic's per-game id (base62 `app_name`), the key to its `GamesConfig`
    /// file. `Some` only for [`GameSource::Heroic`]; the inject command needs it.
    pub heroic_id: Option<String>,
    /// Absolute install directory, when resolvable: `steamapps/common/<dir>`
    /// for a Steam library game (via `steamlocate`'s `resolve_app_dir`), or the
    /// parent of `executable` for a non-Steam shortcut / Heroic game. `None`
    /// when there's nothing to resolve it from (e.g. a shortcut with a blank
    /// target). Used only by the OptiScaler-upgrade feature to find/write
    /// files in the game's folder — nothing else in the read-only discovery
    /// path needs it.
    pub install_dir: Option<PathBuf>,
    /// Box art hint (`file://` path or remote URL) for [`GameSource::Heroic`]
    /// games — see [`crate::heroic::HeroicGame::art`]. `None` for every other
    /// source: Steam games and non-Steam shortcuts resolve art by `app_id`
    /// alone (`art::fetch`'s local-cache/CDN lookup), so they need no hint.
    pub art_url: Option<String>,
    /// Nexus-only extras (slug, prefix, pinned Proton, absorbed mirrors);
    /// `Some` exactly when `source` is [`GameSource::Nexus`].
    pub nexus: Option<crate::nexus::NexusInfo>,
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
pub(crate) fn heroic_app_id(app_name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for b in app_name.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash | 0x8000_0000
}

/// Whether what a game points at is still on disk. A launcher's own
/// "installed" record outlives a folder deleted by hand, a library drive that
/// was swapped out, or a repack moved somewhere else — nothing re-checks it.
///
/// Only an absolute path counts as evidence. No path, or a command line
/// `shortcut_executable` couldn't reduce to one, is assumed present, and so is
/// a path we merely can't read (permissions): a game is never shelved on a
/// guess. Read-only — a single `stat`.
fn on_disk(g: &Game) -> bool {
    let target = match g.source {
        GameSource::Steam => g.install_dir.clone(),
        GameSource::NonSteam | GameSource::Heroic | GameSource::Nexus => {
            g.executable.as_deref().map(PathBuf::from)
        }
    };
    match target.filter(|p| p.is_absolute()) {
        None => true,
        Some(p) => !matches!(std::fs::metadata(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
    }
}

/// `g` with `installed` cleared when its files are gone (see [`on_disk`]).
pub(crate) fn checked(mut g: Game) -> Game {
    g.installed = g.installed && on_disk(&g);
    g
}

/// Heroic games as [`Game`]s (source [`GameSource::Heroic`]) — both
/// sideloaded exes and titles installed through Heroic's native stores
/// (Epic/GOG/Amazon). Independent of any Steam install; empty when Heroic
/// isn't present.
pub fn list_heroic_games() -> Vec<Game> {
    crate::heroic::list_sideloaded()
        .into_iter()
        .chain(crate::heroic::list_installed_native())
        .map(|h| Game {
            app_id: heroic_app_id(&h.app_name),
            name: h.title,
            source: GameSource::Heroic,
            install_dir: parent_of(&h.executable),
            executable: h.executable,
            installed: h.installed,
            heroic_id: Some(h.app_name),
            art_url: h.art,
            nexus: None,
        })
        .map(checked)
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
        out.push(checked(Game {
            app_id: app.app_id,
            name,
            source: GameSource::Steam,
            executable: None,
            installed,
            heroic_id: None,
            install_dir: Some(library.resolve_app_dir(&app)),
            art_url: None,
            nexus: None,
        }));
    }
    out.len() - before
}

/// The target a non-Steam shortcut's `Exe` field points at, or `None` when it
/// names no single Windows executable.
///
/// Steam stores `Exe` as the user typed it: usually one quoted path, sometimes a
/// command line. `trim_matches('"')` used to turn `bash "/x/launch.sh"` into
/// `bash "/x/launch.sh` — an unbalanced quote that then prefilled umu mode and
/// gave OptiScaler a bogus install folder. A command line only yields a target
/// when one of its words is a `.exe`; otherwise there is nothing umu could run.
fn shortcut_executable(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"'));
    let exe = match inner {
        // One quoted token: the common case, `"/games/My Game/game.exe"`.
        Some(inner) if !inner.contains('"') => inner.to_string(),
        // No quotes at all: a bare path, possibly with spaces in it.
        _ if !raw.contains('"') && !raw.contains('\'') => raw.to_string(),
        _ => crate::parser::tokenize(raw)
            .into_iter()
            .rev()
            .find(|w| w.to_ascii_lowercase().ends_with(".exe"))?,
    };
    (!exe.is_empty()).then_some(exe)
}

/// The parent directory of an executable path, for sources (non-Steam
/// shortcuts, Heroic) that only ever hand us a target exe, not a library
/// folder `steamlocate` can resolve.
fn parent_of(executable: &Option<String>) -> Option<PathBuf> {
    executable.as_deref().map(std::path::Path::new).and_then(|p| p.parent()).map(PathBuf::from)
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
    // Heroic's store caches (legendary_library.json alone can be over a MB)
    // and Nexus's games.json don't depend on Steam: read them while the Steam
    // libraries are walked.
    std::thread::scope(|s| {
        let heroic = s.spawn(list_heroic_games);
        let nexus = s.spawn(crate::nexus::list);
        let mut games = list_steam_side(dir, extra_libraries, warn);
        // Heroic games (sideloaded + native-store installs) — independent of
        // Steam, but folded in here so `--list`/`dump()` shows them and they
        // go through the same dedup + sort.
        games.extend(heroic.join().expect("heroic scan panicked"));
        // Nexus's repacks replace their own Steam-shortcut and Heroic mirrors.
        dedup_and_sort(crate::nexus::absorb(games, nexus.join().expect("nexus scan panicked")))
    })
}

/// Installed Steam games across every library, plus non-Steam shortcuts.
fn list_steam_side(
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
            let executable = shortcut_executable(&sc.executable);
            games.push(checked(Game {
                app_id: sc.app_id,
                name: sc.app_name.clone(),
                source: GameSource::NonSteam,
                install_dir: parent_of(&executable),
                executable,
                installed: true,
                heroic_id: None,
                art_url: None,
                nexus: None,
            }));
        }
    }

    games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_executable_unwraps_one_quoted_path() {
        assert_eq!(
            shortcut_executable("\"/games/My Game/game.exe\"").as_deref(),
            Some("/games/My Game/game.exe")
        );
        assert_eq!(shortcut_executable("/games/game.exe").as_deref(), Some("/games/game.exe"));
        // Unquoted with spaces: still one path, not a command line.
        assert_eq!(
            shortcut_executable("  /games/My Game/game.exe ").as_deref(),
            Some("/games/My Game/game.exe")
        );
    }

    #[test]
    fn shortcut_executable_never_returns_an_unbalanced_quote() {
        // The real case: a FitGirl launch script run through bash. There is no
        // Windows exe for umu to run, so there is nothing to prefill.
        assert_eq!(shortcut_executable("bash \"/home/u/Games/cd/launch.sh\""), None);
        // A command line that does name an exe yields that exe.
        assert_eq!(
            shortcut_executable("wine \"/g/My Game/x.exe\" -w").as_deref(),
            Some("/g/My Game/x.exe")
        );
        assert_eq!(shortcut_executable(""), None);
        assert_eq!(shortcut_executable("\"\""), None);
    }

    fn steam(app_id: u32, name: &str) -> Game {
        Game {
            app_id,
            name: name.to_string(),
            source: GameSource::Steam,
            executable: None,
            installed: true,
            heroic_id: None,
            install_dir: None,
            art_url: None,
            nexus: None,
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

    #[test]
    fn on_disk_shelves_only_a_missing_absolute_path() {
        let root = std::env::temp_dir().join(format!("protongen-on-disk-{}", std::process::id()));
        let game_dir = root.join("common/Present Game");
        std::fs::create_dir_all(&game_dir).unwrap();
        let exe = game_dir.join("game.exe");
        std::fs::write(&exe, b"").unwrap();

        let mut present = steam(1, "Present");
        present.install_dir = Some(game_dir.clone());
        assert!(checked(present).installed);

        // Manifest still says installed, folder deleted by hand.
        let mut gone = steam(2, "Gone");
        gone.install_dir = Some(root.join("common/Deleted Game"));
        assert!(!checked(gone).installed);

        let shortcut = |exe: Option<String>| Game {
            source: GameSource::NonSteam,
            executable: exe,
            ..steam(3, "Shortcut")
        };
        assert!(checked(shortcut(Some(exe.display().to_string()))).installed);
        assert!(!checked(shortcut(Some(root.join("moved/game.exe").display().to_string()))).installed);
        // Nothing checkable is never evidence of absence.
        assert!(checked(shortcut(None)).installed);
        assert!(checked(shortcut(Some("game.exe".to_string()))).installed);

        // A launcher that already says "not installed" stays that way.
        let mut flagged = steam(4, "Downloading");
        flagged.installed = false;
        flagged.install_dir = Some(game_dir);
        assert!(!checked(flagged).installed);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn parent_of_resolves_a_shortcut_or_heroic_executable() {
        assert_eq!(
            parent_of(&Some("/games/HELLDIVERS 2/bin/game.exe".to_string())),
            Some(PathBuf::from("/games/HELLDIVERS 2/bin"))
        );
        assert_eq!(parent_of(&None), None);
        // A bare filename with no parent component (e.g. a relative path a
        // shortcut stored oddly) has no directory to resolve to, not "/".
        assert_eq!(parent_of(&Some(String::new())), None);
    }
}
