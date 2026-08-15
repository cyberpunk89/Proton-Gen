//! Persisted state: chosen theme, named presets, and per-game memory.
//! Stored as TOML at `$XDG_CONFIG_HOME/protongen/state.toml`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::params::{self, Catalog, Options};

/// A serializable snapshot of the builder state (everything except the chosen game).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub umu: bool,
    #[serde(default)]
    pub runtime: Option<String>, // runtime internal_name
    #[serde(default)]
    pub env: Vec<(String, String)>,
    #[serde(default)]
    pub wrappers: Vec<(String, String)>, // key -> value (gamescope args; "" otherwise)
    #[serde(default)]
    pub extra_env: String,
    #[serde(default)]
    pub umu_exe: String,
    #[serde(default)]
    pub umu_wineprefix: String,
    #[serde(default)]
    pub umu_gameid: String,
    #[serde(default)]
    pub game_args: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    #[serde(default)]
    pub game_appid: Option<u32>,
    #[serde(default)]
    pub game_name: Option<String>,
    pub config: Config,
}

/// User-supplied discovery paths, for systems protongen's built-in candidates
/// don't cover — a non-CachyOS distro, Steam installed somewhere unusual, or
/// tools outside `$PATH`.
///
/// One sub-struct rather than four flat `Store` fields: discovery takes a single
/// parameter, there is one place to document what these are, and `state.toml`
/// gets a legible `[paths]` table — which matters, because hand-editing it is
/// exactly what this audience does.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Paths {
    /// Extra Steam roots, tried *before* the built-in candidates: a user only
    /// adds one because the defaults were wrong, so an explicit choice outranks
    /// a lucky guess.
    #[serde(default)]
    pub steam_roots: Vec<String>,
    /// Extra Steam library folders (each containing `steamapps/`), merged with
    /// the ones `libraryfolders.vdf` declares.
    #[serde(default)]
    pub steam_libraries: Vec<String>,
    /// Extra directories in the `compatibilitytools.d` layout — one sub-folder
    /// per Proton build, each with a `compatibilitytool.vdf`.
    #[serde(default)]
    pub proton_dirs: Vec<String>,
    /// Program overrides keyed by the catalog `requires` name (`gamescope`,
    /// `gamemoderun`, `mangohud`) plus `umu-run`. Blank or absent = the bare
    /// name. A map rather than named fields so `compute_requires_status` can
    /// consult overrides generically, and a `BTreeMap` for the stable, diffable
    /// TOML `game_memory` and `favorites` already cite.
    #[serde(default)]
    pub bins: BTreeMap<String, String>,
}

impl Paths {
    /// Non-empty, trimmed entries. The Settings rows keep blank placeholder
    /// inputs while the user types, so blanks reach the backend routinely.
    pub fn clean(list: &[String]) -> Vec<&str> {
        list.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub presets: Vec<Preset>,
    /// appid (as string, for TOML) -> last-used config.
    #[serde(default)]
    pub game_memory: BTreeMap<String, Config>,
    /// proton-cachyos build for which the "catalog stale" banner was dismissed.
    #[serde(default)]
    pub dismissed_cachyos_build: String,
    /// App version for which the "update available" banner was dismissed.
    #[serde(default)]
    pub dismissed_update_version: String,
    /// Show recipes/options that don't apply to the detected hardware (default: hide).
    #[serde(default)]
    pub show_irrelevant: bool,
    /// User-declared HDR display capability (not auto-detectable; opt-in).
    #[serde(default)]
    pub hdr: bool,
    /// User-declared FSR 3/4 upscaler-upgrade capability (RDNA3/RDNA4; opt-in,
    /// not reliably auto-detectable). Gates the FSR upgrade params/recipes.
    ///
    /// Superseded by [`Self::gpu_gen`] as the UI control, but kept as a legacy
    /// fallback: a `state.toml` written before `gpu_gen` existed still carries
    /// `fsr4 = true`, and the frontend derives the FSR4 capability from either.
    #[serde(default)]
    pub fsr4: bool,
    /// User-declared AMD GPU generation: `""` (unset / not AMD), `"rdna3"`, or
    /// `"rdna4"`. Set via the Settings generation selector. Drives two derived
    /// frontend capabilities: FSR4 (either generation) and RDNA3-only (so the
    /// RDNA3 FSR4 workaround hides on RDNA4). Opt-in, not reliably detectable.
    #[serde(default)]
    pub gpu_gen: String,
    /// Auto-fetch the ProtonDB tier when a Steam game is selected.
    #[serde(default)]
    pub protondb_auto: bool,
    /// Games pinned to the top of the library grid, under every sort. A
    /// `BTreeSet` for the same reason `game_memory` is a `BTreeMap`: stable,
    /// diffable TOML output rather than whatever order the frontend sent.
    #[serde(default)]
    pub favorites: BTreeSet<u32>,
    /// Last-used library sort ("recent" | "alpha" | "tuned"). Persisted as
    /// last-used-wins rather than surfaced as an explicit setting.
    #[serde(default)]
    pub library_sort: String,
    /// The exact builder state on screen when the app last closed, restored on
    /// next launch so the user reopens where they left off.
    #[serde(default)]
    pub last_session: Option<Config>,
    /// The game selected in that last session (matches `GameDto.app_id`), so it
    /// can be re-selected on launch.
    #[serde(default)]
    pub last_game_appid: Option<u32>,
    /// User-supplied discovery paths. See [`Paths`].
    #[serde(default)]
    pub paths: Paths,
    /// A reusable set of selections the user authors in Settings ("set from
    /// current build") and applies to any game on demand via a button. Reuses
    /// [`Config`], so it survives catalog drift the same way presets do. `None`
    /// until the user saves one.
    #[serde(default)]
    pub global_profile: Option<Config>,
}

impl Store {
    pub fn load() -> Self {
        let Some(path) = params::config_file("state.toml") else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Resolve the XDG path and write. The thin shell over [`Self::save_to`].
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = params::config_file("state.toml") else {
            return Err("no config directory available (is $HOME set?)".to_string());
        };
        self.save_to(&path)
    }

    /// Write the store to `path`, creating parent directories as needed.
    ///
    /// Pure enough to test: every error names the path, because the path is
    /// what makes a write failure actionable ("which directory is read-only?").
    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| format!("couldn't serialize settings: {e}"))?;
        std::fs::write(path, text).map_err(|e| format!("couldn't write {}: {e}", path.display()))
    }

    pub fn remember(&mut self, appid: u32, config: Config) {
        self.game_memory.insert(appid.to_string(), config);
    }

    pub fn recall(&self, appid: u32) -> Option<&Config> {
        self.game_memory.get(&appid.to_string())
    }

    pub fn preset_names(&self) -> Vec<String> {
        self.presets.iter().map(|p| p.name.clone()).collect()
    }

    pub fn upsert_preset(&mut self, preset: Preset) {
        if let Some(slot) = self.presets.iter_mut().find(|p| p.name == preset.name) {
            *slot = preset;
        } else {
            self.presets.push(preset);
        }
    }

    pub fn delete_preset(&mut self, name: &str) {
        self.presets.retain(|p| p.name != name);
    }
}

/// Capture the enabled options into (env, wrapper) key/value lists.
pub fn options_to_lists(
    catalog: &Catalog,
    options: &Options,
) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let mut env = Vec::new();
    let mut wrappers = Vec::new();
    for (def, st) in catalog.wrappers.iter().zip(&options.wrappers) {
        if st.enabled {
            wrappers.push((def.key.clone(), st.value.clone()));
        }
    }
    for (def, st) in catalog.envs.iter().zip(&options.envs) {
        if st.enabled {
            env.push((def.key.clone(), st.value.clone()));
        }
    }
    (env, wrappers)
}

/// Build `Options` from catalog defaults plus the enabled (env, wrapper) lists,
/// **returning the env pairs the catalog has no entry for**.
///
/// Those leftovers are the point. `params.toml` is rewritten in place by the
/// `update-proton-params` skill, so a preset or `game_memory` entry written
/// before a rename keeps naming a key the catalog no longer has — `a552034`
/// renamed `PROTON_ENABLE_NVAPI` to `PROTON_DISABLE_NVAPI` and dropped
/// `PROTON_ENABLE_HDR` outright. Those keys used to vanish here (#62): the
/// assembled command silently lost them, the sync verdict was computed against
/// that lossy command, and `ipc::apply_recipe`'s round-trip through
/// [`options_to_lists`] erased them from disk for good. Callers must re-home
/// them — [`crate::compose::merge_into_extra_env`] is the shared way.
///
/// [`Options`] is strictly parallel to the catalog, so there is nowhere inside
/// it to park an unknown key; hence the out-of-band return rather than a flag.
///
/// **Unknown wrapper keys are still dropped, deliberately.** A wrapper is
/// emitted as a program token by the closed 3-variant [`crate::builder::Wrapper`]
/// enum, so an unknown wrapper has no representation in a built command and no
/// field to survive in. It also cannot drift the way env keys do: the skill
/// never touches `[[wrapper]]`, so the only way to lose one is to hand-edit a
/// user `params.toml`, which is the user saying they don't want it.
#[must_use]
pub fn options_from_lists(
    catalog: &Catalog,
    env: &[(String, String)],
    wrappers: &[(String, String)],
) -> (Options, Vec<(String, String)>) {
    let mut options = Options::from_catalog(catalog);
    for (k, v) in wrappers {
        if let Some(i) = catalog.wrappers.iter().position(|w| &w.key == k) {
            options.wrappers[i].enabled = true;
            options.wrappers[i].value = v.clone();
        }
    }
    let mut unknown = Vec::new();
    for (k, v) in env {
        match catalog.envs.iter().position(|e| &e.key == k) {
            Some(i) => {
                options.envs[i].enabled = true;
                options.envs[i].value = v.clone();
            }
            None => unknown.push((k.clone(), v.clone())),
        }
    }
    (options, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_toml_roundtrip() {
        let mut s = Store {
            theme: "Dracula".into(),
            paths: Paths {
                steam_roots: vec!["/mnt/games/Steam".into()],
                steam_libraries: vec!["/mnt/second/SteamLibrary".into()],
                proton_dirs: vec!["/opt/proton-builds".into()],
                bins: BTreeMap::from([("umu-run".to_string(), "/home/u/.local/bin/umu-run".to_string())]),
            },
            ..Default::default()
        };
        s.upsert_preset(Preset {
            name: "hd2".into(),
            game_appid: Some(553850),
            game_name: Some("HELLDIVERS 2".into()),
            config: Config {
                umu: false,
                env: vec![("PROTON_USE_NTSYNC".into(), "1".into())],
                wrappers: vec![("mangohud".into(), String::new())],
                game_args: "-windowed".into(),
                ..Default::default()
            },
        });
        s.remember(553850, s.presets[0].config.clone());
        s.last_game_appid = Some(553850);
        s.last_session = Some(Config {
            umu: true,
            extra_env: "FOO=bar".into(),
            ..Default::default()
        });
        s.favorites.insert(553850);
        s.favorites.insert(275850);
        s.library_sort = "recent".into();
        s.gpu_gen = "rdna4".into();
        s.global_profile = Some(Config {
            umu: false,
            env: vec![("PROTON_USE_NTSYNC".into(), "1".into())],
            wrappers: vec![("gamemoderun".into(), String::new())],
            ..Default::default()
        });
        let text = toml::to_string_pretty(&s).unwrap();
        let back: Store = toml::from_str(&text).unwrap();
        assert_eq!(back.theme, "Dracula");
        assert_eq!(back.gpu_gen, "rdna4");
        let gp = back.global_profile.as_ref().expect("global_profile round-trips");
        assert_eq!(gp.env, vec![("PROTON_USE_NTSYNC".to_string(), "1".to_string())]);
        assert_eq!(gp.wrappers, vec![("gamemoderun".to_string(), String::new())]);
        assert_eq!(back.presets.len(), 1);
        assert_eq!(back.presets[0].name, "hd2");
        assert_eq!(back.recall(553850).unwrap().game_args, "-windowed");
        assert_eq!(back.last_game_appid, Some(553850));
        let sess = back.last_session.expect("last_session round-trips");
        assert!(sess.umu);
        assert_eq!(sess.extra_env, "FOO=bar");
        // A BTreeSet so the TOML is stable and diffable, not insertion-ordered.
        assert_eq!(back.favorites.iter().copied().collect::<Vec<_>>(), vec![275850, 553850]);
        assert_eq!(back.library_sort, "recent");
        assert_eq!(back.paths.steam_roots, vec!["/mnt/games/Steam".to_string()]);
        assert_eq!(back.paths.proton_dirs, vec!["/opt/proton-builds".to_string()]);
        assert_eq!(
            back.paths.bins.get("umu-run").map(String::as_str),
            Some("/home/u/.local/bin/umu-run")
        );
    }

    /// Every `Store` field is `#[serde(default)]` so that a `state.toml` written
    /// by an older build keeps loading after new fields are added. Without this
    /// the first launch after an upgrade would reset the user's whole config.
    #[test]
    fn a_state_file_predating_the_new_fields_still_loads() {
        let old = r#"
theme = "Dracula"
show_irrelevant = true
[game_memory]
"#;
        let s: Store = toml::from_str(old).expect("an older state.toml still parses");
        assert_eq!(s.theme, "Dracula");
        assert!(s.show_irrelevant);
        assert!(s.favorites.is_empty());
        assert_eq!(s.library_sort, "");
        assert!(s.paths.steam_roots.is_empty());
        assert!(s.paths.bins.is_empty());
        assert_eq!(s.gpu_gen, "");
        assert!(s.global_profile.is_none());
    }

    #[test]
    fn a_state_file_with_a_partial_paths_table_still_loads() {
        // The failure mode a *nested* struct adds over flat fields: a `[paths]`
        // table written by an older build, or hand-edited, carries only some of
        // its keys. Every one of them has to default independently.
        let partial = r#"
theme = "Mocha"
[paths]
steam_roots = ["/mnt/games/Steam"]
"#;
        let s: Store = toml::from_str(partial).expect("a partial [paths] table still parses");
        assert_eq!(s.paths.steam_roots, vec!["/mnt/games/Steam".to_string()]);
        assert!(s.paths.proton_dirs.is_empty());
        assert!(s.paths.steam_libraries.is_empty());
        assert!(s.paths.bins.is_empty());
    }

    #[test]
    fn clean_drops_the_blank_rows_the_settings_ui_produces() {
        // The path rows keep an empty input around while the user types, so
        // blanks reach the backend on every keystroke.
        let list = vec!["  /opt/proton  ".to_string(), String::new(), "   ".to_string()];
        assert_eq!(Paths::clean(&list), vec!["/opt/proton"]);
    }

    #[test]
    fn apply_then_capture_is_stable() {
        let cat = Catalog::bundled();
        let env = vec![("PROTON_NO_NTSYNC".to_string(), "1".to_string())];
        let wrappers = vec![("mangohud".to_string(), String::new())];
        let (opts, unknown) = options_from_lists(&cat, &env, &wrappers);
        let (env2, wrappers2) = options_to_lists(&cat, &opts);
        assert_eq!(env, env2);
        assert_eq!(wrappers, wrappers2);
        assert!(unknown.is_empty(), "every key here is in the catalog");
    }

    #[test]
    fn an_env_key_the_catalog_no_longer_knows_comes_back_as_a_leftover() {
        // #62. `a552034` renamed PROTON_ENABLE_NVAPI to PROTON_DISABLE_NVAPI, so
        // any config saved before it names a key the catalog has no entry for.
        // Returning it is what lets the caller re-home it into `extra_env`
        // instead of the command quietly losing the variable.
        let cat = Catalog::bundled();
        let env = vec![
            ("PROTON_ENABLE_NVAPI".to_string(), "1".to_string()),
            ("PROTON_NO_NTSYNC".to_string(), "1".to_string()),
        ];
        let (opts, unknown) = options_from_lists(&cat, &env, &[]);

        assert_eq!(unknown, vec![("PROTON_ENABLE_NVAPI".to_string(), "1".to_string())]);
        // The known key still lands normally.
        let (captured, _) = options_to_lists(&cat, &opts);
        assert_eq!(captured, vec![("PROTON_NO_NTSYNC".to_string(), "1".to_string())]);
    }

    #[test]
    fn leftovers_keep_their_input_order_and_duplicates() {
        // `compose::merge_into_extra_env` dedups by key and relies on this order,
        // so it has to be the caller's order and not the catalog's.
        let cat = Catalog::bundled();
        let env = vec![
            ("ZZZ_NOT_A_REAL_KEY".to_string(), "2".to_string()),
            ("AAA_NOT_A_REAL_KEY".to_string(), "1".to_string()),
            ("ZZZ_NOT_A_REAL_KEY".to_string(), "3".to_string()),
        ];
        let (_, unknown) = options_from_lists(&cat, &env, &[]);
        assert_eq!(unknown, env);
    }

    #[test]
    fn save_to_writes_a_readable_store() {
        let dir = std::env::temp_dir().join(format!("protongen-save-{}", std::process::id()));
        let path = dir.join("nested").join("state.toml");
        let s = Store {
            theme: "Dracula".into(),
            ..Default::default()
        };

        // Parent directories are created on demand.
        s.save_to(&path).expect("save should succeed");
        let back: Store = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.theme, "Dracula");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_to_reports_a_parent_that_cannot_be_created() {
        // A regular file cannot become a directory, so create_dir_all fails.
        let blocker = std::env::temp_dir().join(format!("protongen-blocker-{}", std::process::id()));
        std::fs::write(&blocker, b"not a directory").unwrap();

        let err = Store::default()
            .save_to(&blocker.join("state.toml"))
            .expect_err("writing under a regular file must fail");

        // The path is the whole point of the message: it's what makes the
        // failure actionable.
        assert!(
            err.contains(&blocker.display().to_string()),
            "error should name the path, got: {err}"
        );

        std::fs::remove_file(&blocker).ok();
    }
}
