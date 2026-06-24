//! Data-driven parameter catalog, loaded from `params.toml`.
//!
//! Load order:
//!   1. `$XDG_CONFIG_HOME/protongen/params.toml` (or `~/.config/...`) — user override
//!   2. the bundled `params.toml` baked in via `include_str!` — always works
//!
//! The bundled file is the single source of truth refreshed by the
//! `/update-proton-params` Claude skill.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::builder::Wrapper;
use crate::which;

/// The bundled default catalog.
const BUNDLED: &str = include_str!("../params.toml");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WrapperKind {
    Plain,
    Gamescope,
}

/// Rich, optional info surfaced in the GUI's per-parameter popup.
#[derive(Clone, Copy, Debug, Default)]
pub struct InfoText<'a> {
    pub details: Option<&'a str>,
    pub example: Option<&'a str>,
    pub url: Option<&'a str>,
}

impl InfoText<'_> {
    pub fn is_empty(&self) -> bool {
        self.details.is_none() && self.example.is_none() && self.url.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WrapperDef {
    pub key: String,
    #[serde(default)]
    pub label: Option<String>,
    pub kind: WrapperKind,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub example: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// Relevance hint: "nvidia" | "amd" | "intel" (unset = any GPU).
    #[serde(default)]
    pub gpu: Option<String>,
    /// Relevance tags: e.g. "wayland", "kde", "ntsync" (unset = always).
    #[serde(default)]
    pub needs: Vec<String>,
}

impl WrapperDef {
    pub fn info(&self) -> InfoText<'_> {
        InfoText {
            details: self.details.as_deref(),
            example: self.example.as_deref(),
            url: self.url.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnvDef {
    pub key: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub example: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// Relevance hint: "nvidia" | "amd" | "intel" (unset = any GPU).
    #[serde(default)]
    pub gpu: Option<String>,
    /// Relevance tags: e.g. "wayland", "kde", "ntsync" (unset = always).
    #[serde(default)]
    pub needs: Vec<String>,
}

impl EnvDef {
    pub fn info(&self) -> InfoText<'_> {
        InfoText {
            details: self.details.as_deref(),
            example: self.example.as_deref(),
            url: self.url.as_deref(),
        }
    }
}

fn default_category() -> String {
    "Other".to_string()
}

/// Catalog metadata (the `[meta]` table) — records the proton-cachyos build the
/// catalog was last refreshed against, used for the "catalog stale" banner.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Meta {
    #[serde(default)]
    pub proton_cachyos_build: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Catalog {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default, rename(deserialize = "wrapper", serialize = "wrappers"))]
    pub wrappers: Vec<WrapperDef>,
    #[serde(default, rename(deserialize = "env", serialize = "envs"))]
    pub envs: Vec<EnvDef>,
}

impl Catalog {
    /// Load from the user override if present, else the bundled default.
    pub fn load() -> Self {
        if let Some(path) = user_config_path() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(cat) = toml::from_str::<Catalog>(&text) {
                    return cat;
                }
                eprintln!("warning: {} failed to parse; using bundled catalog", path.display());
            }
        }
        Self::bundled()
    }

    /// The bundled catalog (also used as the parse-fixture in tests).
    pub fn bundled() -> Self {
        toml::from_str(BUNDLED).expect("bundled params.toml must parse")
    }

    /// Distinct env categories, in first-seen order.
    pub fn categories(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for e in &self.envs {
            if !seen.contains(&e.category) {
                seen.push(e.category.clone());
            }
        }
        seen
    }
}

/// `$XDG_CONFIG_HOME/protongen` (or `~/.config/protongen`).
pub fn config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("protongen"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/protongen"))
}

/// A file under the protongen config dir, e.g. `config_file("state.toml")`.
pub fn config_file(name: &str) -> Option<PathBuf> {
    config_dir().map(|d| d.join(name))
}

/// `$XDG_CONFIG_HOME/protongen/params.toml`, falling back to `~/.config/...`.
fn user_config_path() -> Option<PathBuf> {
    config_file("params.toml")
}

/// Live UI state for one catalog entry (enabled + current value).
#[derive(Clone, Debug)]
pub struct OptionState {
    pub enabled: bool,
    pub value: String,
}

/// All option states, parallel to `wrappers` then `envs`.
pub struct Options {
    pub wrappers: Vec<OptionState>,
    pub envs: Vec<OptionState>,
}

impl Options {
    pub fn from_catalog(cat: &Catalog) -> Self {
        Options {
            wrappers: cat
                .wrappers
                .iter()
                .map(|w| OptionState {
                    enabled: false,
                    value: w.default_value.clone(),
                })
                .collect(),
            envs: cat
                .envs
                .iter()
                .map(|e| OptionState {
                    enabled: false,
                    value: e.default_value.clone(),
                })
                .collect(),
        }
    }
}

/// Translate enabled options into (env pairs, wrappers) for the builder.
pub fn to_spec(cat: &Catalog, opts: &Options) -> (Vec<(String, String)>, Vec<Wrapper>) {
    let mut env = Vec::new();
    let mut wrappers = Vec::new();

    for (def, st) in cat.wrappers.iter().zip(&opts.wrappers) {
        if !st.enabled {
            continue;
        }
        match def.kind {
            WrapperKind::Gamescope => wrappers.push(Wrapper::Gamescope(st.value.clone())),
            WrapperKind::Plain => match def.key.as_str() {
                "gamemoderun" => wrappers.push(Wrapper::Gamemoderun),
                "mangohud" => wrappers.push(Wrapper::Mangohud),
                _ => {}
            },
        }
    }

    for (def, st) in cat.envs.iter().zip(&opts.envs) {
        if st.enabled {
            env.push((def.key.clone(), st.value.clone()));
        }
    }

    (env, wrappers)
}

/// Whether a definition's required binary is installed (None = no requirement).
pub fn requires_status(requires: &Option<String>) -> Option<bool> {
    requires.as_ref().map(|b| which::is_installed(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_parses_and_has_entries() {
        let cat = Catalog::bundled();
        assert!(!cat.wrappers.is_empty(), "expected wrappers");
        assert!(cat.envs.len() > 20, "expected a rich env catalog");
        // Wrappers we rely on by name must exist.
        let keys: Vec<&str> = cat.wrappers.iter().map(|w| w.key.as_str()).collect();
        assert!(keys.contains(&"gamescope"));
        assert!(keys.contains(&"mangohud"));
        assert!(keys.contains(&"gamemoderun"));
    }

    #[test]
    fn to_spec_orders_like_before() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        // Enable mangohud + gamemoderun wrappers and PROTON_ENABLE_WAYLAND.
        for (i, w) in cat.wrappers.iter().enumerate() {
            if w.key == "mangohud" || w.key == "gamemoderun" {
                opts.wrappers[i].enabled = true;
            }
        }
        for (i, e) in cat.envs.iter().enumerate() {
            if e.key == "PROTON_ENABLE_WAYLAND" {
                opts.envs[i].enabled = true;
                opts.envs[i].value = "1".to_string();
            }
        }
        let (env, wrappers) = to_spec(&cat, &opts);
        let cmd = crate::builder::build_command(&env, &wrappers, "");
        assert_eq!(cmd, "PROTON_ENABLE_WAYLAND=1 gamemoderun mangohud %command%");
    }

    #[test]
    fn all_entries_have_full_info() {
        let cat = Catalog::bundled();
        for w in &cat.wrappers {
            assert!(!w.info().is_empty(), "wrapper {} missing info", w.key);
            assert!(w.details.is_some() && w.example.is_some() && w.url.is_some(),
                "wrapper {} missing a details/example/url field", w.key);
        }
        for e in &cat.envs {
            assert!(e.details.is_some() && e.example.is_some() && e.url.is_some(),
                "env {} missing a details/example/url field", e.key);
        }
    }

    #[test]
    fn categories_are_ordered_and_unique() {
        let cat = Catalog::bundled();
        let cats = cat.categories();
        assert!(cats.contains(&"Performance / Sync".to_string()));
        // No duplicates.
        let mut sorted = cats.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), cats.len());
    }
}
