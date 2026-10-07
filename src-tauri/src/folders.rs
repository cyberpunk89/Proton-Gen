//! A game's Wine prefix and shader cache: where they are and how big.
//! Read-only — "Open" hands the folder to the desktop's file manager; nothing
//! here deletes or resets anything (Nexus owns prefix housekeeping for its
//! games, and Steam's "Delete compatibility data" for the rest).
//!
//! Paths are always computed here from discovery, never taken from the
//! webview, so "open folder" can only ever open one of these.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::ipc::GameDto;

/// One folder of a game's, with its size once measured.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Folder {
    /// `prefix` or `shadercache`: what `open_game_folder` takes.
    pub kind: &'static str,
    pub path: String,
    pub exists: bool,
    /// Bytes on disk (apparent size), `None` when it doesn't exist.
    pub bytes: Option<u64>,
}

/// `<library>/steamapps` for a Steam game, from its install dir
/// (`<library>/steamapps/common/<dir>`).
fn steamapps_of(install_dir: &str) -> Option<PathBuf> {
    let p = Path::new(install_dir);
    let common = p.parent()?;
    (common.file_name()? == "common").then(|| common.parent().map(Path::to_path_buf))?
}

/// Where `game`'s prefix and shader cache live. `steam_root` is the Steam
/// install, home to the compatdata of non-Steam shortcuts (Nexus's too).
pub fn locate(game: &GameDto, steam_root: Option<&Path>) -> Vec<(&'static str, PathBuf)> {
    let root_apps = steam_root.map(|r| r.join("steamapps"));
    let mut out = Vec::new();
    match game.source.as_str() {
        "steam" => {
            if let Some(apps) = game.install_dir.as_deref().and_then(steamapps_of) {
                out.push(("prefix", apps.join("compatdata").join(game.app_id.to_string()).join("pfx")));
                out.push(("shadercache", apps.join("shadercache").join(game.app_id.to_string())));
            }
        }
        "non-steam" => {
            if let Some(apps) = root_apps {
                out.push(("prefix", apps.join("compatdata").join(game.app_id.to_string()).join("pfx")));
                out.push(("shadercache", apps.join("shadercache").join(game.app_id.to_string())));
            }
        }
        "nexus" => {
            if let Some(pfx) = game.wine_prefix.as_deref() {
                out.push(("prefix", PathBuf::from(pfx)));
            }
            // Played through its Steam shortcut (same appid), Steam keeps a
            // shader cache for it under the root library.
            if let Some(apps) = root_apps {
                out.push(("shadercache", apps.join("shadercache").join(game.app_id.to_string())));
            }
        }
        _ => {}
    }
    out
}

/// Total size of the files under `dir`. Symlinks are not followed (a Proton
/// prefix links into the Proton install, which isn't the game's), and an
/// unreadable entry is skipped rather than failing the whole count.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                total += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

/// [`locate`], stat'ed and measured.
pub fn measure(game: &GameDto, steam_root: Option<&Path>) -> Vec<Folder> {
    locate(game, steam_root)
        .into_iter()
        .map(|(kind, path)| {
            let exists = path.is_dir();
            Folder {
                kind,
                path: path.display().to_string(),
                exists,
                bytes: exists.then(|| dir_size(&path)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(source: &str, app_id: u32, install_dir: Option<&str>, prefix: Option<&str>) -> GameDto {
        GameDto {
            app_id,
            name: "G".into(),
            source: source.into(),
            executable: None,
            installed: true,
            last_played: None,
            playtime_minutes: None,
            heroic_id: None,
            install_dir: install_dir.map(str::to_string),
            art_url: None,
            nexus_slug: None,
            wine_prefix: prefix.map(str::to_string),
            pinned_proton: None,
            alias_ids: vec![],
        }
    }

    #[test]
    fn steam_games_use_their_own_library() {
        let g = game("steam", 1245620, Some("/mnt/ssd/SteamLibrary/steamapps/common/ELDEN RING"), None);
        let f = locate(&g, Some(Path::new("/home/u/.local/share/Steam")));
        assert_eq!(
            f,
            vec![
                ("prefix", PathBuf::from("/mnt/ssd/SteamLibrary/steamapps/compatdata/1245620/pfx")),
                ("shadercache", PathBuf::from("/mnt/ssd/SteamLibrary/steamapps/shadercache/1245620")),
            ]
        );
        assert!(locate(&game("steam", 1, Some("/weird/place"), None), None).is_empty());
    }

    #[test]
    fn shortcuts_and_nexus_games() {
        let root = Path::new("/s");
        let sc = locate(&game("non-steam", 77, None, None), Some(root));
        assert_eq!(sc[0].1, PathBuf::from("/s/steamapps/compatdata/77/pfx"));
        let nx = locate(&game("nexus", 88, None, Some("/g/x/pfx")), Some(root));
        assert_eq!(nx[0], ("prefix", PathBuf::from("/g/x/pfx")));
        assert_eq!(nx[1], ("shadercache", PathBuf::from("/s/steamapps/shadercache/88")));
        assert!(locate(&game("heroic", 9, None, None), Some(root)).is_empty());
    }

    #[test]
    fn dir_size_sums_files_and_skips_symlinks() {
        let d = std::env::temp_dir().join(format!("protongen-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::write(d.join("x"), [0u8; 100]).unwrap();
        std::fs::write(d.join("a/b/y"), [0u8; 50]).unwrap();
        std::os::unix::fs::symlink("/usr", d.join("link")).unwrap();
        assert_eq!(dir_size(&d), 150);
        let m = measure(&game("nexus", 1, None, Some(d.to_str().unwrap())), None);
        assert_eq!(m[0].bytes, Some(150));
        assert!(m[0].exists);
        std::fs::remove_dir_all(&d).ok();
    }
}
