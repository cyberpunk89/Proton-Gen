//! Lightweight conflict / footgun detection over the enabled options.
//!
//! Each rule is a `Rule` in [`RULES`]: a stable id, the catalog keys it depends
//! on, and a check that turns the current selection into an optional [`Notice`].
//! Notices carry a severity, the parameter keys they implicate (so the UI can
//! jump to the offending row) and — where the remedy is unambiguous — a [`Fix`]
//! the frontend can apply with its existing toggle/set helpers. No Rust command
//! is needed to apply one; a fix is just "disable these, enable these pairs".

use serde::Serialize;

use crate::hardware::Hardware;
use crate::params::{Catalog, Options};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub enum Severity {
    /// Will actively break something (bans, hard conflicts).
    Error,
    /// Works, but not the way the user probably intends.
    Warning,
    /// Purely informational — no effect, or a hardware caveat.
    Info,
}

/// A one-click remedy. Interpreted by the frontend against its own option state.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Fix {
    /// Button text, e.g. "Disable PROTON_USE_WINED3D".
    pub label: String,
    /// Catalog keys (env or wrapper) to turn off.
    pub disable: Vec<String>,
    /// Catalog env keys to turn on, with the value to set.
    pub enable: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Notice {
    /// Stable rule id — safe to use as a list key or a dismissal token.
    pub id: String,
    pub severity: Severity,
    pub message: String,
    /// Catalog keys this notice is about, for click-to-jump. Always a subset of
    /// the owning rule's declared `keys`.
    pub keys: Vec<String>,
    pub fix: Option<Fix>,
}

/// Everything a rule is allowed to look at.
pub struct Ctx<'a> {
    catalog: &'a Catalog,
    options: &'a Options,
    hw: &'a Hardware,
    /// The AMD generation the user declared in Settings (`store.gpu_gen`): "",
    /// "rdna3" or "rdna4".
    ///
    /// Carried separately from `hw` because it is a *declaration*, not a
    /// detection — nothing here can read it off the hardware. Without it the
    /// FSR4 rules can only state both generations' caveats at once, which is
    /// half noise for anyone who has actually picked one.
    gpu_gen: &'a str,
    /// Manual-OptiScaler leftovers in the selected game's folder (see
    /// `optiscaler_upgrade::manual_install_files`). The one rule input read off
    /// the disk rather than the selection — `ipc::lint` scans, rules stay pure.
    game_files: &'a [String],
    /// Kernel anti-cheat the selected game ships ("EasyAntiCheat",
    /// "BattlEye"), from its cached folder scan (`game_scan.rs`); empty when
    /// unscanned. Lets the anti-cheat rules fire without the user having
    /// turned on PROTON_EAC_RUNTIME first.
    anticheat: &'a [String],
}

impl Ctx<'_> {
    /// Enabled *and* not set to `0`. The 1/0 switches are often kept in the
    /// command at `=0` to toggle quickly; [`Self::env_on`] would count those.
    fn flag_on(&self, key: &str) -> bool {
        self.env_value(key).is_some_and(|v| v.trim() != "0")
    }

    /// [`Self::flag_on`] over `keys`, in the given order.
    fn flags_on(&self, keys: &[&str]) -> Vec<String> {
        keys.iter().filter(|k| self.flag_on(k)).map(|k| k.to_string()).collect()
    }

    fn env_on(&self, key: &str) -> bool {
        self.catalog
            .envs
            .iter()
            .zip(&self.options.envs)
            .any(|(d, s)| s.enabled && d.key == key)
    }

    fn wrap_on(&self, key: &str) -> bool {
        self.catalog
            .wrappers
            .iter()
            .zip(&self.options.wrappers)
            .any(|(d, s)| s.enabled && d.key == key)
    }

    /// An enabled wrapper's argument string (gamescope's args); `None` when off.
    fn wrap_value(&self, key: &str) -> Option<&str> {
        self.catalog
            .wrappers
            .iter()
            .zip(&self.options.wrappers)
            .find(|(d, s)| s.enabled && d.key == key)
            .map(|(_, s)| s.value.as_str())
    }

    /// Whether gamescope is on and its args carry `flag` as a whole word.
    fn gamescope_has(&self, flag: &str) -> bool {
        self.wrap_value("gamescope")
            .is_some_and(|args| args.split_whitespace().any(|a| a == flag || a.starts_with(&format!("{flag}="))))
    }

    /// Which of `keys` are currently enabled, in the given order.
    fn envs_on(&self, keys: &[&str]) -> Vec<String> {
        keys.iter()
            .filter(|k| self.env_on(k))
            .map(|k| k.to_string())
            .collect()
    }

    /// An enabled env row's value; `None` when the row is off.
    fn env_value(&self, key: &str) -> Option<&str> {
        self.catalog
            .envs
            .iter()
            .zip(&self.options.envs)
            .find(|(d, s)| s.enabled && d.key == key)
            .map(|(_, s)| s.value.as_str())
    }

    /// Lossless Scaling frame generation is switched on for this launch: a
    /// profile picked, or per-game settings mode, and not vetoed. (A profile
    /// can also auto-match the game from conf.toml — invisible from here.)
    fn lsfg_on(&self) -> bool {
        !self.env_on("DISABLE_LSFGVK") && (self.env_on("LSFGVK_PROFILE") || self.env_on("LSFGVK_ENV"))
    }

    /// Enabled env keys starting with `prefix` — for rules that implicate a
    /// family rather than a fixed list.
    fn envs_on_prefixed(&self, prefix: &str) -> Vec<String> {
        self.catalog
            .envs
            .iter()
            .zip(&self.options.envs)
            .filter(|(d, s)| s.enabled && d.key.starts_with(prefix))
            .map(|(d, _)| d.key.clone())
            .collect()
    }
}

/// One lint rule.
///
/// Only `check` runs in production: `id`, `keys` and `prefixes` are a
/// *declaration about* the check, consumed by the guards in `mod tests`. Hence
/// the allow — they are documentation the test suite happens to enforce.
#[cfg_attr(not(test), allow(dead_code))]
struct Rule {
    id: &'static str,
    /// Every catalog key the rule reads or can report. Verified against the
    /// bundled catalog by `all_notice_keys_exist_in_bundled_catalog` — the
    /// `update-proton-params` skill rewrites `params.toml`, and a renamed key
    /// otherwise makes a rule silently stop matching. That has happened twice:
    /// the `PROTON_ENABLE_NVAPI` -> `PROTON_DISABLE_NVAPI` rename orphaned the
    /// NVAPI rule, and `PROTON_ENABLE_HDR` was dropped outright, quietly killing
    /// the obsolete-alias rule that used to live here.
    keys: &'static [&'static str],
    /// Key *families* the rule may additionally report (e.g. every enabled
    /// `DXVK_*`). Sourced from the catalog itself, so they can't dangle — but
    /// they still have to be declared so the emitted-key check stays exhaustive.
    prefixes: &'static [&'static str],
    check: fn(&Ctx) -> Option<Notice>,
}

const NVAPI_KEYS: &[&str] = &[
    "PROTON_FORCE_NVAPI",
    "DXVK_ENABLE_NVAPI",
    "PROTON_DLSS_UPGRADE",
    "DXVK_NVAPI_VKREFLEX",
    "PROTON_NVIDIA_LIBS",
];

/// lsfg-vk settings that are only read in environment mode (`LSFGVK_ENV=1`).
const LSFG_ENV_ONLY_KEYS: &[&str] = &[
    "LSFGVK_MULTIPLIER",
    "LSFGVK_FLOW_SCALE",
    "LSFGVK_PERFORMANCE_MODE",
    "LSFGVK_DLL_PATH",
    "LSFGVK_PACING_MODE",
    "LSFGVK_OVERRIDE_PRESENT_MODE",
    "LSFGVK_PRESERVE_SWAPCHAIN_IMAGE_COUNT",
    "LSFGVK_NO_FP16",
    "LSFGVK_LOG_LEVEL",
    "LSFGVK_LOG_FILE",
];

/// [`LSFG_ENV_ONLY_KEYS`] plus the switch its rule's fix turns on.
const LSFG_ENV_ONLY_KEYS_AND_ENV: &[&str] = &[
    "LSFGVK_MULTIPLIER",
    "LSFGVK_FLOW_SCALE",
    "LSFGVK_PERFORMANCE_MODE",
    "LSFGVK_DLL_PATH",
    "LSFGVK_PACING_MODE",
    "LSFGVK_OVERRIDE_PRESENT_MODE",
    "LSFGVK_PRESERVE_SWAPCHAIN_IMAGE_COUNT",
    "LSFGVK_NO_FP16",
    "LSFGVK_LOG_LEVEL",
    "LSFGVK_LOG_FILE",
    "LSFGVK_ENV",
];

/// The env vars `prime-run` sets for you.
const PRIME_ENV_KEYS: &[&str] =
    &["__NV_PRIME_RENDER_OFFLOAD", "__GLX_VENDOR_LIBRARY_NAME", "__VK_LAYER_NV_optimus"];

/// [`PRIME_ENV_KEYS`] plus the wrapper its rule reports.
const PRIME_KEYS_AND_WRAPPER: &[&str] = &[
    "prime-run",
    "__NV_PRIME_RENDER_OFFLOAD",
    "__GLX_VENDOR_LIBRARY_NAME",
    "__VK_LAYER_NV_optimus",
];

/// Implicit Vulkan layers that pace the CPU against the GPU to cut latency.
const LATENCY_LAYERS: &[&str] = &["ENABLE_LAYER_MESA_ANTI_LAG", "LOW_LATENCY_LAYER"];

/// [`LATENCY_LAYERS`] plus the injection its OptiScaler rule reports.
const LATENCY_LAYERS_AND_OPTISCALER: &[&str] =
    &["ENABLE_LAYER_MESA_ANTI_LAG", "LOW_LATENCY_LAYER", "PROTON_USE_OPTISCALER"];

/// The `Section.Key=Value` entries of a `PROTON_OPTISCALER_CONFIG` string —
/// the same `;`-separated shape `src/lib/optiscaler.ts` parses.
fn optiscaler_entries(cfg: &str) -> impl Iterator<Item = (&str, &str)> {
    cfg.split(';').filter_map(|e| {
        let (k, v) = e.split_once('=')?;
        Some((k.trim(), v.trim()))
    })
}

/// Whether an OptiScaler config turns OptiFG on (`FrameGen.Enabled=true`).
fn optifg_enabled(cfg: &str) -> bool {
    optiscaler_entries(cfg)
        .any(|(k, v)| k.eq_ignore_ascii_case("FrameGen.Enabled") && v.eq_ignore_ascii_case("true"))
}

/// `cfg` with every `FrameGen.*` entry replaced by an explicit
/// `FrameGen.Enabled=false`. Explicit rather than just dropped: an empty value
/// can't be expressed as a lint fix (a blank value means "leave it alone").
fn without_optifg(cfg: &str) -> String {
    cfg.split(';')
        .filter(|e| !e.trim().is_empty())
        .filter(|e| {
            let key = e.split_once('=').map_or(*e, |(k, _)| k).trim();
            !key.to_ascii_lowercase().starts_with("framegen.")
        })
        .chain(std::iter::once("FrameGen.Enabled=false"))
        .collect::<Vec<_>>()
        .join(";")
}

const RULES: &[Rule] = &[
    // NVAPI / DLSS without an NVIDIA GPU.
    Rule {
        id: "nvapi-without-nvidia",
        keys: NVAPI_KEYS,
        prefixes: &[],
        check: |c| {
            if c.hw.nvidia {
                return None;
            }
            let on = c.envs_on(NVAPI_KEYS);
            if on.is_empty() {
                return None;
            }
            Some(Notice {
                id: "nvapi-without-nvidia".to_string(),
                severity: Severity::Info,
                message: "NVAPI/DLSS options are enabled but no NVIDIA GPU was detected — they'll have no effect.".to_string(),
                keys: on.clone(),
                fix: Some(Fix {
                    label: "Turn off the NVAPI options".to_string(),
                    disable: on,
                    enable: Vec::new(),
                }),
            })
        },
    },
    // FSR4 hardware note, for when we don't know which AMD generation this is.
    //
    // Deliberately silent once `gpu_gen` is set: the two generation-specific
    // rules below say the one thing that actually applies. This rule used to
    // fire unconditionally and recite both generations' caveats even to someone
    // who had told us their card in Settings.
    Rule {
        id: "fsr4-hardware-note",
        keys: &["PROTON_FSR4_UPGRADE"],
        prefixes: &[],
        check: |c| {
            if !c.env_on("PROTON_FSR4_UPGRADE") || !c.gpu_gen.is_empty() {
                return None;
            }
            Some(Notice {
                id: "fsr4-hardware-note".to_string(),
                severity: Severity::Info,
                message: "FSR 4 needs an RDNA3 or RDNA4 AMD GPU. Set your GPU generation in Settings and protongen will show only the options that fit it."
                    .to_string(),
                keys: vec!["PROTON_FSR4_UPGRADE".to_string()],
                fix: None,
            })
        },
    },
    // RDNA3 + MLFG without the WMMA workaround. Auto-fixable now that the
    // generation is a known quantity — the note this replaced could only
    // describe the remedy in prose.
    //
    // Silent under OptiScaler: MLFG is a no-op there (`mlfg-ignored-with-optiscaler`),
    // and a workaround for a no-op is advice to stack one more dead variable.
    Rule {
        id: "rdna3-mlfg-workaround",
        keys: &["PROTON_MLFG_UPGRADE", "DXIL_SPIRV_CONFIG", "PROTON_USE_OPTISCALER"],
        prefixes: &[],
        check: |c| {
            if c.gpu_gen != "rdna3"
                || !c.env_on("PROTON_MLFG_UPGRADE")
                || c.env_on("DXIL_SPIRV_CONFIG")
                || c.flag_on("PROTON_USE_OPTISCALER")
            {
                return None;
            }
            Some(Notice {
                id: "rdna3-mlfg-workaround".to_string(),
                severity: Severity::Warning,
                message: "On RDNA3, multi-frame generation also needs DXIL_SPIRV_CONFIG=wmma_rdna3_workaround."
                    .to_string(),
                keys: vec!["PROTON_MLFG_UPGRADE".to_string()],
                fix: Some(Fix {
                    label: "Add the RDNA3 workaround".to_string(),
                    disable: Vec::new(),
                    enable: vec![(
                        "DXIL_SPIRV_CONFIG".to_string(),
                        "wmma_rdna3_workaround".to_string(),
                    )],
                }),
            })
        },
    },
    // The mirror image: the workaround does nothing on RDNA4. The param row is
    // hidden there, so this only fires on a command imported from an RDNA3 setup
    // or a state file predating the generation selector — which is exactly when
    // nothing else would explain the stray variable.
    Rule {
        id: "rdna4-workaround-noop",
        keys: &["DXIL_SPIRV_CONFIG"],
        prefixes: &[],
        check: |c| {
            if c.gpu_gen != "rdna4" || !c.env_on("DXIL_SPIRV_CONFIG") {
                return None;
            }
            Some(Notice {
                id: "rdna4-workaround-noop".to_string(),
                severity: Severity::Info,
                message: "DXIL_SPIRV_CONFIG is an RDNA3 workaround and does nothing on RDNA4."
                    .to_string(),
                keys: vec!["DXIL_SPIRV_CONFIG".to_string()],
                fix: Some(Fix {
                    label: "Remove DXIL_SPIRV_CONFIG".to_string(),
                    disable: vec!["DXIL_SPIRV_CONFIG".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // OptiScaler settings without the injection that makes them do anything.
    // The builder always enables it, so this catches the other routes in: an
    // imported launch string, or a hand-edited row.
    Rule {
        id: "optiscaler-not-injected",
        keys: &[
            "PROTON_USE_OPTISCALER",
            "PROTON_OPTISCALER_CONFIG",
            "PROTON_OPTISCALER_NAME",
        ],
        prefixes: &[],
        check: |c| {
            if c.env_on("PROTON_USE_OPTISCALER") {
                return None;
            }
            let on = c.envs_on(&["PROTON_OPTISCALER_CONFIG", "PROTON_OPTISCALER_NAME"]);
            if on.is_empty() {
                return None;
            }
            Some(Notice {
                id: "optiscaler-not-injected".to_string(),
                severity: Severity::Warning,
                message: "OptiScaler is configured but not injected — set PROTON_USE_OPTISCALER=1 or the settings are ignored."
                    .to_string(),
                keys: on,
                fix: Some(Fix {
                    label: "Enable PROTON_USE_OPTISCALER".to_string(),
                    disable: Vec::new(),
                    enable: vec![("PROTON_USE_OPTISCALER".to_string(), "1".to_string())],
                }),
            })
        },
    },
    // proton-cachyos' `setup_upscalers` clears its upgrade set once OptiScaler is
    // injected, so MLFG_UPGRADE never reaches amdxc64 — frame generation is
    // whatever OptiScaler's FrameGen settings say. (PROTON_FSR4_UPGRADE still
    // matters there: it picks the FSR runtime version OptiScaler is given.)
    Rule {
        id: "mlfg-ignored-with-optiscaler",
        keys: &["PROTON_MLFG_UPGRADE", "PROTON_USE_OPTISCALER"],
        prefixes: &[],
        check: |c| {
            if !(c.flag_on("PROTON_MLFG_UPGRADE") && c.flag_on("PROTON_USE_OPTISCALER")) {
                return None;
            }
            Some(Notice {
                id: "mlfg-ignored-with-optiscaler".to_string(),
                severity: Severity::Warning,
                message: "PROTON_MLFG_UPGRADE does nothing while OptiScaler is injected — Proton hands FSR over to OptiScaler and skips its own frame-gen upgrade. Choose frame generation in OptiScaler's settings instead."
                    .to_string(),
                keys: vec!["PROTON_MLFG_UPGRADE".to_string(), "PROTON_USE_OPTISCALER".to_string()],
                fix: Some(Fix {
                    label: "Remove PROTON_MLFG_UPGRADE".to_string(),
                    disable: vec!["PROTON_MLFG_UPGRADE".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // Two CPU-pacing layers. The catalog's own Anti-Lag entry already says "try
    // one at a time"; this says it where it's acted on. No fix: either is fine.
    Rule {
        id: "latency-layers-stacked",
        keys: LATENCY_LAYERS,
        prefixes: &[],
        check: |c| {
            let on = c.flags_on(LATENCY_LAYERS);
            if on.len() < 2 {
                return None;
            }
            Some(Notice {
                id: "latency-layers-stacked".to_string(),
                severity: Severity::Warning,
                message: "Mesa Anti-Lag and LOW_LATENCY_LAYER both pace the CPU against the GPU — stacked, they fight over frame timing. Keep one."
                    .to_string(),
                keys: on,
                fix: None,
            })
        },
    },
    // A Vulkan latency layer underneath OptiScaler. The injected OptiScaler
    // ships fakenvapi, which already turns the game's Reflex into AMD/Intel
    // latency reduction, and paces its generated frames — the layer is a
    // second limiter on the same frames. Seen in the wild as launches dying
    // within seconds that came back the moment Anti-Lag was dropped.
    Rule {
        id: "latency-layer-with-optiscaler",
        keys: LATENCY_LAYERS_AND_OPTISCALER,
        prefixes: &[],
        check: |c| {
            if !c.flag_on("PROTON_USE_OPTISCALER") {
                return None;
            }
            let on = c.flags_on(LATENCY_LAYERS);
            if on.is_empty() {
                return None;
            }
            Some(Notice {
                id: "latency-layer-with-optiscaler".to_string(),
                severity: Severity::Warning,
                message: "OptiScaler brings its own latency reduction (fakenvapi) and frame pacing, so a Vulkan latency layer underneath it is a second limiter. If the game crashes at launch or stutters, turn the layer off first."
                    .to_string(),
                keys: on.iter().cloned().chain(["PROTON_USE_OPTISCALER".to_string()]).collect(),
                fix: Some(Fix {
                    label: if on.len() == 1 {
                        format!("Turn off {}", on[0])
                    } else {
                        "Turn off the latency layers".to_string()
                    },
                    disable: on,
                    enable: Vec::new(),
                }),
            })
        },
    },
    // A hand-installed OptiScaler in the game folder *and* Proton's injected
    // one: two builds, two sets of FSR/XeSS runtimes, mixed in one process.
    // No fix — removing files is the user's job (then a Steam file verify), and
    // dropping the injection is only right if they prefer the manual copy.
    Rule {
        id: "optiscaler-double-install",
        keys: &["PROTON_USE_OPTISCALER"],
        prefixes: &[],
        check: |c| {
            if !c.flag_on("PROTON_USE_OPTISCALER") || c.game_files.is_empty() {
                return None;
            }
            const SHOWN: usize = 4;
            let mut files = c.game_files.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
            if c.game_files.len() > SHOWN {
                files.push_str(&format!(" +{} more", c.game_files.len() - SHOWN));
            }
            Some(Notice {
                id: "optiscaler-double-install".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "This game's folder has a manual OptiScaler install ({files}), and PROTON_USE_OPTISCALER injects a second copy — their DLL versions get mixed in one process. Remove the manual files and verify the game in Steam, or turn PROTON_USE_OPTISCALER off."
                ),
                keys: vec!["PROTON_USE_OPTISCALER".to_string()],
                fix: None,
            })
        },
    },
    // lsfg-vk reads its per-game variables only in environment mode; without
    // LSFGVK_ENV=1 it uses conf.toml and these do nothing at all.
    Rule {
        id: "lsfg-needs-env-mode",
        keys: LSFG_ENV_ONLY_KEYS_AND_ENV,
        prefixes: &[],
        check: |c| {
            if c.env_on("LSFGVK_ENV") || c.env_on("DISABLE_LSFGVK") {
                return None;
            }
            let on = c.envs_on(LSFG_ENV_ONLY_KEYS);
            if on.is_empty() {
                return None;
            }
            Some(Notice {
                id: "lsfg-needs-env-mode".to_string(),
                severity: Severity::Warning,
                message: "lsfg-vk ignores these settings and reads conf.toml instead — set LSFGVK_ENV=1 to configure frame generation from the launch options."
                    .to_string(),
                keys: on,
                fix: Some(Fix {
                    label: "Enable LSFGVK_ENV".to_string(),
                    disable: Vec::new(),
                    enable: vec![("LSFGVK_ENV".to_string(), "1".to_string())],
                }),
            })
        },
    },
    // Environment mode doesn't read conf.toml, so there is no profile to pick.
    Rule {
        id: "lsfg-profile-in-env-mode",
        keys: &["LSFGVK_PROFILE", "LSFGVK_ENV"],
        prefixes: &[],
        check: |c| {
            if !(c.env_on("LSFGVK_PROFILE") && c.env_on("LSFGVK_ENV")) {
                return None;
            }
            Some(Notice {
                id: "lsfg-profile-in-env-mode".to_string(),
                severity: Severity::Warning,
                message: "LSFGVK_ENV=1 makes lsfg-vk skip conf.toml, so LSFGVK_PROFILE is ignored — use a profile or per-game settings, not both."
                    .to_string(),
                keys: vec!["LSFGVK_PROFILE".to_string(), "LSFGVK_ENV".to_string()],
                fix: None,
            })
        },
    },
    // DISABLE_LSFGVK vetoes the layer, so anything else in the family is dead.
    Rule {
        id: "lsfg-disabled",
        keys: &["DISABLE_LSFGVK"],
        prefixes: &["LSFGVK_"],
        check: |c| {
            if !c.env_on("DISABLE_LSFGVK") {
                return None;
            }
            let on = c.envs_on_prefixed("LSFGVK_");
            if on.is_empty() {
                return None;
            }
            Some(Notice {
                id: "lsfg-disabled".to_string(),
                severity: Severity::Info,
                message: "DISABLE_LSFGVK turns Lossless Scaling frame generation off, so the other LSFGVK_* settings have no effect."
                    .to_string(),
                keys: std::iter::once("DISABLE_LSFGVK".to_string()).chain(on).collect(),
                fix: None,
            })
        },
    },
    // Two frame generators in one chain: OptiScaler's OptiFG generates frames
    // inside the game, then lsfg-vk generates more from those — artifacts and
    // latency compound. OptiScaler for *upscaling* plus lsfg-vk is the pairing.
    Rule {
        id: "lsfg-double-framegen",
        keys: &["LSFGVK_PROFILE", "LSFGVK_ENV", "PROTON_USE_OPTISCALER", "PROTON_OPTISCALER_CONFIG"],
        prefixes: &[],
        check: |c| {
            if !c.lsfg_on() || !c.env_on("PROTON_USE_OPTISCALER") {
                return None;
            }
            let cfg = c.env_value("PROTON_OPTISCALER_CONFIG")?;
            if !optifg_enabled(cfg) {
                return None;
            }
            Some(Notice {
                id: "lsfg-double-framegen".to_string(),
                severity: Severity::Warning,
                message: "OptiScaler frame generation and Lossless Scaling are both on — frames get generated twice. Keep OptiScaler for upscaling and let Lossless Scaling do the frame generation."
                    .to_string(),
                keys: std::iter::once("PROTON_OPTISCALER_CONFIG".to_string())
                    .chain(c.envs_on(&["LSFGVK_PROFILE", "LSFGVK_ENV"]))
                    .collect(),
                fix: Some(Fix {
                    label: "Turn off OptiScaler frame generation".to_string(),
                    disable: Vec::new(),
                    enable: vec![("PROTON_OPTISCALER_CONFIG".to_string(), without_optifg(cfg))],
                }),
            })
        },
    },
    // proton-cachyos' ML frame-gen upgrade only acts when the game's own FSR
    // frame generation is on — which would stack under lsfg-vk.
    Rule {
        id: "lsfg-with-game-framegen",
        keys: &["LSFGVK_PROFILE", "LSFGVK_ENV", "PROTON_MLFG_UPGRADE"],
        prefixes: &[],
        check: |c| {
            if !c.lsfg_on() || !c.env_on("PROTON_MLFG_UPGRADE") {
                return None;
            }
            Some(Notice {
                id: "lsfg-with-game-framegen".to_string(),
                severity: Severity::Info,
                message: "PROTON_MLFG_UPGRADE upgrades the game's own FSR frame generation. If that is on in-game while Lossless Scaling is too, frames are generated twice — use one or the other."
                    .to_string(),
                keys: std::iter::once("PROTON_MLFG_UPGRADE".to_string())
                    .chain(c.envs_on(&["LSFGVK_PROFILE", "LSFGVK_ENV"]))
                    .collect(),
                fix: None,
            })
        },
    },
    // wined3d routes D3D through OpenGL, so DXVK is out of the picture.
    Rule {
        id: "wined3d-disables-dxvk",
        keys: &["PROTON_USE_WINED3D"],
        prefixes: &["DXVK_"],
        check: |c| {
            if !c.env_on("PROTON_USE_WINED3D") {
                return None;
            }
            let dxvk = c.envs_on_prefixed("DXVK_");
            if dxvk.is_empty() {
                return None;
            }
            Some(Notice {
                id: "wined3d-disables-dxvk".to_string(),
                severity: Severity::Warning,
                message: "PROTON_USE_WINED3D routes D3D through OpenGL — your DXVK_* options won't apply.".to_string(),
                keys: std::iter::once("PROTON_USE_WINED3D".to_string()).chain(dxvk).collect(),
                fix: Some(Fix {
                    label: "Disable PROTON_USE_WINED3D".to_string(),
                    disable: vec!["PROTON_USE_WINED3D".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // HDR output needs a presentation path that can carry it.
    Rule {
        id: "hdr-needs-presentation",
        keys: &["DXVK_HDR", "PROTON_ENABLE_WAYLAND", "gamescope"],
        prefixes: &[],
        check: |c| {
            if !c.env_on("DXVK_HDR") || c.env_on("PROTON_ENABLE_WAYLAND") || c.wrap_on("gamescope") {
                return None;
            }
            Some(Notice {
                id: "hdr-needs-presentation".to_string(),
                severity: Severity::Warning,
                message: "HDR needs PROTON_ENABLE_WAYLAND=1 or gamescope with --hdr-enabled to take effect.".to_string(),
                keys: vec!["DXVK_HDR".to_string()],
                // Only offer the Wayland route on a Wayland session; suggesting
                // it under X11 would swap one non-working setup for another.
                fix: c.hw.wayland.then(|| Fix {
                    label: "Enable PROTON_ENABLE_WAYLAND=1".to_string(),
                    disable: Vec::new(),
                    enable: vec![("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string())],
                }),
            })
        },
    },
    // gamescope nested in a native-Wayland session. No fix: either is valid.
    Rule {
        id: "gamescope-vs-wayland",
        keys: &["gamescope", "PROTON_ENABLE_WAYLAND"],
        prefixes: &[],
        check: |c| {
            if !(c.wrap_on("gamescope") && c.env_on("PROTON_ENABLE_WAYLAND")) {
                return None;
            }
            Some(Notice {
                id: "gamescope-vs-wayland".to_string(),
                severity: Severity::Warning,
                message: "gamescope and PROTON_ENABLE_WAYLAND together can conflict — usually pick one.".to_string(),
                keys: vec!["gamescope".to_string(), "PROTON_ENABLE_WAYLAND".to_string()],
                fix: None,
            })
        },
    },
    // gplasync vs kernel anti-cheat: this one gets accounts banned. Fires on
    // the anti-cheat runtime switches *or* on anti-cheat found in the game's
    // own folder — the second catches it before the user ever enables EAC.
    Rule {
        id: "gplasync-anticheat",
        keys: &[
            "PROTON_DXVK_GPLASYNC",
            "PROTON_EAC_RUNTIME",
            "PROTON_BATTLEYE_RUNTIME",
        ],
        prefixes: &[],
        check: |c| {
            let anticheat = c.envs_on(&["PROTON_EAC_RUNTIME", "PROTON_BATTLEYE_RUNTIME"]);
            if !c.env_on("PROTON_DXVK_GPLASYNC") || (anticheat.is_empty() && c.anticheat.is_empty()) {
                return None;
            }
            Some(Notice {
                id: "gplasync-anticheat".to_string(),
                severity: Severity::Error,
                message: if c.anticheat.is_empty() {
                    "PROTON_DXVK_GPLASYNC can trip kernel anti-cheat — avoid it in EAC/BattlEye games.".to_string()
                } else {
                    format!(
                        "This game ships {} — PROTON_DXVK_GPLASYNC can trip it and risk a ban.",
                        c.anticheat.join(" and ")
                    )
                },
                keys: std::iter::once("PROTON_DXVK_GPLASYNC".to_string()).chain(anticheat).collect(),
                fix: Some(Fix {
                    label: "Disable PROTON_DXVK_GPLASYNC".to_string(),
                    disable: vec!["PROTON_DXVK_GPLASYNC".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // Both switch power profile / CPU governor for the game's lifetime, and
    // restore their own idea of "before" on exit — stacked, they fight.
    Rule {
        id: "game-performance-with-gamemode",
        keys: &["game-performance", "gamemoderun"],
        prefixes: &[],
        check: |c| {
            if !(c.wrap_on("game-performance") && c.wrap_on("gamemoderun")) {
                return None;
            }
            Some(Notice {
                id: "game-performance-with-gamemode".to_string(),
                severity: Severity::Warning,
                message: "game-performance and gamemoderun both switch the power profile for the game — they undo each other on exit. Keep one; on CachyOS, game-performance."
                    .to_string(),
                keys: vec!["game-performance".to_string(), "gamemoderun".to_string()],
                fix: Some(Fix {
                    label: "Disable gamemoderun".to_string(),
                    disable: vec!["gamemoderun".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // prime-run is a script that exports exactly these. Harmless, but noise.
    Rule {
        id: "prime-run-with-prime-env",
        keys: PRIME_KEYS_AND_WRAPPER,
        prefixes: &[],
        check: |c| {
            let manual = c.envs_on(PRIME_ENV_KEYS);
            if !c.wrap_on("prime-run") || manual.is_empty() {
                return None;
            }
            Some(Notice {
                id: "prime-run-with-prime-env".to_string(),
                severity: Severity::Info,
                message: "prime-run already sets the PRIME offload variables — the manual ones are redundant.".to_string(),
                keys: std::iter::once("prime-run".to_string()).chain(manual.clone()).collect(),
                fix: Some(Fix {
                    label: "Drop the manual PRIME variables".to_string(),
                    disable: manual,
                    enable: Vec::new(),
                }),
            })
        },
    },
    // The mangohud wrapper is an LD_PRELOAD layer *inside* the nested session;
    // gamescope draws its own overlay via --mangoapp, which is the supported way.
    Rule {
        id: "mangohud-inside-gamescope",
        keys: &["mangohud", "gamescope"],
        prefixes: &[],
        check: |c| {
            if !(c.wrap_on("mangohud") && c.wrap_on("gamescope")) {
                return None;
            }
            let both = c.gamescope_has("--mangoapp");
            Some(Notice {
                id: "mangohud-inside-gamescope".to_string(),
                severity: Severity::Warning,
                message: if both {
                    "gamescope's --mangoapp already draws MangoHud — the mangohud wrapper adds a second overlay."
                } else {
                    "Inside gamescope, MangoHud works better as gamescope's --mangoapp flag than as the mangohud wrapper."
                }
                .to_string(),
                keys: vec!["mangohud".to_string(), "gamescope".to_string()],
                // Only safe to automate when --mangoapp is already there:
                // otherwise disabling the wrapper just loses the overlay.
                fix: both.then(|| Fix {
                    label: "Disable the mangohud wrapper".to_string(),
                    disable: vec!["mangohud".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // DXVK_HDR under gamescope only reaches the display with --hdr-enabled.
    Rule {
        id: "gamescope-hdr-flag",
        keys: &["DXVK_HDR", "gamescope"],
        prefixes: &[],
        check: |c| {
            if !c.env_on("DXVK_HDR") || !c.wrap_on("gamescope") || c.gamescope_has("--hdr-enabled") {
                return None;
            }
            Some(Notice {
                id: "gamescope-hdr-flag".to_string(),
                severity: Severity::Warning,
                message: "DXVK_HDR under gamescope needs --hdr-enabled in gamescope's arguments, or the game stays SDR."
                    .to_string(),
                keys: vec!["DXVK_HDR".to_string(), "gamescope".to_string()],
                fix: None,
            })
        },
    },
    // OptiScaler is an injected DLL; kernel anti-cheat in the game's own folder
    // can flag it. Only from the scan — the env switches alone say too little.
    Rule {
        id: "optiscaler-anticheat",
        keys: &["PROTON_USE_OPTISCALER"],
        prefixes: &[],
        check: |c| {
            if !c.flag_on("PROTON_USE_OPTISCALER") || c.anticheat.is_empty() {
                return None;
            }
            Some(Notice {
                id: "optiscaler-anticheat".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "This game ships {} — injecting OptiScaler can get flagged online. Keep it to offline/single-player.",
                    c.anticheat.join(" and ")
                ),
                keys: vec!["PROTON_USE_OPTISCALER".to_string()],
                fix: Some(Fix {
                    label: "Disable OptiScaler".to_string(),
                    disable: vec!["PROTON_USE_OPTISCALER".to_string()],
                    enable: Vec::new(),
                }),
            })
        },
    },
    // Two different DXVK forks. No fix: which one to keep is the user's call.
    Rule {
        id: "dxvk-fork-conflict",
        keys: &["PROTON_DXVK_GPLASYNC", "PROTON_DXVK_LOWLATENCY"],
        prefixes: &[],
        check: |c| {
            if !(c.env_on("PROTON_DXVK_GPLASYNC") && c.env_on("PROTON_DXVK_LOWLATENCY")) {
                return None;
            }
            Some(Notice {
                id: "dxvk-fork-conflict".to_string(),
                severity: Severity::Error,
                message: "PROTON_DXVK_GPLASYNC and PROTON_DXVK_LOWLATENCY are different DXVK forks — enable only one.".to_string(),
                keys: vec![
                    "PROTON_DXVK_GPLASYNC".to_string(),
                    "PROTON_DXVK_LOWLATENCY".to_string(),
                ],
                fix: None,
            })
        },
    },
];

/// Produce structured notices for the current selection.
///
/// `gpu_gen` is the user's declared AMD generation from the settings store —
/// see [`Ctx::gpu_gen`].
/// The AMD generation lint should assume — the same answer the frontend's
/// `effectiveGpuGen` + `hwCaps` give the visibility filter: the declared
/// generation, else the detected one, and nothing at all off AMD hardware (a
/// `state.toml` carried to an NVIDIA box keeps its `gpu_gen`). If the two ever
/// disagree, lint offers fixes for rows the filter hides.
pub fn effective_gpu_gen(declared: &str, detected: Option<&str>, amd: bool) -> String {
    if !amd {
        return String::new();
    }
    if declared.is_empty() { detected.unwrap_or_default() } else { declared }.to_string()
}

/// A notice for custom-env tokens that aren't shell assignments (see
/// `compose::invalid_extra_env`) — they're left out of the command, and the
/// user should know why their `A-B=x` did nothing.
pub fn invalid_custom_env(tokens: &[String]) -> Option<Notice> {
    if tokens.is_empty() {
        return None;
    }
    Some(Notice {
        id: "custom-env-invalid".to_string(),
        severity: Severity::Warning,
        message: format!(
            "Custom env {} left out: {} — a variable name is letters, digits and _, not starting with a digit.",
            if tokens.len() == 1 { "entry" } else { "entries" },
            tokens.join(" ")
        ),
        keys: Vec::new(),
        fix: None,
    })
}

/// Enabled rows whose program isn't installed. A missing *wrapper* stops the
/// game launching at all — Steam runs `mangohud %command%`, the shell can't
/// find `mangohud`, and the game silently never starts — so this is an error,
/// not a hint. `installed` answers for a `requires` name (the caller resolves
/// Settings → Paths overrides).
///
/// Lives outside [`RULES`] because the keys it reports are whatever the
/// catalog marks `requires`, not a fixed list a rule could declare.
pub fn missing_programs(
    catalog: &Catalog,
    options: &Options,
    installed: impl Fn(&str) -> bool,
) -> Option<Notice> {
    let wrappers = catalog.wrappers.iter().zip(&options.wrappers).map(|(d, s)| (s.enabled, &d.key, &d.requires, &d.pkg));
    let envs = catalog.envs.iter().zip(&options.envs).map(|(d, s)| (s.enabled, &d.key, &d.requires, &d.pkg));
    let missing: Vec<(&String, &String, &Option<String>)> = wrappers
        .chain(envs)
        .filter_map(|(on, key, req, pkg)| {
            let req = req.as_ref()?;
            (on && !installed(req)).then_some((key, req, pkg))
        })
        .collect();
    if missing.is_empty() {
        return None;
    }
    let programs: Vec<&str> = missing.iter().map(|(_, r, _)| r.as_str()).collect();
    let mut pkgs: Vec<&str> = missing.iter().filter_map(|(_, _, p)| p.as_deref()).collect();
    pkgs.sort_unstable();
    pkgs.dedup();
    let keys: Vec<String> = missing.iter().map(|(k, _, _)| (*k).clone()).collect();
    Some(Notice {
        id: "program-not-installed".to_string(),
        severity: Severity::Error,
        message: format!(
            "{} {} not installed — the game won't start with {} in the command.{}",
            programs.join(", "),
            if programs.len() == 1 { "is" } else { "are" },
            if programs.len() == 1 { "it" } else { "them" },
            if pkgs.is_empty() { String::new() } else { format!(" Install: sudo pacman -S {}", pkgs.join(" ")) },
        ),
        fix: Some(Fix {
            label: if keys.len() == 1 { format!("Disable {}", keys[0]) } else { "Disable them".to_string() },
            disable: keys.clone(),
            enable: Vec::new(),
        }),
        keys,
    })
}

/// `game_files` are the selected game's manual-OptiScaler leftovers, empty when
/// no game is selected or the folder wasn't scanned; `anticheat` is what its
/// folder scan found (see [`Ctx::anticheat`]).
pub fn warnings(
    catalog: &Catalog,
    options: &Options,
    hw: &Hardware,
    gpu_gen: &str,
    game_files: &[String],
    anticheat: &[String],
) -> Vec<Notice> {
    let ctx = Ctx { catalog, options, hw, gpu_gen, game_files, anticheat };
    RULES.iter().filter_map(|r| (r.check)(&ctx)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_gen_falls_back_to_detection_and_is_ignored_off_amd() {
        assert_eq!(effective_gpu_gen("", Some("rdna4"), true), "rdna4");
        assert_eq!(effective_gpu_gen("rdna3", Some("rdna4"), true), "rdna3");
        assert_eq!(effective_gpu_gen("", None, true), "");
        assert_eq!(effective_gpu_gen("rdna3", Some("rdna3"), false), "");
    }
    use crate::params::Catalog;

    fn enable(cat: &Catalog, opts: &mut Options, key: &str, val: &str) {
        if let Some(i) = cat.envs.iter().position(|e| e.key == key) {
            opts.envs[i].enabled = true;
            opts.envs[i].value = val.to_string();
            return;
        }
        if let Some(i) = cat.wrappers.iter().position(|w| w.key == key) {
            opts.wrappers[i].enabled = true;
            opts.wrappers[i].value = val.to_string();
            return;
        }
        panic!("{key} is not in the bundled catalog");
    }

    /// Lint the bundled catalog with `keys` enabled, under `hw`, with no
    /// declared GPU generation.
    fn lint_with(hw: Hardware, keys: &[&str]) -> Vec<Notice> {
        lint_gen(hw, "", keys)
    }

    /// As [`lint_with`], with a declared AMD generation ("rdna3" | "rdna4").
    fn lint_gen(hw: Hardware, gpu_gen: &str, keys: &[&str]) -> Vec<Notice> {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        for k in keys {
            enable(&cat, &mut opts, k, "1");
        }
        warnings(&cat, &opts, &hw, gpu_gen, &[], &[])
    }

    fn find<'a>(notices: &'a [Notice], id: &str) -> Option<&'a Notice> {
        notices.iter().find(|n| n.id == id)
    }

    #[test]
    fn flags_nvapi_without_nvidia() {
        let hw = Hardware { nvidia: false, ..Default::default() };
        let n = lint_with(hw, &["PROTON_FORCE_NVAPI"]);
        let notice = find(&n, "nvapi-without-nvidia").expect("rule fires");
        assert_eq!(notice.severity, Severity::Info);
        assert_eq!(notice.keys, vec!["PROTON_FORCE_NVAPI"]);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.disable.clone()),
            Some(vec!["PROTON_FORCE_NVAPI".to_string()])
        );

        // With NVIDIA present the rule goes quiet.
        let hw2 = Hardware { nvidia: true, ..Default::default() };
        assert!(find(&lint_with(hw2, &["PROTON_FORCE_NVAPI"]), "nvapi-without-nvidia").is_none());
    }

    #[test]
    fn fsr4_note_only_fires_without_a_declared_generation() {
        let amd = Hardware { amd: true, ..Default::default() };
        let notice = find(
            &lint_gen(amd.clone(), "", &["PROTON_FSR4_UPGRADE"]),
            "fsr4-hardware-note",
        )
        .expect("rule fires when the generation is unknown")
        .clone();
        assert_eq!(notice.severity, Severity::Info);
        assert!(notice.fix.is_none(), "nothing to apply — we're asking a question");

        // Once the user has told us, the generic both-generations sentence is
        // pure noise and the specific rules take over.
        // `gen` is a reserved keyword in edition 2024.
        for generation in ["rdna3", "rdna4"] {
            assert!(
                find(
                    &lint_gen(amd.clone(), generation, &["PROTON_FSR4_UPGRADE"]),
                    "fsr4-hardware-note"
                )
                .is_none(),
                "generic note should be silent on {generation}"
            );
        }
    }

    #[test]
    fn offers_the_rdna3_mlfg_workaround() {
        let amd = Hardware { amd: true, ..Default::default() };
        let notice = find(
            &lint_gen(amd.clone(), "rdna3", &["PROTON_FSR4_UPGRADE", "PROTON_MLFG_UPGRADE"]),
            "rdna3-mlfg-workaround",
        )
        .expect("rule fires")
        .clone();
        assert_eq!(notice.severity, Severity::Warning);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.enable.clone()),
            Some(vec![(
                "DXIL_SPIRV_CONFIG".to_string(),
                "wmma_rdna3_workaround".to_string()
            )])
        );

        // Applying the fix silences it.
        let fixed = lint_gen(
            amd.clone(),
            "rdna3",
            &["PROTON_FSR4_UPGRADE", "PROTON_MLFG_UPGRADE", "DXIL_SPIRV_CONFIG"],
        );
        assert!(find(&fixed, "rdna3-mlfg-workaround").is_none());

        // MLFG is what needs it — FSR4 alone does not.
        let upscale_only = lint_gen(amd.clone(), "rdna3", &["PROTON_FSR4_UPGRADE"]);
        assert!(find(&upscale_only, "rdna3-mlfg-workaround").is_none());

        // And it is an RDNA3 remedy only.
        let on_rdna4 = lint_gen(amd, "rdna4", &["PROTON_FSR4_UPGRADE", "PROTON_MLFG_UPGRADE"]);
        assert!(find(&on_rdna4, "rdna3-mlfg-workaround").is_none());
    }

    #[test]
    fn flags_the_rdna3_workaround_as_a_noop_on_rdna4() {
        let amd = Hardware { amd: true, ..Default::default() };
        let notice = find(
            &lint_gen(amd.clone(), "rdna4", &["DXIL_SPIRV_CONFIG"]),
            "rdna4-workaround-noop",
        )
        .expect("rule fires")
        .clone();
        assert_eq!(notice.severity, Severity::Info);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.disable.clone()),
            Some(vec!["DXIL_SPIRV_CONFIG".to_string()])
        );

        // On RDNA3 it is the correct setting, and with no declared generation we
        // have no grounds to call it useless.
        assert!(
            find(&lint_gen(amd.clone(), "rdna3", &["DXIL_SPIRV_CONFIG"]), "rdna4-workaround-noop")
                .is_none()
        );
        assert!(
            find(&lint_gen(amd, "", &["DXIL_SPIRV_CONFIG"]), "rdna4-workaround-noop").is_none()
        );
    }

    #[test]
    fn flags_optiscaler_config_without_injection() {
        let n = lint_with(Hardware::default(), &["PROTON_OPTISCALER_CONFIG"]);
        let notice = find(&n, "optiscaler-not-injected").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.enable.clone()),
            Some(vec![("PROTON_USE_OPTISCALER".to_string(), "1".to_string())])
        );

        // Injection on: nothing to say.
        let injected =
            lint_with(Hardware::default(), &["PROTON_OPTISCALER_CONFIG", "PROTON_USE_OPTISCALER"]);
        assert!(find(&injected, "optiscaler-not-injected").is_none());

        // Injection alone is a valid, complete setup.
        let bare = lint_with(Hardware::default(), &["PROTON_USE_OPTISCALER"]);
        assert!(find(&bare, "optiscaler-not-injected").is_none());
    }

    /// Lint with explicit `(key, value)` pairs and a game folder's leftovers.
    fn lint_vals(gpu_gen: &str, vals: &[(&str, &str)], game_files: &[String]) -> Vec<Notice> {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        for (k, v) in vals {
            enable(&cat, &mut opts, k, v);
        }
        let amd = Hardware { amd: true, ..Default::default() };
        warnings(&cat, &opts, &amd, gpu_gen, game_files, &[])
    }

    #[test]
    fn flags_mlfg_as_a_noop_under_optiscaler() {
        let both = [("PROTON_MLFG_UPGRADE", "1"), ("PROTON_USE_OPTISCALER", "1")];
        let n = lint_vals("rdna3", &both, &[]);
        let notice = find(&n, "mlfg-ignored-with-optiscaler").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.disable.clone()),
            Some(vec!["PROTON_MLFG_UPGRADE".to_string()])
        );
        // No RDNA3 workaround advice for a variable that does nothing.
        assert!(find(&n, "rdna3-mlfg-workaround").is_none());

        // `=0` is off — the shape a real launch line toggles it with.
        let zero = lint_vals("", &[("PROTON_MLFG_UPGRADE", "0"), ("PROTON_USE_OPTISCALER", "1")], &[]);
        assert!(find(&zero, "mlfg-ignored-with-optiscaler").is_none());
        // Without OptiScaler, MLFG is Proton's to apply.
        let alone = lint_vals("", &[("PROTON_MLFG_UPGRADE", "1")], &[]);
        assert!(find(&alone, "mlfg-ignored-with-optiscaler").is_none());
    }

    #[test]
    fn flags_stacked_latency_layers() {
        let one = lint_vals("", &[("ENABLE_LAYER_MESA_ANTI_LAG", "1")], &[]);
        assert!(find(&one, "latency-layers-stacked").is_none());

        let two = lint_vals("", &[("ENABLE_LAYER_MESA_ANTI_LAG", "1"), ("LOW_LATENCY_LAYER", "1")], &[]);
        let notice = find(&two, "latency-layers-stacked").expect("rule fires");
        assert_eq!(notice.keys, vec!["ENABLE_LAYER_MESA_ANTI_LAG", "LOW_LATENCY_LAYER"]);
        assert!(notice.fix.is_none(), "which one to keep is the user's call");

        let off = lint_vals("", &[("ENABLE_LAYER_MESA_ANTI_LAG", "1"), ("LOW_LATENCY_LAYER", "0")], &[]);
        assert!(find(&off, "latency-layers-stacked").is_none());
    }

    #[test]
    fn flags_a_latency_layer_under_optiscaler() {
        let n = lint_vals(
            "",
            &[("ENABLE_LAYER_MESA_ANTI_LAG", "1"), ("PROTON_USE_OPTISCALER", "1")],
            &[],
        );
        let notice = find(&n, "latency-layer-with-optiscaler").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        let fix = notice.fix.as_ref().expect("has a fix");
        assert_eq!(fix.label, "Turn off ENABLE_LAYER_MESA_ANTI_LAG");
        assert_eq!(fix.disable, vec!["ENABLE_LAYER_MESA_ANTI_LAG"]);

        // Either half alone is fine.
        let layer = lint_vals("", &[("ENABLE_LAYER_MESA_ANTI_LAG", "1")], &[]);
        assert!(find(&layer, "latency-layer-with-optiscaler").is_none());
        let opti = lint_vals("", &[("PROTON_USE_OPTISCALER", "1")], &[]);
        assert!(find(&opti, "latency-layer-with-optiscaler").is_none());
    }

    #[test]
    fn flags_a_manual_optiscaler_beside_the_injected_one() {
        let files: Vec<String> = ["dxgi.dll", "OptiScaler.ini", "OptiScaler", "fakenvapi.dll", "OptiScaler.log"]
            .map(String::from)
            .to_vec();
        let n = lint_vals("", &[("PROTON_USE_OPTISCALER", "1")], &files);
        let notice = find(&n, "optiscaler-double-install").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        assert!(notice.message.contains("dxgi.dll, OptiScaler.ini, OptiScaler, fakenvapi.dll +1 more"));
        assert!(notice.fix.is_none());

        // A manual install on its own is a legitimate setup.
        assert!(find(&lint_vals("", &[], &files), "optiscaler-double-install").is_none());
        assert!(
            find(&lint_vals("", &[("PROTON_USE_OPTISCALER", "0")], &files), "optiscaler-double-install")
                .is_none()
        );
        // And a clean folder has nothing to report.
        assert!(
            find(&lint_vals("", &[("PROTON_USE_OPTISCALER", "1")], &[]), "optiscaler-double-install")
                .is_none()
        );
    }

    #[test]
    fn lsfg_settings_need_env_mode() {
        let n = lint_with(Hardware::default(), &["LSFGVK_MULTIPLIER", "LSFGVK_FLOW_SCALE"]);
        let notice = find(&n, "lsfg-needs-env-mode").expect("rule fires");
        assert_eq!(notice.keys, vec!["LSFGVK_MULTIPLIER", "LSFGVK_FLOW_SCALE"]);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.enable.clone()),
            Some(vec![("LSFGVK_ENV".to_string(), "1".to_string())])
        );
        let env = lint_with(Hardware::default(), &["LSFGVK_MULTIPLIER", "LSFGVK_ENV"]);
        assert!(find(&env, "lsfg-needs-env-mode").is_none());
        // A profile alone is config-file mode, which is fine.
        assert!(find(&lint_with(Hardware::default(), &["LSFGVK_PROFILE"]), "lsfg-needs-env-mode").is_none());
    }

    #[test]
    fn lsfg_profile_conflicts_with_env_mode_and_disable_wins() {
        let n = lint_with(Hardware::default(), &["LSFGVK_PROFILE", "LSFGVK_ENV"]);
        assert!(find(&n, "lsfg-profile-in-env-mode").is_some());

        let n = lint_with(Hardware::default(), &["DISABLE_LSFGVK", "LSFGVK_PROFILE"]);
        let notice = find(&n, "lsfg-disabled").expect("rule fires");
        assert_eq!(notice.keys, vec!["DISABLE_LSFGVK", "LSFGVK_PROFILE"]);
        assert!(find(&lint_with(Hardware::default(), &["DISABLE_LSFGVK"]), "lsfg-disabled").is_none());
    }

    #[test]
    fn flags_optifg_stacked_under_lsfg_and_fixes_only_the_framegen_entries() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        enable(&cat, &mut opts, "LSFGVK_PROFILE", "2x FG / 100%");
        enable(&cat, &mut opts, "PROTON_USE_OPTISCALER", "1");
        enable(
            &cat,
            &mut opts,
            "PROTON_OPTISCALER_CONFIG",
            "Upscalers.Dx12Upscaler=ffx;FrameGen.Enabled=true;FrameGen.FGInput=fsrfg",
        );
        let n = warnings(&cat, &opts, &Hardware::default(), "", &[], &[]);
        let notice = find(&n, "lsfg-double-framegen").expect("rule fires");
        let fix = notice.fix.as_ref().expect("has a fix");
        assert_eq!(
            fix.enable,
            vec![(
                "PROTON_OPTISCALER_CONFIG".to_string(),
                "Upscalers.Dx12Upscaler=ffx;FrameGen.Enabled=false".to_string()
            )]
        );

        // Upscaling-only OptiScaler is the intended pairing.
        enable(&cat, &mut opts, "PROTON_OPTISCALER_CONFIG", "Upscalers.Dx12Upscaler=ffx");
        assert!(find(&warnings(&cat, &opts, &Hardware::default(), "", &[], &[]), "lsfg-double-framegen").is_none());

        // And the fix's own output doesn't re-trigger the rule.
        assert!(!optifg_enabled(&without_optifg("FrameGen.Enabled=true")));
        assert_eq!(without_optifg("FrameGen.Enabled=true"), "FrameGen.Enabled=false");
    }

    #[test]
    fn flags_wined3d_only_when_dxvk_is_on() {
        // wined3d alone is a deliberate choice, not a conflict.
        let alone = lint_with(Hardware::default(), &["PROTON_USE_WINED3D"]);
        assert!(find(&alone, "wined3d-disables-dxvk").is_none());

        let both = lint_with(Hardware::default(), &["PROTON_USE_WINED3D", "DXVK_HUD"]);
        let notice = find(&both, "wined3d-disables-dxvk").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        // The implicated DXVK_* keys come along so the UI can highlight them.
        assert!(notice.keys.contains(&"DXVK_HUD".to_string()));
        assert_eq!(
            notice.fix.as_ref().map(|f| f.disable.clone()),
            Some(vec!["PROTON_USE_WINED3D".to_string()])
        );
    }

    #[test]
    fn flags_hdr_without_a_presentation_path() {
        let wayland = Hardware { wayland: true, ..Default::default() };
        let n = lint_with(wayland, &["DXVK_HDR"]);
        let notice = find(&n, "hdr-needs-presentation").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        assert_eq!(
            notice.fix.as_ref().map(|f| f.enable.clone()),
            Some(vec![("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string())])
        );

        // Under X11 the rule still warns, but offers no Wayland fix.
        let x11 = lint_with(Hardware::default(), &["DXVK_HDR"]);
        assert!(find(&x11, "hdr-needs-presentation").unwrap().fix.is_none());

        // Either presentation path silences it.
        let via_wayland = lint_with(
            Hardware { wayland: true, ..Default::default() },
            &["DXVK_HDR", "PROTON_ENABLE_WAYLAND"],
        );
        assert!(find(&via_wayland, "hdr-needs-presentation").is_none());
        let via_gamescope = lint_with(Hardware::default(), &["DXVK_HDR", "gamescope"]);
        assert!(find(&via_gamescope, "hdr-needs-presentation").is_none());
    }

    #[test]
    fn flags_gamescope_against_native_wayland() {
        let n = lint_with(
            Hardware { wayland: true, ..Default::default() },
            &["gamescope", "PROTON_ENABLE_WAYLAND"],
        );
        let notice = find(&n, "gamescope-vs-wayland").expect("rule fires");
        assert_eq!(notice.severity, Severity::Warning);
        // Either choice is legitimate, so there's no "correct" fix to offer.
        assert!(notice.fix.is_none());
    }

    #[test]
    fn flags_gplasync_anticheat() {
        let n = lint_with(
            Hardware::default(),
            &["PROTON_DXVK_GPLASYNC", "PROTON_EAC_RUNTIME"],
        );
        let notice = find(&n, "gplasync-anticheat").expect("rule fires");
        assert_eq!(notice.severity, Severity::Error);
        assert!(notice.keys.contains(&"PROTON_EAC_RUNTIME".to_string()));
        assert_eq!(
            notice.fix.as_ref().map(|f| f.disable.clone()),
            Some(vec!["PROTON_DXVK_GPLASYNC".to_string()])
        );
    }

    #[test]
    fn flags_conflicting_dxvk_forks() {
        let n = lint_with(
            Hardware::default(),
            &["PROTON_DXVK_GPLASYNC", "PROTON_DXVK_LOWLATENCY"],
        );
        let notice = find(&n, "dxvk-fork-conflict").expect("rule fires");
        assert_eq!(notice.severity, Severity::Error);
        // Which fork to keep is the user's call.
        assert!(notice.fix.is_none());
    }

    fn lint_wrap(keys: &[(&str, &str)]) -> Vec<Notice> {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        for (k, v) in keys {
            enable(&cat, &mut opts, k, v);
        }
        warnings(&cat, &opts, &Hardware::default(), "", &[], &[])
    }

    #[test]
    fn flags_game_performance_stacked_with_gamemode() {
        let n = lint_wrap(&[("game-performance", ""), ("gamemoderun", "")]);
        let notice = find(&n, "game-performance-with-gamemode").expect("fires");
        assert_eq!(notice.fix.as_ref().unwrap().disable, vec!["gamemoderun"]);
        assert!(find(&lint_wrap(&[("gamemoderun", "")]), "game-performance-with-gamemode").is_none());
    }

    #[test]
    fn flags_manual_prime_vars_under_prime_run() {
        let n = lint_wrap(&[("prime-run", ""), ("__NV_PRIME_RENDER_OFFLOAD", "1")]);
        let notice = find(&n, "prime-run-with-prime-env").expect("fires");
        assert_eq!(notice.fix.as_ref().unwrap().disable, vec!["__NV_PRIME_RENDER_OFFLOAD"]);
        assert!(find(&lint_wrap(&[("__NV_PRIME_RENDER_OFFLOAD", "1")]), "prime-run-with-prime-env").is_none());
    }

    #[test]
    fn mangohud_wrapper_inside_gamescope_suggests_mangoapp() {
        let n = lint_wrap(&[("mangohud", ""), ("gamescope", "-f")]);
        let notice = find(&n, "mangohud-inside-gamescope").expect("fires");
        assert!(notice.message.contains("--mangoapp"));
        assert!(notice.fix.is_none(), "no fix without --mangoapp: it would just drop the overlay");

        let n = lint_wrap(&[("mangohud", ""), ("gamescope", "-f --mangoapp")]);
        let notice = find(&n, "mangohud-inside-gamescope").expect("fires");
        assert!(notice.fix.is_some(), "double overlay: dropping the wrapper is safe");
    }

    #[test]
    fn gamescope_hdr_needs_the_flag() {
        assert!(find(&lint_wrap(&[("DXVK_HDR", "1"), ("gamescope", "-f")]), "gamescope-hdr-flag").is_some());
        assert!(
            find(&lint_wrap(&[("DXVK_HDR", "1"), ("gamescope", "-f --hdr-enabled")]), "gamescope-hdr-flag").is_none()
        );
        assert!(find(&lint_wrap(&[("gamescope", "-f")]), "gamescope-hdr-flag").is_none());
    }

    #[test]
    fn missing_program_is_an_error_with_install_hint() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        enable(&cat, &mut opts, "mangohud", "");
        enable(&cat, &mut opts, "gamemoderun", "");
        let n = missing_programs(&cat, &opts, |p| p != "mangohud").expect("fires");
        assert_eq!(n.severity, Severity::Error);
        assert_eq!(n.keys, vec!["mangohud"]);
        assert!(n.message.contains("pacman -S mangohud"), "{}", n.message);
        assert!(missing_programs(&cat, &opts, |_| true).is_none());
        // Disabled rows don't count, installed or not.
        assert!(missing_programs(&cat, &Options::from_catalog(&cat), |_| false).is_none());
    }

    #[test]
    fn shipped_anticheat_drives_gplasync_and_optiscaler_rules() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        enable(&cat, &mut opts, "PROTON_DXVK_GPLASYNC", "1");
        enable(&cat, &mut opts, "PROTON_USE_OPTISCALER", "1");
        let eac = vec!["EasyAntiCheat".to_string()];
        let n = warnings(&cat, &opts, &Hardware::default(), "", &[], &eac);
        let g = find(&n, "gplasync-anticheat").expect("fires from the scan alone");
        assert!(g.message.contains("EasyAntiCheat"));
        assert!(find(&n, "optiscaler-anticheat").is_some());
        let none = warnings(&cat, &opts, &Hardware::default(), "", &[], &[]);
        assert!(find(&none, "gplasync-anticheat").is_none());
        assert!(find(&none, "optiscaler-anticheat").is_none());
    }

    #[test]
    fn every_rule_has_a_unique_id() {
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate rule id in RULES");
        assert!(!ids.iter().any(|id| id.is_empty()), "empty rule id");
    }

    /// The load-bearing guard. `params.toml` is rewritten by the
    /// `update-proton-params` skill; when a key is renamed or dropped, a rule
    /// that still names the old one silently stops matching and its
    /// click-to-jump links point at nothing. That has already happened twice
    /// (`PROTON_ENABLE_NVAPI` renamed, `PROTON_ENABLE_HDR` removed), so it is
    /// checked rather than trusted.
    #[test]
    fn all_notice_keys_exist_in_bundled_catalog() {
        let cat = Catalog::bundled();
        let known = |k: &str| {
            cat.envs.iter().any(|e| e.key == k) || cat.wrappers.iter().any(|w| w.key == k)
        };

        for rule in RULES {
            assert!(!rule.keys.is_empty(), "rule {} declares no keys", rule.id);
            for key in rule.keys {
                assert!(
                    known(key),
                    "rule {} depends on {key}, which is no longer in the bundled catalog",
                    rule.id
                );
            }
        }

        // And the emitted keys must stay inside what the rule declared, so the
        // check above actually covers what the UI receives. Drive every rule
        // with the whole catalog enabled, under both a bare and a fully-featured
        // machine — and under every GPU generation, or the generation-gated
        // rules would never fire here and go unchecked.
        let mut opts = Options::from_catalog(&cat);
        for e in opts.envs.iter_mut() {
            e.enabled = true;
        }
        // And a dirty game folder, or the folder-gated rule never fires here.
        let game_files = vec!["OptiScaler.dll".to_string()];
        // And shipped anti-cheat, or the scan-gated rules never fire here.
        let shipped = vec!["EasyAntiCheat".to_string()];
        for w in opts.wrappers.iter_mut() {
            w.enabled = true;
        }
        let hws = [
            Hardware::default(),
            Hardware {
                nvidia: true,
                amd: true,
                intel: true,
                wayland: true,
                kde: true,
                ntsync: true,
                ..Default::default()
            },
        ];
        for hw in hws {
            for gpu_gen in ["", "rdna3", "rdna4"] {
                let ctx = Ctx {
                    catalog: &cat,
                    options: &opts,
                    hw: &hw,
                    gpu_gen,
                    game_files: &game_files,
                    anticheat: &shipped,
                };
                for rule in RULES {
                    let Some(notice) = (rule.check)(&ctx) else { continue };
                    assert_eq!(notice.id, rule.id, "rule {} emits a mismatched id", rule.id);
                    let emitted = notice.keys.iter().chain(
                        notice
                            .fix
                            .iter()
                            .flat_map(|f| f.disable.iter().chain(f.enable.iter().map(|(k, _)| k))),
                    );
                    for key in emitted {
                        let declared = rule.keys.contains(&key.as_str())
                            || rule.prefixes.iter().any(|p| key.starts_with(p));
                        assert!(declared, "rule {} emits undeclared key {key}", rule.id);
                    }
                }
            }
        }
    }
}
