# protongen

A polished desktop app for building **Steam launch commands** on CachyOS. It scans your
installed **Proton runtimes**, **Steam games** and **non-Steam shortcuts**, lets you toggle
a categorized, searchable catalog of env-vars / wrappers (badged installed/missing), and
previews + copies the resulting launch command live.

Built with **Tauri** — a Rust backend (all the scanning/parsing/building logic) behind a
custom-branded **Svelte + TypeScript + Tailwind** UI.

## The UI

- **Hero command builder** — a searchable game picker, a quiet Proton-runtime dropdown, a
  Steam ⇄ umu toggle, and a **live, copyable command preview** that updates as you type.
- **Recipes** — one-click profiles (NVIDIA DLSS+Reflex, AMD FSR, HDR-on-Wayland,
  gamescope upscale, low-latency, max-compatibility) and a **troubleshooter** mapping
  symptoms (black cutscenes, stutter, anti-cheat won't start, crash at launch, …) to the
  right options — shown as rich cards with GPU relevance hints.
- **Parameters** — searchable, collapsible categories. Every parameter has an **ⓘ info
  popover** explaining what it does, its default and accepted values, an example and a docs
  link, so the main view stays clean.
- **Themes** — 10 palettes (Catppuccin Mocha/Macchiato/Frappé/Latte, Dracula, Nord, Tokyo
  Night, Gruvbox, Rosé Pine, One Dark); the whole UI re-themes instantly and the choice
  persists.

## Smart helpers

- **Load current** — for a Steam game, import the Launch Options + Proton it already has set.
- **Hardware-aware** — detects your GPU(s)/Wayland/KDE/ntsync and greys-out options that
  don't apply (e.g. FSR4 on non-AMD); a **notices** strip flags conflicts/footguns.
- **MangoHud builder** — pick overlay metrics from checkboxes; it writes `MANGOHUD_CONFIG`.
- **Per-game memory** — selecting a game restores the config you last used for it.
- **Named presets** — save/load/delete named configurations (`state.toml` in
  `~/.config/protongen/`).
- **Import a command** — paste a Steam `%command%` *or* a `umu-run` command and the toggles,
  values, wrappers and args populate automatically; unrecognized env vars land in **Custom env**.
- **ProtonDB** — opt-in tier + report-count lookup for the selected Steam game, with a link
  to the game's ProtonDB page.

It is **read-only** — it never writes to your Steam config files. You paste the generated
string into *Steam → game → Properties → Launch Options* (and pick the matching Proton in
the compatibility dropdown), or run the generated `umu-run` command directly.

## Two output modes

- **Steam launch options** — the `ENV=v … gamescope … -- gamemoderun mangohud %command%`
  string. Works for Steam games *and* non-Steam shortcuts.
- **umu-launcher (standalone)** — a full command for running a game **outside** Steam:
  `GAMEID=umu-0 PROTONPATH=<runtime> ENV=v … umu-run "game.exe"`. `PROTONPATH` auto-fills
  from the selected runtime; selecting a non-Steam shortcut prefills the `.exe`.

## Build & run

Requires **Rust**, **pnpm** (or npm), and the Tauri Linux deps (`webkit2gtk-4.1`).

```sh
pnpm install
pnpm tauri dev        # run in development (hot-reload UI)
pnpm tauri build      # produce a release binary + .deb / AppImage bundle
./install.sh          # build + install as a user-level desktop app (no sudo)
```

The frontend also runs in a plain browser (`pnpm dev`) using built-in mock data — handy for
working on the UI without launching the full app.

Non-interactive dump (verification / scripting):

```sh
cargo run --manifest-path src-tauri/Cargo.toml -- --list
```

## How it works

- **Runtimes** are discovered from `compatibilitytool.vdf` in
  `/usr/share/steam/compatibilitytools.d` (system) and
  `~/.local/share/Steam/compatibilitytools.d` (user), plus Valve-bundled Proton under
  `steamapps/common/Proton*`.
- **Games** come from `appmanifest_*.acf` across libraries; **non-Steam shortcuts** from
  `shortcuts.vdf` (via the `steamlocate` crate). Runtime/redistributable apps are filtered.
- **Parameters** live in `src-tauri/params.toml` (data-driven, not hardcoded). The app loads
  `$XDG_CONFIG_HOME/protongen/params.toml` if present, else the bundled copy — so you can
  customize without rebuilding. Each wrapper/env entry can declare `requires = "<binary>"`,
  which drives the green "installed" / red "missing" badge.
- **Command** ordering is assembled in `src-tauri/src/builder.rs` (pure, unit-tested):
  wrappers sort outermost→inner, `gamescope` owns the `--` separator, and the target
  (`%command%` or `umu-run "<exe>"`) appears exactly once.
- The frontend talks to the backend through Tauri commands in `src-tauri/src/ipc.rs`; the
  `Config` struct in `store.rs` is the shared contract.

Targets the **native** Steam install (`~/.local/share/Steam`), not Flatpak.

## Keeping parameters current

Proton env-vars change with each release. To refresh the catalog, ask Claude Code to run
the bundled skill:

```
/update-proton-params
```

It fetches the latest Proton README, proton-cachyos changelog, CachyOS wiki and
vkd3d-proton docs, diffs them against `src-tauri/params.toml`, and adds/updates entries
(without touching your `$XDG_CONFIG_HOME` override). See
`.claude/skills/update-proton-params/SKILL.md`.

## Layout

| Path | Responsibility |
| --- | --- |
| `src-tauri/src/builder.rs` | pure command assembly — Steam + umu (unit-tested) |
| `src-tauri/params.toml` + `src/params.rs` | data-driven catalog, load/override, `to_spec` |
| `src-tauri/src/ipc.rs` | Tauri command surface (bootstrap, build, parse, lint, recipes, …) |
| `src-tauri/src/{steam,runtime,games,steamcfg}.rs` | read-only Steam/Proton discovery |
| `src-tauri/src/{hardware,lint,which}.rs` | hardware detection, conflict notices, `$PATH` |
| `src-tauri/src/{store,parser,protondb,recipes}.rs` | presets/memory, import, ProtonDB, recipes |
| `src/App.svelte` + `src/lib/components/` | the UI (hero, recipes, parameters, dialogs) |
| `src/lib/state.svelte.ts` | central reactive state (Svelte 5 runes) |
| `src/lib/ipc.ts` + `src/lib/mock.ts` | typed command wrappers + browser fallback data |
| `src/app.css` | design tokens + the 10 theme palettes |
| `recipes.toml` (in `src-tauri/`) + `src/recipes.rs` | profiles + troubleshooter bundles |
