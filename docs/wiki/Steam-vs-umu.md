# Steam vs umu

protongen has two output modes, toggled in the left rail. They produce different strings for
different situations.

**Short version:** if the game is in Steam, use Steam mode. If it isn't, use umu mode.

## Steam mode

Produces a launch-options string built around Steam's `%command%` placeholder:

```
DXVK_HDR=1 PROTON_ENABLE_WAYLAND=1 gamescope -W 2560 -H 1440 -f -- gamemoderun mangohud %command%
```

`%command%` is where Steam substitutes the actual game executable, so everything before it runs
first and wraps the game.

### Where to paste it

1. Right-click the game in your Steam library → **Properties**
2. **General** tab → **Launch Options**
3. Paste, then close the dialog.

If you also selected a Proton runtime in protongen, set it in Steam too:
**Properties → Compatibility → Force the use of a specific Steam Play compatibility tool**, and
pick the runtime protongen names at the bottom of the command bar. Steam mode's string carries
no `PROTONPATH`, so the runtime dropdown is the only thing that decides which Proton runs.

### Ordering

You don't need to think about the order you toggle wrappers in. protongen sorts them into a
fixed sequence — `gamescope` outermost, then `gamemoderun`, then `mangohud` — so the output is
identical no matter what order you clicked. `gamescope` always owns the `--` separator, and
`%command%` appears exactly once.

Steam mode works for **non-Steam shortcuts** too, as long as you've already added them to Steam.

## umu mode

Produces a complete, standalone command you run in a terminal:

```
WINEPREFIX=/home/you/prefixes/witcher GAMEID=umu-0 PROTONPATH=/path/to/GE-Proton9-20 DXVK_HDR=1 umu-run "/games/The Witcher 3/witcher3.exe"
```

The assignment order is fixed: optional `WINEPREFIX`, then `GAMEID` (defaults to `umu-0` if you
leave it blank), then `PROTONPATH`, then your environment variables, then any wrappers, then
`umu-run` and the executable.

The path is quoted only when it contains spaces. If you haven't picked an executable yet, the
command shows `<game.exe>` as a placeholder.

### When to use it

For anything that isn't in Steam at all — a GOG or itch.io install, a repack, a standalone
`.exe`. umu ([Unified Launcher for Games on Linux](https://github.com/Open-Wine-Components/umu-launcher))
runs Proton outside Steam, giving you the same runtime and the same Proton fixes without adding
the game to your library.

Unlike Steam mode, umu mode **does** carry `PROTONPATH`, so the runtime you pick in protongen is
the runtime that runs.

### Selecting a non-Steam shortcut

If you pick a non-Steam shortcut from the library grid, protongen prefills the executable path
from the shortcut, so you usually don't need to browse for it.

## GE-Proton, downloaded automatically

The runtime dropdown has a synthetic entry at the top: **GE-Proton (latest · umu auto-download)**.

Selecting it emits `PROTONPATH=GE-Proton` — the bare codename rather than a path. umu recognises
that and fetches the newest GE-Proton release itself, so you always get the current version with
no version pin and nothing to maintain. protongen doesn't download anything; umu does.

**This only does something in umu mode.** In Steam mode there's no `PROTONPATH` in the output at
all, so the entry has nothing to act on — pick a real installed runtime and set it in Steam's
compatibility dropdown instead.

## Installing a repack with umu

umu mode has an **Installer mode** toggle. It relabels the executable field from *Game .exe* to
*Installer .exe (setup.exe)* — but the real point is the two-step workflow, which depends on
using the same GAMEID both times:

1. Turn **Installer mode** on.
2. Point it at the repack's `setup.exe` and enter a **GAMEID** — anything you'll remember, e.g.
   `umu-witcher3`.
3. Copy the command and run it. The installer runs inside a fresh Proton prefix at
   `~/Games/umu/<GAMEID>`.
4. When it finishes, turn **Installer mode** off and point at the installed game's `.exe` —
   **keeping the same GAMEID**.

Step 4 is the part that matters. umu derives the prefix path from the GAMEID, so reusing it
means the game launches into the prefix it was just installed into. Change the GAMEID and you
get an empty prefix and a game that isn't there.

## Switching modes

The toggle changes what the command bar builds and what the *Game & runtime* section asks for,
but it doesn't discard anything — your enabled options carry across. Switching back and forth is
free.
