---
name: update-proton-params
description: Refresh protongen's params.toml parameter catalog from upstream Proton / proton-cachyos / DXVK / VKD3D docs. Use when the user wants to update the Proton launch parameters, add newly-introduced env vars, or says "update proton params", "refresh the catalog", or after a Proton version bump.
---

# Update the Proton parameter catalog

`protongen` reads its launch-parameter catalog from `params.toml` at the repo root
(the app bakes it in via `include_str!`, and a user copy at
`$XDG_CONFIG_HOME/protongen/params.toml` overrides it). This skill refreshes that
catalog when Proton evolves.

## Sources (fetch the current versions)

Use WebFetch on each, preferring the newest stable Proton branch:

1. Proton README (env var reference):
   `https://github.com/ValveSoftware/Proton/blob/proton_11.0/README.md`
   (also check `.../blob/master/README.md` for newer vars; bump the branch number when
   a new major Proton ships).
2. proton-cachyos changelog + README (CachyOS-only knobs like FSR4/XeSS/NTSync):
   `https://github.com/CachyOS/proton-cachyos/blob/cachyos_main/CHANGELOG.md`
   `https://github.com/CachyOS/proton-cachyos/blob/cachyos_main/README.md`
3. CachyOS gaming wiki: `https://wiki.cachyos.org/configuration/gaming/`
4. vkd3d-proton env vars: `https://deepwiki.com/HansKristian-Work/vkd3d-proton/8.4-environment-variables`
5. DXVK config reference (HUD / frame rate / config): `https://github.com/doitsujin/dxvk`

## Steps

1. **Read** the current `params.toml` to see what's already covered.
2. **Fetch** the sources above. Extract every documented `PROTON_*`, `DXVK_*`,
   `VKD3D_*`, `WINE*`, and relevant `__GL*`/`__NV*` variable, plus wrapper tools.
3. **Diff**: list (a) new variables not in `params.toml`, (b) variables whose meaning,
   default, or accepted values changed, (c) variables removed/deprecated upstream.
4. **Edit `params.toml`** (the repo-root file — the single source of truth):
   - Add new `[[env]]` entries with a sensible `category` (reuse existing category
     names: Performance / Sync, Render backend, Upscaling & frame-gen, NVIDIA,
     Display / Wayland / HDR, Anti-cheat, DXVK, VKD3D (D3D12), Wine / Overrides,
     Overlay / Post-processing, Logging / Debug, Audio, Compatibility / Misc —
     add a new category only if nothing fits). **Upscaling & frame-gen is
     vendor-neutral** (FSR, DLSS, XeSS, OptiScaler, MLFG — each entry carries its
     own `gpu` hint and self-filters); **NVIDIA** is for NVAPI, the NVIDIA
     runtime libs and PRIME offload. Do not merge them back: a single
     "NVIDIA / Upscaling" heading meant AMD users navigated to a row labelled
     NVIDIA to find their FSR options, while every NVIDIA entry under it was
     hidden by the relevance filter.
   - Each entry needs `key`, `category`, `default_value`, a one-line `help`, and
     `values = [...]` when there's a small fixed set (drives the GUI combo box).
   - Also author the **rich info** fields used by the GUI's per-parameter info popup:
     `details` (2-4 sentence accurate explanation + tradeoffs/when-to-use), `example`
     (a concrete launch-options snippet, e.g. `DXVK_HUD=fps %command%`), and `url`
     (canonical doc link). Keep these populated for every entry.
   - Set the **relevance hints** when a parameter is vendor/session-specific so the GUI can
     grey it out on irrelevant hardware: `gpu = "nvidia"|"amd"|"intel"` (e.g. NVAPI/DLSS →
     nvidia, FSR/MLFG → amd) and/or `needs = [...]` from the capability list
     `wayland|kde|ntsync|hdr|fsr4|rdna3|rdna4`. Leave both unset for universal options.
     **Only those seven values are valid** — `params::KNOWN_NEEDS` is asserted against this
     file by `cargo test`, because an unrecognised tag reads as "always relevant" at
     runtime and would silently filter nothing. For AMD generation-specific options use
     `rdna3` / `rdna4`, not `fsr4` (which matches both).
   - **Preserve `tier` on existing entries.** `tier = "advanced"` hides an entry behind the
     GUI's show-advanced toggle, and roughly 57 of the ~93 entries carry it. This refresh
     rewrites `params.toml` wholesale, so dropping the field would un-hide the entire long
     tail in one go. Diff the tags before and after. Assign `tier = "advanced"` to new
     entries that are debugging, logging, low-level tuning, or a niche variant of something
     already listed; leave it off for anything a gamer would plausibly toggle on purpose.
     `params::tests::bundled_tiers_hide_a_meaningful_share_without_burying_the_basics`
     catches wholesale loss, but not a handful of individually-dropped tags.
   - For deprecated vars, remove them or note the deprecation in `help`.
   - Keep wrappers' `requires = "<binary>"` so the installed/missing badge keeps working.
   - Update the **dated header comment** at the top (`updated YYYY-MM-DD`) and the
     Proton version it targets.
   - Update the **`[meta]` table**: set `proton_cachyos_build` to the installed
     proton-cachyos build you refreshed against (the `YYYYMMDD` from the runtime's
     display name / `version` file, e.g. `20260602`) and `updated` to today's date. This
     clears the app's "catalog stale" banner after the user reinstalls.
5. **Do NOT** overwrite a user's `$XDG_CONFIG_HOME/protongen/params.toml`; only edit the
   repo `params.toml`. Mention to the user that their override file (if any) shadows it.
6. **Verify**: run `cargo test` (the `params::tests::bundled_parses_and_has_entries`
   test confirms the TOML still parses and `to_spec` ordering holds) and
   `cargo run -- --list` (prints the new catalog counts).
7. **Summarize** the diff for the user: added / changed / removed variables.

## Notes

- The TOML schema is documented in the header of `params.toml` and parsed by
  `src/params.rs` (`WrapperDef` / `EnvDef`). Match those field names exactly.
- Prefer small, well-described entries over dumping every obscure debug flag; this is a
  curated catalog, not an exhaustive dump. Favor variables a gamer would actually toggle.
