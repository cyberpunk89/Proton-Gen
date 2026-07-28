# Recipes and troubleshooting

Recipes are one-click bundles of options. There are two kinds: **profiles** for a setup you
want, and **troubleshooter fixes** for a symptom you're seeing. Both live on the *Recipes*
section, which is where the builder opens.

## Profiles

| Recipe | What it does | Needs |
|---|---|---|
| **NVIDIA: DLSS + Reflex** | NVAPI on, auto-upgrade DLSS, and enable Reflex low-latency in Vulkan/DXVK | NVIDIA GPU |
| **AMD: FSR 4 upgrade** | Auto-upgrade the game's FSR to FSR 4 with multi-frame generation (RDNA4) | AMD GPU, FSR4 flag |
| **AMD: FSR 4 on RDNA3** | FSR 4 upscaling + multi-frame generation on RDNA3 (redstone frame-gen with the RDNA3 workaround) | AMD GPU, FSR4 flag |
| **HDR on Wayland** | Native Wayland driver + DXVK HDR output | Wayland, HDR flag |
| **Gamescope upscale (1440p)** | Run inside gamescope at 2560×1440 fullscreen — good for FSR/integer scaling and frame caps | — |
| **Low-latency competitive** | GameMode + the low-latency layer + DXVK low-latency fork, with a MangoHud FPS readout | `/dev/ntsync` |
| **Max compatibility + logging** | Crash-resilience knobs plus a Proton log for diagnosing a stubborn game | — |
| **Cap frame rate (60 FPS)** | Limit the frame rate via DXVK — no extra tools, any GPU. Edit the value for other caps | — |
| **Borderless fullscreen (gamescope)** | gamescope borderless at native resolution — smoother alt-tab and overlay handling | — |
| **GameMode performance** | Feral GameMode CPU governor + IO priority tuning while the game runs. Safe, universal one-click boost | — |

## Troubleshooter

Pick the row that matches what you're seeing.

| Symptom | Recipe | What it does |
|---|---|---|
| Pre-rendered movies (intro/FMV) are black or absent | **Black or missing cutscenes / videos** | Forces GStreamer media playback, which fixes most missing-video cases |
| Hitches the first time a shader/effect is used | **Stutter when new effects appear** | Keeps DXVK's shader state cache and uses the gplasync fork — **not for anti-cheat games** |
| EAC/BattlEye title closes immediately or errors | **Anti-cheat game won't launch** | Enables Proton's bundled EAC and BattlEye runtimes |
| Crashes before or at the main menu | **Game crashes at launch** | Common crash workarounds plus a log to pinpoint the cause |
| DXVK shows artifacts, black textures, or won't initialise | **Rendering glitches — fall back to WineD3D** | Routes Direct3D through OpenGL instead of DXVK, as a last resort |
| Gamepad isn't recognised or is mis-mapped | **Controller not detected** | Routes input through SDL instead of HIDRAW/Steam Input |
| Missing glyphs, mojibake, or wrong date formats | **Japanese / CJK text is broken** | Runs the game under a Japanese locale — adjust the value for other languages |

## Recipes add, they never remove

This is the one behaviour that surprises people.

Applying a recipe **enables and sets the options it lists, and leaves everything else exactly as
it was.** It does not reset your selection first. So applying two recipes gives you the union of
both, not the second one.

That's useful when you're stacking deliberately — GameMode performance on top of a frame cap,
say. It's confusing when you're trying recipes one after another to see which helps, because
you end up with all of them at once.

If you want a clean slate, hit **Reset** in the command bar before applying the next one.

One more detail: if a recipe references an option that isn't in the current parameter catalogue,
it's appended to **Custom env** rather than silently dropped.

## Notices

The notices strip appears above the parameters when your current selection contains something
contradictory. There are eight checks:

| Notice | Why |
|---|---|
| NVAPI/DLSS options enabled but no NVIDIA GPU detected | They'll have no effect |
| `PROTON_FSR4_UPGRADE` needs an FSR4-capable AMD GPU (RDNA3/RDNA4) | On RDNA3, multi-frame generation also needs `DXIL_SPIRV_CONFIG=wmma_rdna3_workaround` |
| `PROTON_USE_WINED3D` routes D3D through OpenGL | Your `DXVK_*` options won't apply |
| `PROTON_ENABLE_HDR` is an obsolete alias | Prefer `DXVK_HDR=1` |
| HDR needs `PROTON_ENABLE_WAYLAND=1` or gamescope with `--hdr-enabled` | Otherwise there's no presentation path for it |
| gamescope and `PROTON_ENABLE_WAYLAND` together can conflict | Usually pick one |
| `PROTON_DXVK_GPLASYNC` can trip kernel anti-cheat | Avoid it in EAC/BattlEye games |
| `PROTON_DXVK_GPLASYNC` and `PROTON_DXVK_LOWLATENCY` are different DXVK forks | Enable only one |

Notices are advisory. Nothing is blocked — you can paste a command that has warnings on it.

## Why is this option greyed out?

protongen detects some things about your system and dims options that can't apply:

| It detects | How |
|---|---|
| NVIDIA GPU | `/sys/module/nvidia` exists, or `nvidia-smi` is on your `$PATH` |
| AMD GPU | `/sys/module/amdgpu` |
| Intel GPU | `/sys/module/i915` or `/sys/module/xe` |
| Wayland session | `XDG_SESSION_TYPE` or `WAYLAND_DISPLAY` |
| KDE Plasma | `XDG_CURRENT_DESKTOP` |
| ntsync support | `/dev/ntsync` exists |

A dimmed option shows a short reason: *needs NVIDIA GPU*, *needs Wayland session*,
*needs /dev/ntsync*, and so on.

Two things to know:

- **Detection never blocks you.** Dimmed options are still toggleable, and anything hidden is
  revealable — each category shows a "N hidden for your hardware / Show all" line, and Settings
  has a **Show unsupported options** toggle that reveals recipes too.
- **When in doubt, it shows the option.** If detection can't determine something, it's treated
  as relevant rather than hidden.

### The two things it can't detect

Some capabilities genuinely can't be read off the system, so they're opt-in switches in
**Settings → Behavior**:

- **I have an HDR display** — there's no reliable way to know whether your monitor is HDR-capable
  *and* configured for it. Off by default, so the HDR recipe and HDR parameters stay out of the
  way of the majority who can't use them.
- **I have an RDNA3/RDNA4 GPU** — `/sys/module/amdgpu` tells protongen you have an AMD card but
  not which architecture. FSR 3/4 upscaler upgrades only work on RDNA3 and RDNA4; enabling them
  on an older card produces silent no-ops or breakage. So they're hidden until you confirm.

Turn the relevant one on and the corresponding options and recipes appear.

## The yellow "catalog stale" banner

It means: **you've updated Proton since this app's parameter list was last refreshed, so a few
brand-new environment variables may be missing from the catalogue.**

protongen's catalogue records which proton-cachyos build it was written against. On startup it
reads the build date out of your installed proton-cachyos runtime name and compares. If yours is
newer, you get the banner.

Nothing is broken and nothing needs fixing on your side — every option already in the catalogue
still works. The remedy is a protongen update, since a refreshed catalogue ships with each
release. Check for one, or dismiss the banner; a dismissal is remembered for that Proton build,
so it won't nag again until you update Proton again.

If a specific new Proton option is missing and you need it now, you can add it yourself with a
custom catalogue — see [Settings and files](Settings-and-files) — or use the **Custom env**
field to set it directly.
