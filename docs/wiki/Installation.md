# Installation

## Requirements

- **Linux.** Built and tested on CachyOS; any Arch-based distro should work.
- **A native Steam install** — protongen looks in `~/.local/share/Steam`, `~/.steam/steam` and
  `~/.steam/root`.
- **`webkit2gtk-4.1`** — the app uses your system WebView rather than bundling a browser engine.

Optional, and only needed for the features that use them:

| Package | Used for |
|---|---|
| `gamescope` | the gamescope wrapper |
| `gamemode` (`gamemoderun`) | the GameMode wrapper |
| `mangohud` | the MangoHud overlay |
| `umu-launcher` (`umu-run`) | [umu mode](Steam-vs-umu) |

protongen checks your `$PATH` for these and badges each option green (installed) or red
(missing), so you'll see at a glance what you're missing. A missing program doesn't stop you
toggling the option — it just won't work until you install it.

## Install the prebuilt binary

The [Releases page](https://github.com/cyberpunk89/Proton-Gen/releases) publishes a `protongen`
binary and a matching `protongen.sha256`.

```bash
curl -LO https://github.com/cyberpunk89/Proton-Gen/releases/latest/download/protongen
curl -LO https://github.com/cyberpunk89/Proton-Gen/releases/latest/download/protongen.sha256
sha256sum -c protongen.sha256
install -Dm755 protongen ~/.local/bin/protongen
```

> The release binary is built inside an `archlinux:latest` container on purpose. It's
> dynamically linked, so building it on Ubuntu would produce soname mismatches against
> CachyOS's rolling libraries.

That gives you a working binary but no desktop entry. For a menu shortcut and icon, use
`install.sh` below.

## Build and install from source

Needs **Rust**, **pnpm** (npm works as a fallback), and the Tauri Linux dependencies.

```bash
git clone https://github.com/cyberpunk89/Proton-Gen.git
cd Proton-Gen
./install.sh
```

`install.sh` needs **no sudo**. It installs three files:

| File | Path |
|---|---|
| binary | `~/.local/bin/protongen` |
| icon | `~/.local/share/icons/hicolor/scalable/apps/protongen.svg` |
| launcher | `~/.local/share/applications/protongen.desktop` |

It then refreshes the desktop and icon caches if those tools are present. If `~/.local/bin`
isn't on your `$PATH` it says so — the menu shortcut still works either way, because the
`.desktop` file uses an absolute path.

### ⚠️ Don't use `cargo build --release`

It will appear to work and produce a binary that fails at runtime with a blank window.

A plain cargo build leaves the app in **development mode**, where it tries to load the UI from
a Vite dev server at `localhost:1420` that isn't running. The frontend has to be compiled and
embedded into the binary, which is what the Tauri CLI does.

Use `./install.sh`, or `pnpm tauri build` if you want the bundles.

### Building a `.deb` or AppImage

`pnpm tauri build` produces both — they're configured in `tauri.conf.json`. Note that **CI does
not publish them**: the release workflow builds with `--no-bundle`, so Releases only ever
contains the bare binary. If you want a package, build it locally.

## Updating

protongen updates itself. On launch it checks the GitHub Releases API and compares the latest
tag against its own compiled-in version. If there's a newer one, a green banner appears with
the version delta and a link to the release notes.

Clicking **Update now** downloads the new binary, fetches the published `.sha256`, **verifies
the checksum and aborts if it doesn't match**, then atomically replaces the running executable.
Replacing a running binary is safe on Linux — the open file keeps working — so **the new
version takes effect the next time you start the app**.

Two things worth knowing:

- **There's no code signing.** Integrity comes from HTTPS plus the published checksum.
- If the replacement fails (for example the binary is in a directory you can't write to), the
  error tells you to re-run `install.sh` instead.

A failed update check — offline, rate-limited, whatever — is silently ignored. It never blocks
startup and never shows a false banner.

### What the version number tells you

protongen uses `X.Y.Z` version numbers, and the part that changes tells you what's in the
update:

| Change | Means | Example |
|---|---|---|
| **Last number** (`Z`) | **Parameter refresh only** — the Proton/DXVK/VKD3D catalog was updated (new env vars, fixed descriptions). No app behaviour changes. | `0.8.0 → 0.8.1` |
| **Middle number** (`Y`) | **Feature update** — new functionality, UI, or fixes in the app itself. | `0.8.x → 0.9.0` |
| **First number** (`X`) | A milestone release. | `→ 1.0.0` |

So a `.1`-style bump is a safe, quick catalog top-up; a `.0` bump is a real feature release.

## Uninstalling

```bash
./uninstall.sh
```

Removes the binary, icon and launcher, and refreshes the caches. It deliberately **leaves
`~/.config/protongen/` alone**, so your presets, per-game memory and any custom `params.toml`
survive a reinstall. Delete that directory by hand if you want a clean slate.

## What protongen is not

- **Not a Proton installer or version manager.** It discovers runtimes you already have; it
  doesn't download or install them. The one exception is indirect — selecting *GE-Proton
  (latest)* in [umu mode](Steam-vs-umu) makes **umu**, not protongen, fetch it.
- **Not a Steam configuration tool.** It never writes Steam files. You paste the result
  yourself.
- **Not Flatpak-aware.** Flatpak Steam is excluded by design, not by oversight. If your Steam
  is a Flatpak, protongen won't find your games.
- **Linux only.**
