# protongen

A desktop app for building **Proton launch commands** on CachyOS — for Steam, and for
[umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) outside Steam.

It scans your installed Proton runtimes, Steam games and non-Steam shortcuts, gives you a
searchable catalogue of 87 environment variables and wrappers (each with an explanation, a
default, and a docs link), and shows the resulting launch command live as you toggle things.

**It never writes to your Steam configuration.** The only output is a string you copy and paste
yourself.

Built with **Tauri** — a Rust backend behind a **Svelte + TypeScript + Tailwind** UI.

## Install

Grab the binary from [Releases](https://github.com/cyberpunk89/Proton-Gen/releases), or build
from source:

```sh
git clone https://github.com/cyberpunk89/Proton-Gen.git
cd Proton-Gen && ./install.sh      # user-level install, no sudo
```

Needs Rust, pnpm, and `webkit2gtk-4.1`. The app updates itself from GitHub Releases thereafter.

> Don't use `cargo build --release` — it leaves the app in dev mode looking for a Vite server
> that isn't running. Use `./install.sh` or `pnpm tauri build`.

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

## Development

```sh
pnpm install
pnpm dev              # UI only, in a browser, with mock data
pnpm tauri dev        # the full app
pnpm check            # type-check the TS ↔ Rust contract
cd src-tauri && cargo test
```

Targets the **native** Steam install (`~/.local/share/Steam`), not Flatpak.

See [`design.md`](design.md) for the architecture and [`CLAUDE.md`](CLAUDE.md) for the
operational map.
