//! What a game ships, read off its install folder: bundled upscaler DLLs
//! (DLSS, FSR, XeSS), kernel anti-cheat, and the engine. Read-only — names
//! and directory structure only, no file is opened.
//!
//! This is what turns generic recipes into per-game suggestions ("this game
//! ships DLSS → the DLSS→FSR 4 profile applies"; "it ships EasyAntiCheat →
//! don't offer gplasync"), so it reports facts as plain [`GameScan::tags`]
//! that `recipes.toml`'s `when = [...]` can match without a Rust change.
//!
//! Bounded: at most [`MAX_DEPTH`] levels and [`MAX_ENTRIES`] directory
//! entries, so a 150 GB install with a deep asset tree costs milliseconds,
//! not a full walk. The DLLs and markers this looks for sit near the top (the
//! exe's own folder, `Binaries/Win64`, `EasyAntiCheat/`).

use std::path::{Path, PathBuf};

use serde::Serialize;

const MAX_DEPTH: usize = 5;
const MAX_ENTRIES: usize = 20_000;

/// Asset-only folders that never hold the files looked for here, skipped so
/// the entry budget goes to the ones that do.
const SKIP_DIRS: &[&str] = &["content", "paks", "movies", "media", "streamingassets", "data", "localization"];

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct GameScan {
    /// Upscalers / frame generators the game bundles, for display:
    /// "DLSS", "DLSS Frame Gen", "FSR", "XeSS".
    pub upscalers: Vec<String>,
    /// Kernel anti-cheat it ships: "EasyAntiCheat", "BattlEye",
    /// "nProtect GameGuard".
    pub anticheat: Vec<String>,
    /// "Unreal Engine" | "Unity" | "RE Engine", when recognisable.
    pub engine: Option<String>,
    /// The folder the real game exe lives in when it isn't the install root
    /// (Unreal's `<Project>/Binaries/Win64`) — where DLL overrides and
    /// OptiScaler have to go.
    pub exe_dir: Option<String>,
    /// Machine tags for recipe matching (`recipes.toml` `when`): `dlss`,
    /// `dlss-fg`, `fsr`, `xess`, `anticheat`, `eac`, `battleye`, `unreal`,
    /// `unity`, `re-engine`.
    pub tags: Vec<String>,
}

#[derive(Default)]
struct Found {
    dlss: bool,
    dlss_fg: bool,
    fsr: bool,
    xess: bool,
    eac: bool,
    battleye: bool,
    gameguard: bool,
    unreal: bool,
    unity: bool,
    re_engine: bool,
    shipping_exe_dir: Option<PathBuf>,
}

impl Found {
    fn file(&mut self, name: &str, dir: &Path) {
        match name {
            "nvngx_dlss.dll" => self.dlss = true,
            "nvngx_dlssg.dll" => self.dlss_fg = true,
            "libxess.dll" | "libxess_dx11.dll" => self.xess = true,
            "unityplayer.dll" => self.unity = true,
            "re_chunk_000.pak" => self.re_engine = true,
            "start_protected_game.exe" | "easyanticheat_x64.dll" | "easyanticheat_eos_setup.exe" => {
                self.eac = true
            }
            "beclient_x64.dll" | "beservice_x64.exe" => self.battleye = true,
            _ if name.starts_with("ffx_fsr") || name.starts_with("amd_fidelityfx_") => self.fsr = true,
            // Unreal's layout: `<Project>/Binaries/Win64/<Game>[-Win64-Shipping].exe`.
            // The `-Shipping` suffix is common but not universal (Jedi:
            // Survivor ships `JediSurvivor.exe`), so the folder shape counts too.
            _ if name.ends_with(".exe") && (name.ends_with("-win64-shipping.exe") || is_binaries_win64(dir)) => {
                self.unreal = true;
                self.shipping_exe_dir.get_or_insert_with(|| dir.to_path_buf());
            }
            _ => {}
        }
    }

    fn dir(&mut self, name: &str) {
        match name {
            "easyanticheat" | "easyanticheat_eos" => self.eac = true,
            "battleye" => self.battleye = true,
            "gameguard" => self.gameguard = true,
            _ => {}
        }
    }
}

/// `…/Binaries/Win64`, case-insensitively.
fn is_binaries_win64(dir: &Path) -> bool {
    let name = |p: Option<&Path>| p.and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_ascii_lowercase());
    name(Some(dir)).as_deref() == Some("win64") && name(dir.parent()).as_deref() == Some("binaries")
}

/// Scan `install_dir`. An unreadable or missing folder yields an empty scan.
pub fn scan(install_dir: &Path) -> GameScan {
    let mut found = Found::default();
    let mut budget = MAX_ENTRIES;
    walk(install_dir, 0, &mut found, &mut budget);
    summarize(found, install_dir)
}

fn walk(dir: &Path, depth: usize, found: &mut Found, budget: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        // `file_type` doesn't follow symlinks: a link loop can't recurse.
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            found.dir(&name);
            if depth + 1 < MAX_DEPTH && !SKIP_DIRS.contains(&name.as_str()) {
                subdirs.push(entry.path());
            }
        } else {
            found.file(&name, dir);
        }
    }
    // Files first, then subfolders breadth-ish: the top levels are where the
    // markers are, so they get the budget before any deep asset tree does.
    for sub in subdirs {
        walk(&sub, depth + 1, found, budget);
    }
}

fn summarize(f: Found, root: &Path) -> GameScan {
    let mut s = GameScan::default();
    let mut tag = |t: &str| s.tags.push(t.to_string());
    if f.dlss {
        tag("dlss");
    }
    if f.dlss_fg {
        tag("dlss-fg");
    }
    if f.fsr {
        tag("fsr");
    }
    if f.xess {
        tag("xess");
    }
    if f.eac || f.battleye || f.gameguard {
        tag("anticheat");
    }
    if f.eac {
        tag("eac");
    }
    if f.battleye {
        tag("battleye");
    }
    let engine = if f.re_engine {
        Some(("re-engine", "RE Engine"))
    } else if f.unreal {
        Some(("unreal", "Unreal Engine"))
    } else if f.unity {
        Some(("unity", "Unity"))
    } else {
        None
    };
    if let Some((t, _)) = engine {
        tag(t);
    }
    s.engine = engine.map(|(_, label)| label.to_string());

    for (on, label) in [(f.dlss, "DLSS"), (f.dlss_fg, "DLSS Frame Gen"), (f.fsr, "FSR"), (f.xess, "XeSS")] {
        if on {
            s.upscalers.push(label.to_string());
        }
    }
    for (on, label) in [(f.eac, "EasyAntiCheat"), (f.battleye, "BattlEye"), (f.gameguard, "nProtect GameGuard")] {
        if on {
            s.anticheat.push(label.to_string());
        }
    }
    s.exe_dir = f
        .shipping_exe_dir
        .filter(|d| d != root)
        .map(|d| d.display().to_string());
    s
}

/// Whether a scan's `tags` satisfy a recipe's `when` list: every plain tag
/// present, every `!tag` absent. An empty list matches nothing — `when` is
/// opt-in. The frontend does the matching (`util.ts` `matchesWhen`); this is
/// the reference its semantics are tested against here.
#[cfg(test)]
pub fn matches(when: &[String], tags: &[String]) -> bool {
    !when.is_empty()
        && when.iter().all(|w| match w.strip_prefix('!') {
            Some(neg) => !tags.iter().any(|t| t == neg),
            None => tags.iter().any(|t| t == w),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "protongen-scan-{}-{}",
            std::process::id(),
            files.len() + files.iter().map(|f| f.len()).sum::<usize>()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for f in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            if f.ends_with('/') {
                std::fs::create_dir_all(&p).unwrap();
            } else {
                std::fs::write(&p, b"").unwrap();
            }
        }
        root
    }

    #[test]
    fn finds_unreal_dlss_and_eac_with_the_real_exe_dir() {
        let root = tree(&[
            "Game.exe",
            "EasyAntiCheat/Settings.json",
            "MyGame/Binaries/Win64/MyGame-Win64-Shipping.exe",
            "MyGame/Binaries/Win64/nvngx_dlss.dll",
            "MyGame/Binaries/Win64/nvngx_dlssg.dll",
            "MyGame/Content/Paks/pakchunk0.pak",
        ]);
        let s = scan(&root);
        assert_eq!(s.engine.as_deref(), Some("Unreal Engine"));
        assert_eq!(s.upscalers, vec!["DLSS", "DLSS Frame Gen"]);
        assert_eq!(s.anticheat, vec!["EasyAntiCheat"]);
        assert!(s.exe_dir.unwrap().ends_with("MyGame/Binaries/Win64"));
        for t in ["dlss", "dlss-fg", "anticheat", "eac", "unreal"] {
            assert!(s.tags.iter().any(|x| x == t), "missing {t}: {:?}", s.tags);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn re_engine_wins_and_fsr_xess_are_seen() {
        let root = tree(&["re_chunk_000.pak", "amd_fidelityfx_dx12.dll", "libxess.dll", "BattlEye/"]);
        let s = scan(&root);
        assert_eq!(s.engine.as_deref(), Some("RE Engine"));
        assert_eq!(s.upscalers, vec!["FSR", "XeSS"]);
        assert_eq!(s.anticheat, vec!["BattlEye"]);
        assert_eq!(s.exe_dir, None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unreal_by_folder_shape_and_gameguard() {
        let root = tree(&["SwGame/Binaries/Win64/JediSurvivor.exe", "bin/GameGuard/GameMon.des"]);
        let s = scan(&root);
        assert_eq!(s.engine.as_deref(), Some("Unreal Engine"));
        assert!(s.exe_dir.unwrap().ends_with("SwGame/Binaries/Win64"));
        assert_eq!(s.anticheat, vec!["nProtect GameGuard"]);
        assert!(s.tags.iter().any(|t| t == "anticheat"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_folder_is_an_empty_scan() {
        assert_eq!(scan(Path::new("/nonexistent/protongen/scan")), GameScan::default());
    }

    #[test]
    fn when_needs_every_tag_and_honours_negation() {
        let tags: Vec<String> = ["dlss", "unreal"].map(String::from).to_vec();
        let w = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(matches(&w(&["dlss"]), &tags));
        assert!(matches(&w(&["dlss", "!anticheat"]), &tags));
        assert!(!matches(&w(&["dlss", "!unreal"]), &tags));
        assert!(!matches(&w(&["fsr"]), &tags));
        assert!(!matches(&[], &tags));
    }
}
