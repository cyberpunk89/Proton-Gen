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
    ///
    /// Returns a warning rather than only printing to stderr: a GUI user who
    /// drops a slightly malformed params.toml otherwise gets the bundled
    /// catalog with no indication their file was ignored.
    pub fn load() -> (Self, Option<ConfigWarning>) {
        let Some(path) = user_config_path() else {
            return (Self::bundled(), None);
        };
        // A missing override is the normal case, not a problem worth reporting.
        let Ok(text) = std::fs::read_to_string(&path) else {
            return (Self::bundled(), None);
        };

        let (cat, error) = Self::parse_or_bundled(&text);
        let warning = error.map(|error| {
            eprintln!("warning: {} failed to parse; using bundled catalog", path.display());
            ConfigWarning {
                kind: WarningKind::Parse,
                file: "params.toml".to_string(),
                path: path.display().to_string(),
                error,
            }
        });
        (cat, warning)
    }

    /// Pure core of [`Self::load`]: parse `text`, falling back to the bundled
    /// catalog and returning the toml error message.
    pub fn parse_or_bundled(text: &str) -> (Self, Option<String>) {
        match toml::from_str::<Catalog>(text) {
            Ok(cat) => (cat, None),
            Err(e) => (Self::bundled(), Some(e.to_string())),
        }
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

/// What kind of user input a [`ConfigWarning`] is about. The UI needs this to
/// word the banner: "couldn't be parsed; using the bundled catalog" is right for
/// a TOML override and nonsense for a Settings path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarningKind {
    /// A `params.toml` / `recipes.toml` override that failed to parse.
    Parse,
    /// A path configured in Settings that discovery could not use.
    Path,
}

/// Something the user configured that protongen could not use, surfaced rather
/// than silently ignored.
///
/// Deliberately not fatal: one bad entry must not kill discovery of everything
/// else. Deliberately not silent either — that is what makes a portability
/// feature feel broken.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ConfigWarning {
    pub kind: WarningKind,
    /// For a parse warning, the override file (`params.toml`). For a path
    /// warning, the Settings field it came from (`Steam root`, `Proton
    /// directory`, …) — the label the user needs to go and find.
    pub file: String,
    /// Absolute path, so the message can tell the user what to go and fix.
    pub path: String,
    /// The underlying toml parse error, or why the path was unusable.
    pub error: String,
}

impl ConfigWarning {
    /// A path from Settings that discovery could not use.
    pub fn path(field: &str, path: impl std::fmt::Display, error: impl Into<String>) -> Self {
        Self {
            kind: WarningKind::Path,
            file: field.to_string(),
            path: path.to_string(),
            error: error.into(),
        }
    }
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
///
/// `Clone` so `recipes::diff` can apply a recipe to a throwaway copy and compare,
/// keeping `recipes::apply` the single definition of the merge.
#[derive(Clone, Debug)]
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
        let cmd = crate::builder::build_command(&env, &wrappers, "", &crate::builder::Bins::default());
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

    #[test]
    fn parse_or_bundled_accepts_valid_toml() {
        let text = r#"
[[env]]
key = "MY_OVERRIDE"
category = "Custom"
default_value = "1"
values = ["1"]
help = "A user-supplied entry."
"#;
        let (cat, err) = Catalog::parse_or_bundled(text);
        assert!(err.is_none(), "valid toml should not warn: {err:?}");
        assert_eq!(cat.envs.len(), 1);
        assert_eq!(cat.envs[0].key, "MY_OVERRIDE");
    }

    #[test]
    fn parse_or_bundled_falls_back_and_reports_the_error() {
        let (cat, err) = Catalog::parse_or_bundled("this is not = valid toml [[[");
        // The user still gets a working app...
        assert_eq!(cat.envs.len(), Catalog::bundled().envs.len());
        // ...but the reason their file was ignored survives, rather than
        // being thrown away by an `if let Ok`.
        assert!(err.is_some(), "garbage input must produce an error message");
    }
}
