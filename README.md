<div align="center">

<img src="assets/protongen.svg" width="84" alt="">

# protongen

**Build Proton launch commands without memorising environment variables.**

protongen scans your Proton runtimes, your Steam library and your non-Steam shortcuts, then
gives you a searchable, explained catalogue of every tuning knob — and writes the launch
command for you as you toggle things. For Steam, and for
[umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) outside Steam.

[**Install**](#install) · [Quick tour](#a-quick-tour) · [How to use it](#how-to-use-it) ·
[Documentation](https://github.com/cyberpunk89/Proton-Gen/wiki) ·
[Releases](https://github.com/cyberpunk89/Proton-Gen/releases)

![Platform: Linux](https://img.shields.io/badge/platform-Linux-1793d1?style=flat-square)
![Latest release](https://img.shields.io/github/v/release/cyberpunk89/Proton-Gen?style=flat-square&color=b4befe)
![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-f5c2e7?style=flat-square)
![Catalog: Proton 11.0 / proton-cachyos 20261005](https://img.shields.io/badge/catalog-Proton%2011.0%20%C2%B7%20cachyos%2020261005-a6e3a1?style=flat-square)

<img src="docs/screenshots/builder.webp" alt="protongen building a Steam launch command: a category of NVIDIA options with three toggled on, and the finished command pinned at the bottom of the window">

</div>

---

## The problem it solves

Getting a Windows game running well under Proton usually comes down to a handful of environment
variables and wrapper programs — `PROTON_ENABLE_WAYLAND=1`, `DXVK_HDR=1`, `gamemoderun`,
`gamescope -W 2560 -H 1440 --`. Which ones you need depends on your GPU, your session and the
game, and the knowledge is scattered across the Proton README, the proton-cachyos changelog, the
CachyOS wiki, DXVK/VKD3D docs and forum threads.

protongen puts all of it in one window:

- **137 environment variables and 6 wrappers**, in 15 categories — each with a plain-English
  explanation, its default, accepted values, an example and a link to the upstream docs.
- **20 one-click recipes** — 12 curated profiles and 8 symptom-based troubleshooter fixes.
- **Live command preview**, pinned to the bottom of the window. One click copies it, and
  **Open in Steam** jumps straight to the game's Properties so you can paste.
- **Simple or Advanced** — a curated grid of the options most people reach for, or the full
  categorised catalogue. Both drive the same selection, so switching never loses anything.
- **Hardware-aware**: it detects your GPU vendor, Wayland, KDE and `/dev/ntsync`, and hides
  options that can't apply to you (always revealable with *Show all*).
- **Steam, Heroic, Nexus and umu** — Steam games and shortcuts, Heroic games (sideloaded plus
  Epic/GOG/Amazon), games in [Nexus](#works-with-nexus), or a standalone `umu-run` command.

### It never touches your Steam configuration

protongen **reads** your Steam files to find games, runtimes and shortcuts. It **never writes to
any of them**. The only output is a string of text — nothing changes until *you* paste it into
Steam yourself. Its own files are settings in `~/.config/protongen/` and downloaded cover art in
`~/.cache/protongen/`.

Five features do write elsewhere, and each one only runs when you click through a confirm step
that names exactly what it will touch:

- **Heroic** — writes the env vars and wrappers into that game's Heroic per-game config
  (backed up first; keys protongen doesn't own are left alone).
- **OptiScaler** — downloads the latest OptiScaler release from GitHub and extracts it into
  the game's own folder (an existing `OptiScaler.ini` is kept).
- **MangoHud / vkBasalt export** — merges the overlay or effect chain you built into your
  system-wide `~/.config/MangoHud/MangoHud.conf` or `~/.config/vkBasalt/vkBasalt.conf`
  (backed up first; lines protongen doesn't manage are preserved).
- **Nexus** — hands a game's tuning to Nexus with `nexus-cli --set-launch`; Nexus then updates
  the launch script, Steam shortcut and Heroic entry it keeps for that game. protongen writes
  none of those files itself.

## Install

### Option 1 — prebuilt binary

Each [release](https://github.com/cyberpunk89/Proton-Gen/releases) publishes a `protongen`
binary and a matching `protongen.sha256`.

```bash
curl -LO https://github.com/cyberpunk89/Proton-Gen/releases/latest/download/protongen
curl -LO https://github.com/cyberpunk89/Proton-Gen/releases/latest/download/protongen.sha256
sha256sum -c protongen.sha256
install -Dm755 protongen ~/.local/bin/protongen
```

The release binary is built inside an `archlinux:latest` container on purpose — it's dynamically
linked, so an Ubuntu build would hit soname mismatches against CachyOS's rolling libraries.

### Option 2 — build from source (adds a menu entry and icon)

Needs **Rust**, **pnpm** (npm works as a fallback) and the Tauri Linux dependencies. No sudo.

```bash
git clone https://github.com/cyberpunk89/Proton-Gen.git
cd Proton-Gen && ./install.sh
```

That installs three files: the binary to `~/.local/bin/protongen`, an icon to
`~/.local/share/icons/`, and a launcher to `~/.local/share/applications/` — so protongen shows
up in your app menu. `./uninstall.sh` removes all three and deliberately leaves your config
alone.

> [!WARNING]
> Don't build with `cargo build --release`. It produces a binary that starts to a blank window,
> because a plain cargo build leaves the app in development mode looking for a Vite dev server.
> Use `./install.sh` or `pnpm tauri build`.

### Requirements

- **Linux** — built and tested on CachyOS; any Arch-based distro should work.
- **A native Steam install** (`~/.local/share/Steam`, `~/.steam/steam` or `~/.steam/root`).
  Flatpak Steam is not supported.
- **`webkit2gtk-4.1`** — the app uses your system WebView instead of bundling a browser engine.

**On a different distro?** The [prebuilt binary](#option-1--prebuilt-binary) is built in an
`archlinux:latest` container and dynamically linked against Arch's rolling libraries, so it may
not run as-is elsewhere (glibc/webkit2gtk ABI mismatches). [Building from
source](#option-2--build-from-source-adds-a-menu-entry-and-icon) works on any modern distro with
Rust, pnpm, and your distro's `webkit2gtk-4.1` package — e.g. `webkit2gtk4.1-devel` on Fedora,
`libwebkit2gtk-4.1-dev` on Debian/Ubuntu, `webkit2gtk-4.1` on Arch (see [Tauri's Linux
prerequisites](https://v2.tauri.app/start/prerequisites/#linux) for the rest of the build
toolchain). None of protongen's own discovery — Steam, Proton runtimes, wrapper binaries — is
distro-specific; the first-run **Your system** check and **Settings → Paths** cover any layout
that isn't one of the built-in guesses.

These are optional, and only needed for the features that use them. protongen checks your
`$PATH` and badges each option installed or missing, so you can see at a glance what you'd need:

| Package | Used for |
|---|---|
| `gamescope` | the gamescope wrapper |
| `gamemode` (`gamemoderun`) | the GameMode wrapper |
| `mangohud` | the MangoHud overlay |
| `vkbasalt` | post-processing (CAS sharpening, FXAA, SMAA, ReShade FX) |
| `umu-launcher` (`umu-run`) | umu mode |

If one of these lives somewhere off your `$PATH`, set its location under
**Settings → Paths** and protongen will emit that path into the command rather than the
bare name — which matters because Steam launched from a desktop entry often has a
`$PATH` without `~/.local/bin`.

### Updating

protongen updates itself. On launch it compares its own version against the latest GitHub
release; if there's a newer one, a banner offers to install it. The download is checksum-verified
against the published `.sha256` and aborted on mismatch, then the running executable is replaced
atomically — the new version takes effect next time you start the app. A failed check (offline,
rate-limited) is silently ignored.

## How to use it

1. **Pick a game** — or start a generic command.
2. **Toggle what you need** — from a category, a search, or a one-click recipe.
3. **Copy the command** and paste it into Steam → right-click the game → *Properties* →
   *Launch Options*. In umu mode, paste it into a terminal or a script instead.

If you already have launch options set, hit **Import**, paste the string, and protongen parses
it back into toggles so you can keep building from where you are.

**Ctrl+K** opens a command palette over games, parameters, recipes, presets and actions.

## A quick tour

### First run: your system, at a glance

The welcome tour's first step is a live status check, not static copy: your Steam install (or
why it couldn't find one), how many Proton runtimes turned up, and which optional tools
(gamescope, GameMode, MangoHud, umu-launcher…) are on your `$PATH` — with a direct link into
**Settings → Paths** if anything needs pointing at a non-default location. It shows once; the
same information stays available afterwards as banners and in **Settings → Paths** itself.

### Your library, discovered

Steam games across every library folder, plus non-Steam shortcuts. Cover art comes from your local
Steam library cache, with a CDN fallback — titles with neither get a placeholder tile, as below.
Or skip the library entirely and build a generic command.

<img src="docs/screenshots/library.webp" alt="The game picker: a grid of installed titles with a filter box and a Generic command button">

### Every option explained

No bare env-var names. Every entry has an ⓘ popover with what it does, its default, its accepted
values, a copy-ready example and a link to the upstream documentation — enforced by a unit test,
so it's never empty.

<img src="docs/screenshots/parameter-details.webp" alt="An option's info popover showing a description, default, accepted values, example command and documentation link">

### One-click recipes, and a troubleshooter

Twelve curated profiles (DLSS + Reflex, HDR on Wayland, gamescope upscaling, GameMode, frame caps…)
and eight symptom-based fixes — *black cutscenes*, *stutter when new effects appear*, *anti-cheat
game won't launch*. Applying one merges onto your current selection; it never silently turns
things off.

<img src="docs/screenshots/recipes.webp" alt="The Recipes screen showing profile cards with tags and Apply buttons, plus a Troubleshooter section">

### Search the whole catalogue

Search matches keys, descriptions and details across all 14 categories at once.

<img src="docs/screenshots/search.webp" alt="Search results for hdr, listing matching options from several categories">

### Steam or umu

Steam mode emits a `%command%` string plus the Proton runtime to select in Steam's dropdown. umu
mode emits a complete `umu-run` invocation with `GAMEID`, `PROTONPATH`, an optional
`WINEPREFIX`, game arguments and an installer mode for repack setups — so you can run Windows
games with Proton entirely outside Steam. Picking *GE-Proton (latest)* lets umu fetch and keep
the newest GE-Proton itself.

<img src="docs/screenshots/umu.webp" alt="umu mode: Proton runtime picker, installer mode, game exe, WINEPREFIX and GAMEID fields, with a full umu-run command below">

### Presets, per-game memory, and honest warnings

Save named presets, and protongen remembers what you used for each game so switching back
restores it. A preset can be shared as a `protongen:v1:` text code that anyone can paste back
in. A notices strip flags conflicts before you paste — enabling gplasync in an
EAC/BattlEye title, HDR without Wayland or gamescope, two different DXVK forks at once.

### Logs, and an optional AI coach

The **log viewer** reads the tail of a game's logs without leaving the app — Proton's
`PROTON_LOG=1` log (following `PROTON_LOG_DIR`, and umu's naming for umu-run games), DXVK's and
VKD3D-Proton's own logs, and Nexus's per-launch output for its games — newest first, with a
switcher when there's more than one. If
you point **Settings** at a local OpenAI-compatible server (LM Studio, Ollama, llama.cpp), an
opt-in **log coach** suggests tuning from that log, and the **troubleshooter** diagnoses a
problem you describe in your own words — both offer changes you apply with a click, never
automatically.

> [!NOTE]
> The AI features are off by default. When enabled, the game's Proton log (or your description
> plus error lines from it), the built launch command and your detected hardware are sent to the
> endpoint you configured — keep it local if you don't want that data leaving your machine.

### Builders for the fiddly ones

MangoHud, vkBasalt, OptiScaler, gamescope and `WINEDLLOVERRIDES` each get a dialog builder
instead of a hand-written string. The gamescope builder offers your monitor's native mode and
refresh rate (read from its EDID) as a one-click output size; the OptiScaler builder composes
`OptiScaler.ini` settings into `PROTON_OPTISCALER_CONFIG`, and can fetch the latest OptiScaler
build into the game's folder.

### Across games

Select several games in the library and apply one preset to all of them; **Compare** shows what
differs between the current builder, any preset and any game's saved tuning. The Game & runtime
panel shows each game's Wine prefix and shader-cache size with a button to open the folder, and
— opt-in, from [AreWeAntiCheatYet](https://areweanticheatyet.com) — whether its anti-cheat runs
on Linux at all.

### Works with Nexus

Nexus — a launcher for repacks, Steam and Heroic games — launches the
games; protongen tunes them. A Nexus game shows up once (not as its Steam shortcut *and* its
Heroic sideload), opens on its own prefix and the Proton build Nexus runs it with, and **Apply to
Nexus** sends the tuning back. Nexus's *Tune in protongen* opens the game here — again in the
same window if protongen is already open — and scripts can read protongen's data as JSON:

```bash
protongen --list --json                # runtimes and games (ids, sources, Nexus slugs)
protongen --game-config <id> [--umu]   # a game's saved tuning and its built command
protongen --catalog --json             # wrappers in command order, env keys
```

### Ten themes

Catppuccin (all four flavours), Dracula, Nord, Tokyo Night, Gruvbox, Rosé Pine and One Dark.

| Catppuccin Latte | Gruvbox |
|---|---|
| <img src="docs/screenshots/theme-latte.webp" alt="protongen in the light Catppuccin Latte theme"> | <img src="docs/screenshots/theme-gruvbox.webp" alt="protongen in the Gruvbox theme"> |

Plus an HDR toggle and an **AMD GPU generation** selector (RDNA3 / RDNA4, pre-filled from your
GPU's PCI id when it can be read) that decides which FSR upgrade options and recipes are shown, and a
**Paths** section for the cases auto-discovery misses — extra Steam roots and library
folders, extra Proton directories, and where to find `umu-run`, `gamescope`, `gamemoderun`
and `mangohud`. Every change re-scans immediately and tells you what it found, so a wrong
path says so instead of failing quietly.

<img src="docs/screenshots/settings.webp" alt="The settings drawer with the Appearance section expanded, showing the ten themes">

## Keeping the catalogue current

The parameter catalogue is plain TOML baked into the binary
([`params.toml`](src-tauri/params.toml), [`recipes.toml`](src-tauri/recipes.toml)) and records
which proton-cachyos build it was written against. When your installed build is newer, the app
tells you the catalogue may be stale. You can also drop your own copy of either file into
`~/.config/protongen/` to override the bundled one.

## Documentation

| Page | |
|---|---|
| [Home](https://github.com/cyberpunk89/Proton-Gen/wiki) | What it does and the read-only guarantee |
| [Installation](https://github.com/cyberpunk89/Proton-Gen/wiki/Installation) | Install, update, requirements, limitations |
| [Steam vs umu](https://github.com/cyberpunk89/Proton-Gen/wiki/Steam-vs-umu) | The two output modes, and running games outside Steam |
| [Recipes and troubleshooting](https://github.com/cyberpunk89/Proton-Gen/wiki/Recipes-and-troubleshooting) | One-click profiles, symptom-based fixes, greyed-out options |
| [Settings and files](https://github.com/cyberpunk89/Proton-Gen/wiki/Settings-and-files) | Presets, per-game memory, storage locations, privacy |
| [Glossary](https://github.com/cyberpunk89/Proton-Gen/wiki/Glossary) | `%command%`, umu, compat tools, prefixes |

Wiki pages are generated from [`docs/wiki/`](docs/wiki) — edit those, not the wiki.

## What protongen is not

- **Not a Proton installer or version manager.** It discovers runtimes you already have. The one
  exception is indirect: choosing *GE-Proton (latest)* in umu mode makes **umu** download it.
- **Not a Steam configuration tool.** It never writes Steam files; you paste the result.
- **Not a launcher.** It builds the command; Steam, Heroic, umu or Nexus runs the game.
- **Not Flatpak-aware.** Flatpak Steam is excluded by design.
- **Linux only.**

## Development

Built with **Tauri 2** — a Rust core behind a **Svelte 5 + TypeScript + Tailwind** UI. Command
assembly is a pure, unit-tested Rust function; the Tauri commands are just a serialization
bridge.

```bash
pnpm install
pnpm dev              # UI only, in a browser, with mock data
pnpm tauri dev        # the full app
pnpm check            # type-check, incl. types.ts against the Rust structs (src/lib/contract.ts)
pnpm test             # frontend unit tests (vitest)
cd src-tauri && cargo test   # Rust tests; also regenerates src/lib/generated/
cargo run -- --list   # discovery sanity check from the terminal
```

See [`design.md`](design.md) for the architecture and [`CLAUDE.md`](CLAUDE.md) for the
operational map.

## License

**[GNU GPL v3.0 or later](LICENSE)** — © 2026 the protongen authors.

You're free to use, study, share and modify protongen. If you distribute a modified version, it
has to stay free software under the same license, with source available. There's no warranty; see
the [license text](LICENSE) for the details.

The bundled [Lexend](https://www.lexend.com/) font is a separate work under the SIL Open Font
License 1.1 ([`public/fonts/OFL.txt`](public/fonts/OFL.txt)).

## Credits

Standing on the shoulders of [Proton](https://github.com/ValveSoftware/Proton),
[proton-cachyos](https://github.com/CachyOS/proton-cachyos),
[umu-launcher](https://github.com/Open-Wine-Components/umu-launcher),
[DXVK](https://github.com/doitsujin/dxvk),
[vkd3d-proton](https://github.com/HansKristian-Work/vkd3d-proton),
[gamescope](https://github.com/ValveSoftware/gamescope),
[GameMode](https://github.com/FeralInteractive/gamemode) and
[MangoHud](https://github.com/flightlessmango/MangoHud).
