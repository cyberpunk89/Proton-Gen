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
                kind: params::WarningKind::Parse,
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

/// What applying a recipe would do to one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Currently off; applying turns it on.
    Enable,
    /// Already on, but with a different value.
    ValueChange,
    /// Already on with this exact value — applying changes nothing.
    NoOp,
    /// Not in the catalog, so it lands in the custom-env string.
    ExtraEnv,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecipeChange {
    pub key: String,
    pub kind: ChangeKind,
    /// The current value, when there is one to replace.
    pub from: Option<String>,
    pub to: String,
    pub is_wrapper: bool,
}

/// What `apply` *would* change, without changing it.
///
/// Implemented by cloning and calling `apply`, deliberately: that keeps `apply`
/// the single source of merge truth. Re-deriving the merge here would drift, and
/// a naive pair-diff could not tell "this key went to extra_env because the
/// catalog no longer has it" from "the user typed it into extra_env themselves".
pub fn diff(
    recipe: &Recipe,
    catalog: &Catalog,
    options: &Options,
    extra_env: &str,
) -> Vec<RecipeChange> {
    let mut after = options.clone();
    let mut after_extra = extra_env.to_string();
    apply(recipe, catalog, &mut after, &mut after_extra);

    let mut out = Vec::new();

    for (k, _) in &recipe.wrappers {
        let Some(i) = catalog.wrappers.iter().position(|w| &w.key == k) else {
            continue;
        };
        let (before, now) = (&options.wrappers[i], &after.wrappers[i]);
        let kind = if !before.enabled {
            ChangeKind::Enable
        } else if before.value != now.value {
            ChangeKind::ValueChange
        } else {
            ChangeKind::NoOp
        };
        out.push(RecipeChange {
            key: k.clone(),
            kind,
            from: before.enabled.then(|| before.value.clone()),
            to: now.value.clone(),
            is_wrapper: true,
        });
    }

    for (k, v) in &recipe.env {
        match catalog.envs.iter().position(|e| &e.key == k) {
            Some(i) => {
                let (before, now) = (&options.envs[i], &after.envs[i]);
                let kind = if !before.enabled {
                    ChangeKind::Enable
                } else if before.value != now.value {
                    ChangeKind::ValueChange
                } else {
                    ChangeKind::NoOp
                };
                out.push(RecipeChange {
                    key: k.clone(),
                    kind,
                    from: before.enabled.then(|| before.value.clone()),
                    to: now.value.clone(),
                    is_wrapper: false,
                });
            }
            None => {
                // Already present in extra_env means apply() would skip it.
                let pair = format!("{k}={v}");
                let already = extra_env.split_whitespace().any(|t| t == pair);
                out.push(RecipeChange {
                    key: k.clone(),
                    kind: if already { ChangeKind::NoOp } else { ChangeKind::ExtraEnv },
                    from: None,
                    to: v.clone(),
                    is_wrapper: false,
                });
            }
        }
    }

    out
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

    fn low_latency() -> Recipe {
        Recipes::bundled()
            .recipes
            .into_iter()
            .find(|r| r.name == "Low-latency competitive")
            .unwrap()
    }

    #[test]
    fn diff_on_a_fresh_config_is_all_enables() {
        let cat = Catalog::bundled();
        let opts = Options::from_catalog(&cat);
        let changes = diff(&low_latency(), &cat, &opts, "");

        assert!(!changes.is_empty());
        assert!(
            changes.iter().all(|c| c.kind == ChangeKind::Enable),
            "nothing is on yet, so every listed key must read as Enable: {changes:?}"
        );
        assert!(changes.iter().all(|c| c.from.is_none()));
    }

    #[test]
    fn diff_after_applying_is_all_noop() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        let mut extra = String::new();
        let recipe = low_latency();
        apply(&recipe, &cat, &mut opts, &mut extra);

        let changes = diff(&recipe, &cat, &opts, &extra);
        assert!(
            changes.iter().all(|c| c.kind == ChangeKind::NoOp),
            "applying twice must be a no-op — this is what makes the preview \
             honest about additive-only stacking: {changes:?}"
        );
    }

    #[test]
    fn diff_reports_a_value_change_when_the_key_is_already_on() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        let recipe = low_latency();

        // Turn one of the recipe's env keys on with a deliberately wrong value.
        let (key, wanted) = recipe.env.first().expect("recipe sets at least one env");
        let i = cat.envs.iter().position(|e| &e.key == key).expect("key is in the catalog");
        opts.envs[i].enabled = true;
        opts.envs[i].value = format!("{wanted}-not");

        let changes = diff(&recipe, &cat, &opts, "");
        let c = changes.iter().find(|c| &c.key == key).unwrap();
        assert_eq!(c.kind, ChangeKind::ValueChange);
        assert_eq!(c.from.as_deref(), Some(format!("{wanted}-not").as_str()));
        assert_eq!(&c.to, wanted);
    }

    #[test]
    fn diff_routes_an_uncatalogued_key_to_extra_env() {
        let cat = Catalog::bundled();
        let opts = Options::from_catalog(&cat);
        let mut recipe = low_latency();
        recipe.env = vec![("NOT_IN_THE_CATALOG".into(), "1".into())];
        recipe.wrappers.clear();

        let changes = diff(&recipe, &cat, &opts, "");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::ExtraEnv);
        assert_eq!(changes[0].to, "1");

        // And once it is already in extra_env, applying again would do nothing.
        let again = diff(&recipe, &cat, &opts, "NOT_IN_THE_CATALOG=1");
        assert_eq!(again[0].kind, ChangeKind::NoOp);
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
