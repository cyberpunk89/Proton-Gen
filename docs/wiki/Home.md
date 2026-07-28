# protongen

**A desktop app for building Proton launch commands on CachyOS.**

Getting a Windows game running well under Proton often comes down to a handful of environment
variables and wrapper programs — `PROTON_ENABLE_WAYLAND=1`, `DXVK_HDR=1`, `gamescope -W 2560
-H 1440 --`, `gamemoderun`, and so on. Which ones you need depends on your GPU, your session,
and the game. The knowledge is scattered across the Proton README, the proton-cachyos
changelog, the CachyOS wiki, DXVK and VKD3D docs, and forum posts.

protongen collects that into one place. It scans what you actually have installed, gives you a
searchable catalogue of every option with an explanation and a docs link, and shows you the
resulting launch command live as you toggle things.

## It never touches your Steam configuration

This is the core guarantee, so it's worth stating before anything else.

protongen **reads** your Steam files to discover games, runtimes and shortcuts. It **never
writes to any of them**. Its only output is a string of text. Nothing changes anywhere until
*you* copy that string and paste it into Steam yourself.

That means it can't break your library, and it also means the app can't tell you whether you've
actually applied a change — you'll know because you pasted it.

The only files protongen writes are its own, under `~/.config/protongen/`. See
[Settings and files](Settings-and-files).

## What it does

- **Discovers** your installed Proton runtimes (system, user, Valve-bundled), your Steam games
  across every library folder, and your non-Steam shortcuts.
- **Presents** 87 environment variables and wrapper programs in 11 searchable categories. Every
  single one has an ⓘ popover with an explanation, its default, accepted values, an example and
  a link to upstream documentation — that's enforced by a unit test, so it's never empty.
- **Builds** the launch string live, in a pinned bar at the bottom of the window. One click
  copies it.
- **Guides** you — one-click recipes for common setups, a symptom-based troubleshooter, and a
  notices strip that flags conflicting options before you paste them.
- **Remembers** what you used for each game, so switching back restores it.

It's hardware-aware: it detects your GPU vendor, whether you're on Wayland, whether you're
running KDE, and whether `/dev/ntsync` exists, then hides options that can't apply to you. (You
can always show them again — see [Recipes and troubleshooting](Recipes-and-troubleshooting).)

## Where to go next

| Page | What's on it |
|---|---|
| [Installation](Installation) | Getting it installed, updating it, requirements, and what isn't supported |
| [Steam vs umu](Steam-vs-umu) | The two output modes, when to use each, and how to run games outside Steam |
| [Recipes and troubleshooting](Recipes-and-troubleshooting) | The one-click profiles, the symptom-based fixes, and why an option might be greyed out |
| [Settings and files](Settings-and-files) | Themes, presets, per-game memory, where things are stored, and privacy |
| [Glossary](Glossary) | `%command%`, umu, compat tools, prefixes — the vocabulary |

## Requirements at a glance

Linux, and a **native** Steam install. Flatpak Steam is deliberately not supported. Full detail
on the [Installation](Installation) page.
