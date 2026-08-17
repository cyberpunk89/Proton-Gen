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
pub struct Fix {
    /// Button text, e.g. "Disable PROTON_USE_WINED3D".
    pub label: String,
    /// Catalog keys (env or wrapper) to turn off.
    pub disable: Vec<String>,
    /// Catalog env keys to turn on, with the value to set.
    pub enable: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize)]
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
}

impl Ctx<'_> {
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

    /// Which of `keys` are currently enabled, in the given order.
    fn envs_on(&self, keys: &[&str]) -> Vec<String> {
        keys.iter()
            .filter(|k| self.env_on(k))
            .map(|k| k.to_string())
            .collect()
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
    Rule {
        id: "rdna3-mlfg-workaround",
        keys: &["PROTON_MLFG_UPGRADE", "DXIL_SPIRV_CONFIG"],
        prefixes: &[],
        check: |c| {
            if c.gpu_gen != "rdna3"
                || !c.env_on("PROTON_MLFG_UPGRADE")
                || c.env_on("DXIL_SPIRV_CONFIG")
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
    // gplasync vs kernel anti-cheat: this one gets accounts banned.
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
            if !c.env_on("PROTON_DXVK_GPLASYNC") || anticheat.is_empty() {
                return None;
            }
            Some(Notice {
                id: "gplasync-anticheat".to_string(),
                severity: Severity::Error,
                message: "PROTON_DXVK_GPLASYNC can trip kernel anti-cheat — avoid it in EAC/BattlEye games.".to_string(),
                keys: std::iter::once("PROTON_DXVK_GPLASYNC".to_string()).chain(anticheat).collect(),
                fix: Some(Fix {
                    label: "Disable PROTON_DXVK_GPLASYNC".to_string(),
                    disable: vec!["PROTON_DXVK_GPLASYNC".to_string()],
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
pub fn warnings(catalog: &Catalog, options: &Options, hw: &Hardware, gpu_gen: &str) -> Vec<Notice> {
    let ctx = Ctx { catalog, options, hw, gpu_gen };
    RULES.iter().filter_map(|r| (r.check)(&ctx)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        warnings(&cat, &opts, &hw, gpu_gen)
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
            &lint_gen(amd, "", &["PROTON_FSR4_UPGRADE"]),
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
                find(&lint_gen(amd, generation, &["PROTON_FSR4_UPGRADE"]), "fsr4-hardware-note")
                    .is_none(),
                "generic note should be silent on {generation}"
            );
        }
    }

    #[test]
    fn offers_the_rdna3_mlfg_workaround() {
        let amd = Hardware { amd: true, ..Default::default() };
        let notice = find(
            &lint_gen(amd, "rdna3", &["PROTON_FSR4_UPGRADE", "PROTON_MLFG_UPGRADE"]),
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
            amd,
            "rdna3",
            &["PROTON_FSR4_UPGRADE", "PROTON_MLFG_UPGRADE", "DXIL_SPIRV_CONFIG"],
        );
        assert!(find(&fixed, "rdna3-mlfg-workaround").is_none());

        // MLFG is what needs it — FSR4 alone does not.
        let upscale_only = lint_gen(amd, "rdna3", &["PROTON_FSR4_UPGRADE"]);
        assert!(find(&upscale_only, "rdna3-mlfg-workaround").is_none());

        // And it is an RDNA3 remedy only.
        let on_rdna4 = lint_gen(amd, "rdna4", &["PROTON_FSR4_UPGRADE", "PROTON_MLFG_UPGRADE"]);
        assert!(find(&on_rdna4, "rdna3-mlfg-workaround").is_none());
    }

    #[test]
    fn flags_the_rdna3_workaround_as_a_noop_on_rdna4() {
        let amd = Hardware { amd: true, ..Default::default() };
        let notice = find(
            &lint_gen(amd, "rdna4", &["DXIL_SPIRV_CONFIG"]),
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
            find(&lint_gen(amd, "rdna3", &["DXIL_SPIRV_CONFIG"]), "rdna4-workaround-noop")
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
            },
        ];
        for hw in hws {
            for gpu_gen in ["", "rdna3", "rdna4"] {
                let ctx = Ctx { catalog: &cat, options: &opts, hw: &hw, gpu_gen };
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
