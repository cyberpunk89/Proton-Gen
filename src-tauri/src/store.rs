//! Persisted state: chosen theme, named presets, and per-game memory.
//! Stored as TOML at `$XDG_CONFIG_HOME/protongen/state.toml`.

use std::collections::BTreeMap;

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
    #[serde(default)]
    pub fsr4: bool,
    /// Auto-fetch the ProtonDB tier when a Steam game is selected.
    #[serde(default)]
    pub protondb_auto: bool,
    /// The exact builder state on screen when the app last closed, restored on
    /// next launch so the user reopens where they left off.
    #[serde(default)]
    pub last_session: Option<Config>,
    /// The game selected in that last session (matches `GameDto.app_id`), so it
    /// can be re-selected on launch.
    #[serde(default)]
    pub last_game_appid: Option<u32>,
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

/// Reset options to catalog defaults, then enable + set values from the lists.
pub fn apply_lists(
    catalog: &Catalog,
    options: &mut Options,
    env: &[(String, String)],
    wrappers: &[(String, String)],
) {
    *options = Options::from_catalog(catalog);
    for (k, v) in wrappers {
        if let Some(i) = catalog.wrappers.iter().position(|w| &w.key == k) {
            options.wrappers[i].enabled = true;
            options.wrappers[i].value = v.clone();
        }
    }
    for (k, v) in env {
        if let Some(i) = catalog.envs.iter().position(|e| &e.key == k) {
            options.envs[i].enabled = true;
            options.envs[i].value = v.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_toml_roundtrip() {
        let mut s = Store {
            theme: "Dracula".into(),
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
        let text = toml::to_string_pretty(&s).unwrap();
        let back: Store = toml::from_str(&text).unwrap();
        assert_eq!(back.theme, "Dracula");
        assert_eq!(back.presets.len(), 1);
        assert_eq!(back.presets[0].name, "hd2");
        assert_eq!(back.recall(553850).unwrap().game_args, "-windowed");
        assert_eq!(back.last_game_appid, Some(553850));
        let sess = back.last_session.expect("last_session round-trips");
        assert!(sess.umu);
        assert_eq!(sess.extra_env, "FOO=bar");
    }

    #[test]
    fn apply_then_capture_is_stable() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        let env = vec![("PROTON_NO_NTSYNC".to_string(), "1".to_string())];
        let wrappers = vec![("mangohud".to_string(), String::new())];
        apply_lists(&cat, &mut opts, &env, &wrappers);
        let (env2, wrappers2) = options_to_lists(&cat, &opts);
        assert_eq!(env, env2);
        assert_eq!(wrappers, wrappers2);
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

/// Env pairs from a parsed/imported command that are NOT in the catalog,
/// rendered as a space-separated `K=V` string for the "custom env" field.
pub fn unknown_env_string(catalog: &Catalog, env: &[(String, String)]) -> String {
    env.iter()
        .filter(|(k, _)| !catalog.envs.iter().any(|e| &e.key == k))
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}
