//! Read-only discovery for lsfg-vk — Lossless Scaling's frame generation as a
//! Vulkan layer on Linux (<https://lsfg-vk.dev>).
//!
//! lsfg-vk is driven entirely by environment variables and its own
//! `conf.toml`, so the launch-string side is plain catalog data (`LSFGVK_*` in
//! `params.toml`). What the frontend can't know by itself, and what this module
//! answers, is the state of the user's machine:
//!
//! - is the layer installed at all (an *implicit* Vulkan layer — a manifest in
//!   one of the loader's `implicit_layer.d` directories, not a binary on PATH),
//!   and is it the 2.x layer the `LSFGVK_*` variables belong to;
//! - which profiles the user's `conf.toml` defines, so `LSFGVK_PROFILE` is a
//!   pick from a list rather than a name typed from memory;
//! - where `Lossless.dll` is. lsfg-vk only searches the *default* Steam roots
//!   on its own, so a game library elsewhere (a second drive, a custom
//!   library folder) needs an explicit `LSFGVK_DLL_PATH` in per-game mode. The
//!   app already knows every library, so it can fill that in.
//!
//! Nothing here writes: profiles are edited in lsfg-vk's own `lsfg-vk-ui`.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Lossless Scaling's Steam appid — the DLL lives in its install folder.
pub const LOSSLESS_SCALING_APPID: u32 = 993090;

/// The 2.x layer, configured with `LSFGVK_*` and `conf.toml` `version = 2`.
const LAYER_V2: &str = "VK_LAYER_LSFGVK_frame_generation";
/// The 1.x layer, configured with `LSFG_*`. Recognised only to say "update".
const LAYER_V1: &str = "VK_LAYER_LS_frame_generation";

/// File names lsfg-vk looks for inside a DLL directory, in its own order.
const DLL_NAMES: &[&str] = &["lsfg-vk.dll", "Lossless.dll", "LosslessScaling.dll"];

/// The installed layer, from its Vulkan manifest.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LsfgLayer {
    /// Manifest path, shown so "installed where?" has an answer.
    pub manifest: String,
    /// The manifest's `implementation_version` ("2" for lsfg-vk 2.x).
    pub version: String,
    /// True for the 1.x layer, whose variables are `LSFG_*`, not `LSFGVK_*`.
    pub legacy: bool,
}

/// One `[[profile]]` from `conf.toml`. Every setting is optional: a key the
/// user never wrote falls back to lsfg-vk's own default, and the UI says so
/// rather than inventing a value.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LsfgProfile {
    pub name: String,
    /// Executables / process names / path suffixes that auto-activate it.
    pub active_in: Vec<String>,
    pub multiplier: Option<i64>,
    pub flow_scale: Option<f64>,
    pub performance_mode: Option<bool>,
    pub pacing: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct LsfgStatus {
    /// `None` when no lsfg-vk layer manifest is installed anywhere.
    pub layer: Option<LsfgLayer>,
    /// The config file lsfg-vk will read — shown even when absent.
    pub config_path: String,
    pub config_found: bool,
    /// Why the config couldn't be read, when it exists but doesn't parse.
    pub config_error: Option<String>,
    pub profiles: Vec<LsfgProfile>,
    /// A `Lossless.dll` that exists on disk — the configured one if it
    /// resolves, else the one in Lossless Scaling's Steam install folder.
    pub dll: Option<String>,
    /// Where `dll` came from: "config" or "steam".
    pub dll_source: Option<String>,
    /// Whether `dll` sits outside the roots lsfg-vk searches by itself, so
    /// per-game (`LSFGVK_ENV=1`) mode needs an explicit `LSFGVK_DLL_PATH`.
    pub dll_needs_path: bool,
    /// Whether Lossless Scaling's install folder was found through Steam.
    pub steam_install: Option<String>,
    /// lsfg-vk's profile editor, `lsfg-vk-ui`, is on PATH.
    pub ui_installed: bool,
}

/// Probe everything. `steam_install` is Lossless Scaling's install folder as
/// game discovery resolved it (appid [`LOSSLESS_SCALING_APPID`]), if any.
pub fn detect(steam_install: Option<&Path>) -> LsfgStatus {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let layer = find_layer(&layer_dirs());

    let config_path = config_path(home.as_deref());
    let (config_found, config_error, profiles, config_dll) = match std::fs::read_to_string(&config_path)
    {
        Ok(text) => match parse_config(&text) {
            Ok(c) => (true, None, c.profiles, c.dll),
            Err(e) => (true, Some(e), Vec::new(), None),
        },
        Err(_) => (false, None, Vec::new(), None),
    };

    let from_config = config_dll.as_deref().and_then(|d| resolve_dll(Path::new(d)));
    let from_steam = steam_install.and_then(resolve_dll);
    let (dll, dll_source) = match (from_config, from_steam) {
        (Some(p), _) => (Some(p), Some("config")),
        (None, Some(p)) => (Some(p), Some("steam")),
        (None, None) => (None, None),
    };
    let dll_needs_path =
        dll.as_deref().is_some_and(|p| !in_default_search(p, home.as_deref()));

    LsfgStatus {
        layer,
        config_path: config_path.display().to_string(),
        config_found,
        config_error,
        profiles,
        dll: dll.map(|p| p.display().to_string()),
        dll_source: dll_source.map(str::to_string),
        dll_needs_path,
        steam_install: steam_install.map(|p| p.display().to_string()),
        ui_installed: crate::which::is_installed("lsfg-vk-ui"),
    }
}

/// The Vulkan loader's implicit-layer search path on Linux, in its order:
/// config dirs before data dirs, user before system.
fn layer_dirs() -> Vec<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
    let home = var("HOME").map(PathBuf::from);
    let mut roots: Vec<PathBuf> = Vec::new();

    roots.extend(var("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".config"))));
    match var("XDG_CONFIG_DIRS") {
        Some(dirs) => roots.extend(std::env::split_paths(&dirs)),
        None => roots.push(PathBuf::from("/etc/xdg")),
    }
    roots.push(PathBuf::from("/etc"));
    roots.extend(var("XDG_DATA_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".local/share"))));
    match var("XDG_DATA_DIRS") {
        Some(dirs) => roots.extend(std::env::split_paths(&dirs)),
        None => roots.extend([PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]),
    }

    let mut out: Vec<PathBuf> = Vec::new();
    for r in roots {
        let d = r.join("vulkan/implicit_layer.d");
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// The first lsfg-vk layer manifest in `dirs`. The 2.x layer wins over a
/// leftover 1.x manifest wherever each sits — both can be installed at once
/// after an upgrade, and only the 2.x one reads `LSFGVK_*`.
fn find_layer(dirs: &[PathBuf]) -> Option<LsfgLayer> {
    let mut legacy = None;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        // Deterministic, and the 64-bit manifest before its `.x86` twin.
        paths.sort();
        for path in paths {
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Some(layer) = read_manifest(&path) else { continue };
            if !layer.legacy {
                return Some(layer);
            }
            legacy.get_or_insert(layer);
        }
    }
    legacy
}

fn read_manifest(path: &Path) -> Option<LsfgLayer> {
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let layer = json.get("layer")?;
    let name = layer.get("name")?.as_str()?;
    let legacy = match name {
        LAYER_V2 => false,
        LAYER_V1 => true,
        _ => return None,
    };
    let version = match layer.get("implementation_version") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    };
    Some(LsfgLayer { manifest: path.display().to_string(), version, legacy })
}

/// `$XDG_CONFIG_HOME/lsfg-vk/conf.toml`, falling back to the system-wide
/// `/etc/lsfg-vk/conf.toml` only when the user has none — lsfg-vk's own order.
fn config_path(home: Option<&Path>) -> PathBuf {
    let user = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".config")))
        .map(|d| d.join("lsfg-vk/conf.toml"));
    let system = PathBuf::from("/etc/lsfg-vk/conf.toml");
    match user {
        Some(u) if u.exists() || !system.exists() => u,
        _ => system,
    }
}

struct ParsedConfig {
    dll: Option<String>,
    profiles: Vec<LsfgProfile>,
}

/// Parse `conf.toml` leniently: a profile with an odd field keeps the fields
/// that do parse, because the point is to list profiles by name — refusing the
/// whole file over one typo would hide all of them.
fn parse_config(text: &str) -> Result<ParsedConfig, String> {
    let root: toml::Table = text.parse().map_err(|e: toml::de::Error| e.message().to_string())?;
    let dll = root
        .get("global")
        .and_then(|g| g.get("dll"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let profiles = root
        .get("profile")
        .and_then(|p| p.as_array())
        .map(|arr| arr.iter().filter_map(|p| p.as_table()).filter_map(parse_profile).collect())
        .unwrap_or_default();

    Ok(ParsedConfig { dll, profiles })
}

fn parse_profile(t: &toml::Table) -> Option<LsfgProfile> {
    let name = t.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    // `active_in` is a string or an array of strings.
    let active_in = match t.get("active_in") {
        Some(toml::Value::String(s)) => vec![s.clone()],
        Some(toml::Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    let flow_scale = match t.get("flow_scale") {
        Some(toml::Value::Float(f)) => Some(*f),
        Some(toml::Value::Integer(i)) => Some(*i as f64),
        _ => None,
    };
    Some(LsfgProfile {
        name: name.to_string(),
        active_in,
        multiplier: t.get("multiplier").and_then(|v| v.as_integer()),
        flow_scale,
        performance_mode: t.get("performance_mode").and_then(|v| v.as_bool()),
        // The docs call it `pacing`; lsfg-vk-ui writes `pacing_mode`.
        pacing: t
            .get("pacing_mode")
            .or_else(|| t.get("pacing"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

/// A configured DLL path to the file it means: a file as-is, a directory
/// searched for lsfg-vk's known file names. `None` when nothing exists.
fn resolve_dll(p: &Path) -> Option<PathBuf> {
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    if p.is_dir() {
        return DLL_NAMES.iter().map(|n| p.join(n)).find(|c| c.is_file());
    }
    None
}

/// Whether lsfg-vk would find `dll` without being told: it searches only the
/// standard Steam roots' `steamapps/common/Lossless Scaling` folders.
fn in_default_search(dll: &Path, home: Option<&Path>) -> bool {
    let Some(home) = home else { return false };
    let Some(dir) = dll.parent() else { return false };
    let Ok(dir) = dir.canonicalize() else { return false };
    [
        ".steam/steam/steamapps/common",
        ".steam/debian-installation/steamapps/common",
        ".local/share/Steam/steamapps/common",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common",
        "snap/steam/common/.local/share/Steam/steamapps/common",
    ]
    .iter()
    .filter_map(|root| home.join(root).join("Lossless Scaling").canonicalize().ok())
    .any(|d| d == dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("protongen-lsfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const CONF: &str = r#"
version = 2

[global]
allow_fp16 = true
dll = "/games/steamapps/common/Lossless Scaling/"

[[profile]]
active_in = [ "vkcube", "vkcubepp" ]
flow_scale = 0.85
multiplier = 4
name = "4x FG / 85% [Performance]"
performance_mode = true
pacing_mode = "vsync"

[[profile]]
active_in = "GenshinImpact.exe"
flow_scale = 1
multiplier = 2
name = "2x FG / 100%"

[[profile]]
name = "default"
multiplier = "oops"
"#;

    #[test]
    fn parses_profiles_leniently() {
        let c = parse_config(CONF).unwrap();
        assert_eq!(c.dll.as_deref(), Some("/games/steamapps/common/Lossless Scaling/"));
        assert_eq!(c.profiles.len(), 3);

        let p = &c.profiles[0];
        assert_eq!(p.name, "4x FG / 85% [Performance]");
        assert_eq!(p.active_in, ["vkcube", "vkcubepp"]);
        assert_eq!(p.multiplier, Some(4));
        assert_eq!(p.flow_scale, Some(0.85));
        assert_eq!(p.performance_mode, Some(true));
        assert_eq!(p.pacing.as_deref(), Some("vsync"));

        // A bare string `active_in`, and an integer flow scale.
        assert_eq!(c.profiles[1].active_in, ["GenshinImpact.exe"]);
        assert_eq!(c.profiles[1].flow_scale, Some(1.0));

        // A mistyped field is dropped, not the profile.
        assert_eq!(c.profiles[2].name, "default");
        assert_eq!(c.profiles[2].multiplier, None);
        assert!(c.profiles[2].active_in.is_empty());
    }

    #[test]
    fn broken_toml_is_an_error_not_a_panic() {
        assert!(parse_config("[[profile]\nname = ").is_err());
        // An empty file is a valid config with nothing in it.
        let c = parse_config("").unwrap();
        assert!(c.profiles.is_empty() && c.dll.is_none());
    }

    #[test]
    fn finds_the_v2_layer_over_a_leftover_v1() {
        let d = scratch("layers");
        let (a, b) = (d.join("a"), d.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(
            a.join("VkLayer_LS_frame_generation.json"),
            r#"{"layer":{"name":"VK_LAYER_LS_frame_generation","implementation_version":"1"}}"#,
        )
        .unwrap();
        std::fs::write(a.join("other.json"), r#"{"layer":{"name":"VK_LAYER_MANGOHUD"}}"#).unwrap();
        std::fs::write(a.join("broken.json"), "{").unwrap();
        std::fs::write(
            b.join("VkLayer_LSFGVK_frame_generation.json"),
            r#"{"layer":{"name":"VK_LAYER_LSFGVK_frame_generation","implementation_version":"2"}}"#,
        )
        .unwrap();

        let layer = find_layer(&[a.clone(), b.clone()]).expect("found");
        assert!(!layer.legacy);
        assert_eq!(layer.version, "2");

        // Only the old one: reported, flagged legacy.
        let old = find_layer(&[a]).expect("found");
        assert!(old.legacy);

        assert!(find_layer(&[d.join("missing")]).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn resolves_a_dll_directory_or_file() {
        let d = scratch("dll");
        assert!(resolve_dll(&d).is_none());
        std::fs::write(d.join("Lossless.dll"), b"MZ").unwrap();
        assert_eq!(resolve_dll(&d), Some(d.join("Lossless.dll")));
        assert_eq!(resolve_dll(&d.join("Lossless.dll")), Some(d.join("Lossless.dll")));
        assert!(resolve_dll(&d.join("nope.dll")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_custom_library_needs_an_explicit_dll_path() {
        let home = scratch("home");
        let default = home.join(".local/share/Steam/steamapps/common/Lossless Scaling");
        let custom = home.join("Games/SteamLibrary/steamapps/common/Lossless Scaling");
        for d in [&default, &custom] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("Lossless.dll"), b"MZ").unwrap();
        }
        assert!(in_default_search(&default.join("Lossless.dll"), Some(&home)));
        assert!(!in_default_search(&custom.join("Lossless.dll"), Some(&home)));
        let _ = std::fs::remove_dir_all(&home);
    }
}
