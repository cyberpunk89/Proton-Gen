//! One-click recipes: curated profiles + a symptom-based troubleshooter.
//! Loaded from `recipes.toml` (user override or bundled), mirroring `params.rs`.

use serde::{Deserialize, Serialize};

use crate::params::{self, Catalog, Options};

const BUNDLED: &str = include_str!("../recipes.toml");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecipeKind {
    Profile,
    Fix,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recipe {
    pub name: String,
    pub kind: RecipeKind,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub symptom: Option<String>,
    /// Relevance hint: "nvidia" | "amd" | "intel" | "any" (or unset = any).
    #[serde(default)]
    pub gpu: Option<String>,
    /// Capability requirements: "wayland" | "kde" | "ntsync" | "hdr". A recipe
    /// whose needs aren't met is hidden (or dimmed) by the frontend.
    #[serde(default)]
    pub needs: Vec<String>,
    /// Optional Phosphor-style icon name for the card (frontend maps it).
    #[serde(default)]
    pub icon: Option<String>,
    /// Optional accent color (hex, e.g. "#a6e3a1") for the card.
    #[serde(default)]
    pub accent: Option<String>,
    /// Optional free-form tags rendered as chips on the card.
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    #[serde(default)]
    pub wrappers: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Recipes {
    #[serde(default, rename(deserialize = "recipe", serialize = "recipes"))]
    pub recipes: Vec<Recipe>,
}

impl Recipes {
    /// Load from the user override if present, else the bundled default.
    /// Mirrors [`params::Catalog::load`]; see there for why the error survives.
    pub fn load() -> (Self, Option<params::ConfigWarning>) {
        let Some(path) = params::config_file("recipes.toml") else {
            return (Self::bundled(), None);
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return (Self::bundled(), None);
        };

        let (r, error) = Self::parse_or_bundled(&text);
        let warning = error.map(|error| {
            eprintln!("warning: {} failed to parse; using bundled recipes", path.display());
            params::ConfigWarning {
                file: "recipes.toml".to_string(),
                path: path.display().to_string(),
                error,
            }
        });
        (r, warning)
    }

    /// Pure core of [`Self::load`].
    pub fn parse_or_bundled(text: &str) -> (Self, Option<String>) {
        match toml::from_str::<Recipes>(text) {
            Ok(r) => (r, None),
            Err(e) => (Self::bundled(), Some(e.to_string())),
        }
    }

    pub fn bundled() -> Self {
        toml::from_str(BUNDLED).expect("bundled recipes.toml must parse")
    }

    pub fn by_kind(&self, kind: RecipeKind) -> impl Iterator<Item = (usize, &Recipe)> {
        self.recipes
            .iter()
            .enumerate()
            .filter(move |(_, r)| r.kind == kind)
    }
}

/// Merge a recipe onto the current options: enable + set its listed keys, leave
/// everything else untouched. Keys not in the catalog go to `extra_env`.
pub fn apply(recipe: &Recipe, catalog: &Catalog, options: &mut Options, extra_env: &mut String) {
    for (k, v) in &recipe.wrappers {
        if let Some(i) = catalog.wrappers.iter().position(|w| &w.key == k) {
            options.wrappers[i].enabled = true;
            if !v.is_empty() {
                options.wrappers[i].value = v.clone();
            }
        }
    }
    for (k, v) in &recipe.env {
        if let Some(i) = catalog.envs.iter().position(|e| &e.key == k) {
            options.envs[i].enabled = true;
            options.envs[i].value = v.clone();
        } else {
            // Unknown key (catalog drift) → append to custom env.
            let pair = format!("{k}={v}");
            if !extra_env.split_whitespace().any(|t| t == pair) {
                if !extra_env.is_empty() {
                    extra_env.push(' ');
                }
                extra_env.push_str(&pair);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_parses_and_has_both_kinds() {
        let r = Recipes::bundled();
        assert!(r.by_kind(RecipeKind::Profile).count() >= 3);
        assert!(r.by_kind(RecipeKind::Fix).count() >= 3);
        // Every fix should describe a symptom.
        for (_, rec) in r.by_kind(RecipeKind::Fix) {
            assert!(rec.symptom.is_some(), "fix '{}' missing symptom", rec.name);
        }
    }

    #[test]
    fn apply_enables_listed_keys_only() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        let mut extra = String::new();
        let recipe = Recipes::bundled()
            .recipes
            .into_iter()
            .find(|r| r.name == "Low-latency competitive")
            .unwrap();
        apply(&recipe, &cat, &mut opts, &mut extra);

        let on_env: Vec<&str> = cat
            .envs
            .iter()
            .zip(&opts.envs)
            .filter(|(_, s)| s.enabled)
            .map(|(d, _)| d.key.as_str())
            .collect();
        assert!(on_env.contains(&"LOW_LATENCY_LAYER"));
        assert!(on_env.contains(&"PROTON_DXVK_LOWLATENCY"));
        let on_wrap: Vec<&str> = cat
            .wrappers
            .iter()
            .zip(&opts.wrappers)
            .filter(|(_, s)| s.enabled)
            .map(|(d, _)| d.key.as_str())
            .collect();
        assert!(on_wrap.contains(&"gamemoderun"));
        assert!(on_wrap.contains(&"mangohud"));
    }

    #[test]
    fn parse_or_bundled_falls_back_and_reports_the_error() {
        let (r, err) = Recipes::parse_or_bundled("nope = [[[");
        assert_eq!(r.recipes.len(), Recipes::bundled().recipes.len());
        assert!(err.is_some(), "garbage input must produce an error message");

        let (ok, none) = Recipes::parse_or_bundled(BUNDLED);
        assert!(none.is_none());
        assert_eq!(ok.recipes.len(), Recipes::bundled().recipes.len());
    }
}
