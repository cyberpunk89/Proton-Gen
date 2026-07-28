//! Lightweight conflict / footgun detection over the enabled options.

use crate::hardware::Hardware;
use crate::params::{Catalog, Options};

/// Produce human-readable notices for the current selection.
pub fn warnings(catalog: &Catalog, options: &Options, hw: &Hardware) -> Vec<String> {
    // Helpers over the enabled set.
    let env_on = |key: &str| -> bool {
        catalog
            .envs
            .iter()
            .zip(&options.envs)
            .any(|(d, s)| s.enabled && d.key == key)
    };
    let wrap_on = |key: &str| -> bool {
        catalog
            .wrappers
            .iter()
            .zip(&options.wrappers)
            .any(|(d, s)| s.enabled && d.key == key)
    };

    let mut w = Vec::new();

    // NVAPI / DLSS without an NVIDIA GPU.
    let nvapi = ["PROTON_FORCE_NVAPI", "DXVK_ENABLE_NVAPI", "PROTON_DLSS_UPGRADE", "DXVK_NVAPI_VKREFLEX", "PROTON_NVIDIA_LIBS"];
    if !hw.nvidia && nvapi.iter().any(|k| env_on(k)) {
        w.push("NVAPI/DLSS options are enabled but no NVIDIA GPU was detected — they'll have no effect.".to_string());
    }

    // FSR4 hardware note.
    if env_on("PROTON_FSR4_UPGRADE") {
        w.push("PROTON_FSR4_UPGRADE needs an FSR4-capable AMD GPU (RDNA3 or RDNA4); on RDNA3, MLFG also needs DXIL_SPIRV_CONFIG=wmma_rdna3_workaround.".to_string());
    }

    // wined3d disables DXVK.
    if env_on("PROTON_USE_WINED3D") {
        let dxvk = catalog
            .envs
            .iter()
            .zip(&options.envs)
            .any(|(d, s)| s.enabled && d.key.starts_with("DXVK_"));
        if dxvk {
            w.push("PROTON_USE_WINED3D routes D3D through OpenGL — your DXVK_* options won't apply.".to_string());
        }
    }

    // Obsolete HDR alias.
    if env_on("PROTON_ENABLE_HDR") {
        w.push("PROTON_ENABLE_HDR is an obsolete alias — prefer DXVK_HDR=1.".to_string());
    }

    // HDR needs a presentation path.
    if (env_on("DXVK_HDR") || env_on("PROTON_ENABLE_HDR"))
        && !env_on("PROTON_ENABLE_WAYLAND")
        && !wrap_on("gamescope")
    {
        w.push("HDR needs PROTON_ENABLE_WAYLAND=1 or gamescope with --hdr-enabled to take effect.".to_string());
    }

    // gamescope + native Wayland.
    if wrap_on("gamescope") && env_on("PROTON_ENABLE_WAYLAND") {
        w.push("gamescope and PROTON_ENABLE_WAYLAND together can conflict — usually pick one.".to_string());
    }

    // gplasync vs anti-cheat.
    if env_on("PROTON_DXVK_GPLASYNC") && (env_on("PROTON_EAC_RUNTIME") || env_on("PROTON_BATTLEYE_RUNTIME")) {
        w.push("PROTON_DXVK_GPLASYNC can trip kernel anti-cheat — avoid it in EAC/BattlEye games.".to_string());
    }

    // Mutually-exclusive DXVK forks.
    if env_on("PROTON_DXVK_GPLASYNC") && env_on("PROTON_DXVK_LOWLATENCY") {
        w.push("PROTON_DXVK_GPLASYNC and PROTON_DXVK_LOWLATENCY are different DXVK forks — enable only one.".to_string());
    }

    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Catalog;

    fn enable(cat: &Catalog, opts: &mut Options, key: &str, val: &str) {
        if let Some(i) = cat.envs.iter().position(|e| e.key == key) {
            opts.envs[i].enabled = true;
            opts.envs[i].value = val.to_string();
        }
    }

    #[test]
    fn flags_nvapi_without_nvidia() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        enable(&cat, &mut opts, "PROTON_FORCE_NVAPI", "1");
        let hw = Hardware { nvidia: false, ..Default::default() };
        assert!(warnings(&cat, &opts, &hw).iter().any(|m| m.contains("NVIDIA")));
        // With NVIDIA present, that warning disappears.
        let hw2 = Hardware { nvidia: true, ..Default::default() };
        assert!(!warnings(&cat, &opts, &hw2).iter().any(|m| m.contains("no NVIDIA")));
    }

    #[test]
    fn flags_gplasync_anticheat() {
        let cat = Catalog::bundled();
        let mut opts = Options::from_catalog(&cat);
        enable(&cat, &mut opts, "PROTON_DXVK_GPLASYNC", "1");
        enable(&cat, &mut opts, "PROTON_EAC_RUNTIME", "1");
        let hw = Hardware::default();
        assert!(warnings(&cat, &opts, &hw).iter().any(|m| m.contains("anti-cheat")));
    }
}
