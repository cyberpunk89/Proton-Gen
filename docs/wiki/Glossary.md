# Glossary

### `%command%`

A placeholder Steam substitutes with the actual game executable when it builds the launch
command. Anything you put before it runs first and wraps the game; anything after it becomes
arguments to the game. It's what makes Steam's Launch Options field able to do more than pass
flags.

### Proton

Valve's compatibility layer for running Windows games on Linux — a bundle of Wine plus DXVK,
VKD3D-Proton and various patches. Steam ships several versions; you pick one per game in
*Properties → Compatibility*.

### proton-cachyos

CachyOS's Proton build, with additional patches and performance work on top of upstream Proton.
It's the build protongen's parameter catalogue is written against, and its version is what the
["catalog stale" banner](Recipes-and-troubleshooting#the-yellow-catalog-stale-banner) compares
against.

### GE-Proton

GloriousEggroll's community Proton build, carrying media codecs and game-specific fixes that
upstream Proton doesn't include. Often the thing to try when a game misbehaves on stock Proton.
In [umu mode](Steam-vs-umu), protongen can hand umu the bare codename `GE-Proton` and let it
fetch the newest release automatically.

### umu / `umu-run`

The [Unified Launcher for Games on Linux](https://github.com/Open-Wine-Components/umu-launcher).
It runs Proton **outside** Steam, so you get the same runtime and the same fixes for a game that
isn't in your library. `umu-run` is the command. See [Steam vs umu](Steam-vs-umu).

### Compat tool

Steam's term for a Proton runtime as it appears in the compatibility dropdown. Each one declares
its name in a `compatibilitytool.vdf` file, which is how protongen discovers them.

### Wrapper

A program that runs *around* the game rather than setting a variable for it. protongen knows
three: `gamescope` (a micro-compositor for scaling, frame limits and HDR), `gamemoderun` (Feral
GameMode's CPU and IO tuning), and `mangohud` (the performance overlay). They nest in a fixed
order in the output.

### `WINEPREFIX`

The directory holding a Wine/Proton virtual Windows installation — its registry, its C: drive,
its installed runtimes. Each game generally gets its own so they can't interfere with each
other. In umu mode you can set one explicitly; leave it blank and umu derives one from the
GAMEID.

### `GAMEID`

umu's identifier for a game. It determines the prefix path (`~/Games/umu/<GAMEID>`) and lets umu
look up game-specific fixes. Defaults to `umu-0`. Using a consistent GAMEID matters when
[installing a repack](Steam-vs-umu#installing-a-repack-with-umu) — the installer and the game
must share one, or the game launches into an empty prefix.

### ProtonDB

A community site collecting user reports on how well each Steam game runs under Proton. Games
are rated **Platinum** (perfect out of the box), **Gold** (perfect after tweaks), **Silver**
(minor issues), **Bronze** (major issues), or **Borked** (doesn't run). protongen can show the
tier and report count for a selected game.

### DXVK

Translates Direct3D 9/10/11 to Vulkan. Most of the `DXVK_*` variables in the catalogue configure
it. The alternative, WineD3D, translates to OpenGL instead and is generally slower — worth trying
only when DXVK misbehaves.

### VKD3D-Proton

The Direct3D 12 equivalent of DXVK — translates D3D12 to Vulkan. Configured by the `VKD3D_*`
variables.

### NVAPI

NVIDIA's proprietary API that games use for DLSS, Reflex and other vendor features. Proton can
expose a translation layer so those work under Linux, which is what the NVAPI options switch on.

### ntsync

A Linux kernel module implementing Windows synchronisation primitives natively, which is faster
than emulating them in userspace. protongen checks for `/dev/ntsync` and dims the options that
depend on it if it's absent.

### VDF / ACF

Valve's plain-text key-value file formats. Steam stores launch options and compat-tool choices in
`localconfig.vdf` and `config.vdf`, non-Steam shortcuts in `shortcuts.vdf`, and per-game install
metadata in `appmanifest_*.acf`. protongen reads all of these and writes none of them.
