# Settings and files

## Three ways your options get saved

These are easy to confuse, so:

| | What it is | How it's triggered | Scope |
|---|---|---|---|
| **Recipe** | A bundle of options that ships with the app — 10 profiles and 7 troubleshooter fixes | You click a card | Adds to whatever you already have selected. See [Recipes and troubleshooting](Recipes-and-troubleshooting) |
| **Preset** | A named snapshot of your entire configuration, created by you | Header → **Presets** → *Save current…* | Whatever you named it. Load or delete from the same menu |
| **Game memory** | Automatic. No naming, no action needed | Selecting a game | Per-game. Switching games saves the one you're leaving and restores the one you're opening |

Game memory is the one that runs invisibly: tune a game, go back to the library, open a
different game, come back — your first game's settings are exactly as you left them. It's keyed
on the Steam app ID. For non-Steam shortcuts it also remembers the umu executable path.

The app also restores the exact screen you were on when you last closed it.

## Importing an existing command

Two ways to pull settings in rather than building them up:

- **Header → Import** — paste any Steam launch-options string or a `umu-run` command and hit
  *Parse & fill*. protongen works out which mode it is, then populates the toggles, values,
  wrappers and arguments. Anything it doesn't recognise lands in **Custom env** rather than
  being dropped.
- **Load current** — in the *Game & runtime* section, for a Steam game that already has launch
  options set, this imports them directly. Handy for picking up where you left off with a game
  you tuned by hand.

## Settings

Open with the gear in the header. Three collapsible sections.

### Appearance

Ten themes: **Catppuccin Mocha** (default), Catppuccin Macchiato, Catppuccin Frappé, Catppuccin
Latte, Dracula, Nord, Tokyo Night, Gruvbox, Rosé Pine, and One Dark. The whole UI re-themes
instantly and the choice persists. Latte is the only light theme.

### Behavior

| Toggle | What it does |
|---|---|
| **Show unsupported options** | Lists recipes that don't match your detected hardware |
| **I have an HDR display** | Enables HDR recipes. HDR can't be auto-detected |
| **I have an RDNA3/RDNA4 GPU** | Shows FSR 3/4 upscaler-upgrade options, hidden by default |
| **Auto-check ProtonDB** | Fetches the compatibility tier when you select a Steam game |

The two hardware declarations are explained in
[Recipes and troubleshooting](Recipes-and-troubleshooting#the-two-things-it-cant-detect).

### MangoHud overlay

A builder for the MangoHud on-screen overlay, with a live preview of what it'll look like.

Two modes:

- **Build overlay** — tick the metrics you want (FPS, frame timing, CPU/GPU load, CPU/GPU temp,
  RAM, VRAM, GPU name), pick a position and font size, set colours and background opacity, and
  optionally an FPS limit. It writes the result into `MANGOHUD_CONFIG`.
- **Use config file** — point at an existing MangoHud config instead, writing
  `MANGOHUD_CONFIGFILE`.

Either way it also switches on the `mangohud` wrapper for you. The builder reads an existing
config back in, so loading a preset or importing a command repopulates the controls rather than
starting blank.

## Where things are stored

| Path | What's in it |
|---|---|
| `~/.config/protongen/state.toml` | **The only file protongen writes.** Theme, presets, per-game memory, your settings toggles, dismissed banners, last session |
| `~/.config/protongen/params.toml` | *Optional.* Your own parameter catalogue, overriding the built-in one |
| `~/.config/protongen/recipes.toml` | *Optional.* Your own recipes, overriding the built-in ones |
| `~/.cache/protongen/art/` | Downloaded game artwork |
| `~/.local/bin/protongen` | The binary |
| `~/.local/share/applications/protongen.desktop` | Menu launcher |
| `~/.local/share/icons/hicolor/scalable/apps/protongen.svg` | Icon |

(If you've set `XDG_CONFIG_HOME` or `XDG_CACHE_HOME`, those are used instead of `~/.config` and
`~/.cache`.)

Nothing is installed system-wide and nothing needs sudo.

## Customising the catalogue

Both `params.toml` and `recipes.toml` can be overridden without rebuilding the app. Drop a file
at `~/.config/protongen/params.toml` and protongen loads yours instead of the bundled one — so
you can add a brand-new Proton variable, change a default, or write your own recipes.

If your file has a syntax error, protongen falls back to the bundled copy and keeps working, so
a typo can't brick the app.

`uninstall.sh` deliberately leaves these files in place.

## Privacy

**No telemetry.** protongen does not report usage, crashes, or anything else.

It makes exactly three kinds of outbound request, all of which degrade silently if you're
offline:

1. **ProtonDB** — only when *Auto-check ProtonDB* is on, or you click the chip. It fetches the
   summary for one game: tier, confidence, score, report count. **Your launch options are never
   sent.**
2. **Steam CDN** — game artwork, and only when it isn't already in your local Steam library
   cache. Cached afterwards.
3. **GitHub Releases** — the update check, on startup.

Everything else is local. The app runs with a minimal permission set — clipboard text, a file
picker, and opening links — with no shell execution and no general filesystem access.

And to restate the thing from the [Home](Home) page: it reads your Steam configuration files but
**never writes to any of them**.
