# protongen — Screen blueprints (Penpot artboards)

Layout recipes for rebuilding the app's screens in Penpot. Each screen is one
artboard. All colors/sizes reference the tokens in
[`tokens/protongen.tokens.json`](tokens/protongen.tokens.json) and the component
specs in [`design-system.md`](design-system.md).

**Window / artboard size:** design at **1280 × 800** (desktop). The window is
resizable and frameless (`decorations: false`) — the app draws its own title bar.

The whole window sits on **`effect.gradient.bg`** (two iris radial glows over
`color.bg`). Put that as the artboard background on every screen.

---

## Global shell (present on every builder screen)

```
┌────────────────────────────────────────────────────────────────────┐
│  HEADER  (h ≈ 44, padding px space.4 / py space.2, drag region)      │
├──────────────┬─────────────────────────────────────────────────────┤
│              │  SCROLL REGION (flex-1, overflow-y auto)              │
│   NAV RAIL   │    ┌── centered column, max-w content-max (896) ──┐   │
│   (240,      │    │  banners → Notices → MainPanel / SimplePanel  │   │
│   advanced   │    └───────────────────────────────────────────────┘  │
│   mode only) │                                                       │
│              ├─────────────────────────────────────────────────────┤
│              │  COMMAND BAR (pinned, border-top, bg mantle-30)      │
│              │    centered column max-w content-max → CommandPreview │
└──────────────┴─────────────────────────────────────────────────────┘
```

### Header (`Header.svelte`)
- Bar: `flex items-center gap-2.5`, padding `px space.4 / py space.2`.
- Left: logo (`size.logo` 28, `radius.lg`) + stack → `protongen`
  (`font.size.sm`/medium/`color.text`) over Steam root path
  (`font.size.xs-plus`/`color.muted`).
- Right cluster (`ml-auto`, `gap-1.5`), builder view only:
  UiModeToggle · **Import** (bordered ghost button) · **Presets** (bordered
  button w/ bookmark) · **Logs** (bordered, disabled w/o game) · then always:
  **Refresh** (`size.control` 32 icon button) · **Settings** (icon button) ·
  window controls: Minimize · Maximize · Close (close hovers to `color.danger`
  fill + white).
- Bordered ghost button spec: border `color.border`, fill `surface-2` @ 50%,
  text `color.subtext`, `px space.2-5 / py space.1-5`, `radius.lg`; hover border
  → `accent` @ 50%.

### Nav rail (`NavRail.svelte`) — advanced mode only
- `aside` width `size.navrail-w` (240), full height, right border `color.border`,
  fill `alpha.mantle-30`.
- Top block (`p space.3`, `gap space.2-5`): **CurrentGameCard** · **ModeToggle**
  (Steam ⇄ umu segmented) · **Search field** (magnifier icon left, clear-X right
  when filled; input = `surface-2` fill, `border`, `radius.lg`, `font.size.xs`).
- Nav list (`px space.2`, item gap `space.0-5`, scrolls):
  - `Active options` (ListChecks, count) · `Recipes` (Sparkle)
  - section label **"PARAMETERS"** (`font.size.2xs`/medium/uppercase/`tracking.wider`/`color.muted`)
  - `Wrappers` + one row per visible category (Faders icon, count pill)
  - section label **"SETUP"** → `Game & runtime` (Cpu)
- **Nav item — inactive:** text `color.subtext`, hover fill `surface-2`,
  `px space.2-5 / py space.2`, `radius.lg`, `font.size.command` (13), icon 16.
- **Nav item — active:** fill `alpha.accent-16`, text `color.accent`, icon
  weight `fill`.
- **Count pill:** `radius.full`, `font.size.2xs`; inactive fill `alpha.accent-16`
  + `color.accent` text; active fill `alpha.accent-22`.

### Command bar (`CommandPreview` in a pinned footer)
- Footer: `shrink-0`, top border `color.border`, fill `alpha.mantle-30`,
  `px space.5 / py space.3`. Inner centered column max-w content-max.
- The card = §3.15 **Command preview** (gradient fill, mono syntax tokens),
  with a copy button and the **Apply to Heroic / Open in Steam** primary button.

---

## Screen 1 — Library (grid of games)

The landing view (`app.view === "library"`). No nav rail; no command bar.

```
HEADER
────────────────────────────────────────────────
 (optional banners: update / stale / errors)
 centered column, max-w library-max (1152), px space.6
 ┌ toolbar: search + sort + filters ─────────────┐
 └────────────────────────────────────────────────┘
 ┌ RESPONSIVE GRID of Game tiles (§3.12) ─────────┐
 │  ▢ ▢ ▢ ▢ ▢ ▢                                    │
 │  ▢ ▢ ▢ ▢ ▢ ▢   aspect 2/3, gap space.3          │
 └────────────────────────────────────────────────┘
```
- Tiles: build the `GameTile` component with these variant states to show off:
  default · hover (lifted, accent ring) · selected · **in-sync** (green check) ·
  **drifted** (peach warning) · **not-applied** (dashed accent) · **favourite**
  (gold star) · **shortcut badge** · **Heroic badge** · placeholder (no art).
- Draw ~12 tiles; reuse 2–3 real portrait placeholders + a couple glyph
  placeholders.

---

## Screen 2 — Builder · Advanced (the core screen)

`app.view === "builder"`, `uiMode === "advanced"`. Full shell: header + nav rail
+ scrolling parameter list + command bar.

```
HEADER
──────────┬───────────────────────────────────────
 NAV RAIL │  MainPanel: category header + list of
 (240)    │  Option rows (§3.7)
          │   ▸ [switch] Label ……… [value] [badges][i]
          │       help text…
          │   ▸ [switch] Label (ON, accent-09 bg) ……
          │   ▸ [switch] Label (dimmed 55%) ………
          ├───────────────────────────────────────
          │  COMMAND BAR → CommandPreview
```
- Center column max-w content-max (896), `px space.5 / py space.4`, `gap space.3`.
- Above the rows: **Notices** (lint chips) if any.
- Build a `MainPanel` frame containing 5–7 `OptionRow` instances covering every
  row variant: off, on, dimmed, with text field, with select, with segmented
  control, with recipe chip, with installed/missing badge, with search-match
  highlight.

---

## Screen 3 — Builder · Simple

`uiMode === "simple"`. **No nav rail** — full-width curated card view.

```
HEADER
────────────────────────────────────────────────
 centered column max-w content-max (896)
  CurrentGameCard
  ┌ curated cards (SimplePanel) — grouped toggles ┐
  │  a few high-value options as large rows        │
  └────────────────────────────────────────────────┘
────────────────────────────────────────────────
 COMMAND BAR → CommandPreview
```
- Reuse `OptionRow` and `Card`; the difference from Advanced is layout
  (no rail, curated subset), not new primitives.

---

## Screen 4 — Dialogs & overlays (one artboard, gallery style)

Lay these on a single artboard over a dimmed shell screenshot to show the scrim.
Scrim = `alpha.scrim-dialog` + `effect.blur.overlay`. All use the **Dialog**
component (§3.5).

1. **Import a command** — title + subtitle, mono `<textarea>` (surface-2/border/
   `radius.lg`), footer: Cancel (ghost) + *Parse & fill* (primary). Width 512.
2. **Save preset** — width `size.dialog-w-sm` (384), single text input
   (autofocus), Cancel + Save (primary, disabled when empty → `opacity.disabled-btn`).
3. **Settings drawer** (`SettingsDrawer`) — right-side drawer using the
   **Popover surface** styling (opaque, `shadow.popover`); contains the opt-in
   capability toggles (HDR, AMD generation → fsr4/rdna3/rdna4) as `Switch` rows +
   theme picker + path fields.
4. **Presets popover** (`width popover-w` 256) — global-profile button (accent
   outline), list of preset rows each with a delete (trash → hover danger), and
   a *Save current…* row.
5. **Info popover** — small opaque popover with details/example/default/docs link.
6. **Command palette** — centered search overlay (opaque popover surface, list of
   commands with `⌘`-style shortcut hints).
7. **Toast** (§3.14) — bottom, opaque popover surface, status icon + message.

---

## Screen 5 — Empty / loading / error states

One artboard, three panels:

1. **Loading:** centered spinner (`CircleNotch`, `size 28`, `color.accent`,
   spinning) on the gradient bg.
2. **Init error:** centered, max-w ~512 — danger heading with `WarningCircle`,
   body `color.subtext`, a `<pre>` stack-trace box (surface-2/border/`radius.xl`/
   mono `font.size.xs`), then **Retry** (primary) + **Copy details** (ghost).
3. **Banners strip:** the three banner variants stacked —
   - error: `alpha.danger-08` fill, `alpha.danger-35` border, `color.danger` text
   - warning: `alpha.warning-08` fill, `alpha.warning-35` border, icon `warning`
   - persist-error (`role=alert`): danger, bold lead line.
   All: `flex gap-2`, `px space.4 / py space.2-5`, `radius.xl`, `font.size.xs`,
   16px `WarningCircle` icon.

---

## Suggested Penpot page structure

- **Page: Foundations** — token swatches, type ramp, spacing/radius rulers,
  elevation samples (a visual audit of `design-system.md`).
- **Page: Components** — every component from §3 with its state variants.
- **Page: Screens** — one artboard per screen above (Library, Builder·Advanced,
  Builder·Simple, Overlays, States).

Build order: Foundations (import tokens first) → Components → Screens.
