# protongen — Design System (Penpot input)

A build-ready specification of protongen's visual language, written to be
transcribed into a **Penpot design system**: token sets first, then components
expressed in those tokens. Everything here is extracted from the live app
(`src/app.css`, `src/lib/themes.ts`, and the Svelte components in
`src/lib/components/`).

**Single theme: Rosé Pine (dark).** The app ships ten themes, but per the brief
this system defines exactly one. Every value below is the resolved Rosé Pine
value — no `var()` indirection, no light mode.

> **How to use this in Penpot**
> 1. Create one **Token Set** per section under "Token system" (Primitives,
>    Semantic color, Typography, Spacing, Sizing, Radius, Border, Opacity,
>    Elevation, Effects).
> 2. Create one **Theme** named `Rosé Pine` that activates all sets.
> 3. Primitive tokens hold raw hex. Semantic tokens **alias** primitives
>    (`{palette.accent}`) so the whole UI can be re-themed later by swapping the
>    primitive set only.
> 4. Build the components in "Component library" as Penpot components, styling
>    every property with a token — never a raw value.

---

## 1. Design principles

- **Calm, dim, low-glare.** Deep near-black indigo background, soft muted-lilac
  accent, generous translucency. Nothing is pure white or pure black except
  intentional overlays and control knobs.
- **Layered translucency, not hard panels.** Large surfaces are semi-transparent
  cards with a 12px backdrop blur floating over a two-radial-gradient background.
  Elevated surfaces (popovers, dialogs, dropdowns) go opaque + shadowed instead.
- **Type is quiet and functional.** Lexend for UI, a mono family for the command
  string. Small sizes (11–14px dominate), medium weight for emphasis, regular for
  the rest. Only two weights are ever used.
- **One accent, semantic status colors.** Lilac accent drives interactivity;
  green/red/yellow/peach carry status meaning. Accent is reused for focus rings,
  selection, active states, and tinted fills at low alpha.
- **Everything rounded.** `rounded-lg` (10px) is the workhorse; pills for
  chips/badges/switches; larger radii for cards and tiles.
- **Accessible focus is non-negotiable.** A 2px accent outline at 2px offset on
  every interactive element via `:focus-visible`, following each element's own
  radius.

---

## 2. Token system

### 2.1 Primitive color tokens (`palette.*`)

The raw Rosé Pine ramp. Nothing in the UI should reference these directly except
the semantic layer.

| Token | Hex | Note |
| --- | --- | --- |
| `palette.base` | `#191724` | Rosé Pine "Base" — app background |
| `palette.surface` | `#1f1d2e` | Rosé Pine "Surface" |
| `palette.overlay` | `#26233a` | Rosé Pine "Overlay" |
| `palette.overlay-hi` | `#393552` | Rosé Pine "Highlight Med" |
| `palette.text` | `#e0def4` | Rosé Pine "Text" |
| `palette.subtle` | `#c9c7e0` | between Text and Subtle |
| `palette.muted` | `#908caa` | Rosé Pine "Muted" |
| `palette.iris` | `#c4a7e7` | Rosé Pine "Iris" — accent |
| `palette.foam` | `#9ccfd8` | Rosé Pine "Foam" — used as "green"/success |
| `palette.love` | `#eb6f92` | Rosé Pine "Love" — used as "red"/danger |
| `palette.gold` | `#f6c177` | Rosé Pine "Gold" — used as "yellow"/warning |
| `palette.rose` | `#ebbcba` | Rosé Pine "Rose" — used as "peach" |
| `palette.pine` | `#31748f` | Rosé Pine "Pine" — used as "blue" |
| `palette.white` | `#ffffff` | control knobs, art captions |
| `palette.black` | `#000000` | scrim/overlay base |

> Naming note: the app's CSS calls these `--green/--red/--yellow/--peach/--blue`
> for cross-theme consistency, but in Rosé Pine they resolve to Foam/Love/Gold/
> Rose/Pine. The semantic layer below uses role names, so this mismatch never
> leaks into components.

### 2.2 Semantic color tokens (`color.*`)

These are what components reference. Each **aliases** a primitive.

**Surfaces & structure**

| Token | Value / alias | Role |
| --- | --- | --- |
| `color.bg` | `{palette.base}` `#191724` | window background base (behind gradients) |
| `color.mantle` | `#1f1d2e` | deepest inset panel / darker zone |
| `color.surface` | `#1f1d2e` @ **28%** → `rgba(31,29,46,0.28)` | translucent card fill (`--surface` = `#1f1d2e48`) |
| `color.surface-solid` | `#1f1d2e` | opaque elevated surface (popover/dialog/dropdown) |
| `color.surface-2` | `#26233a` | solid raised control fill (inputs, switch-off, chips) |
| `color.overlay` | `#393552` | scrollbar thumb, hairline dividers |
| `color.border` | `#26233a` | default border color |

**Text**

| Token | Value | Role |
| --- | --- | --- |
| `color.text` | `#e0def4` | primary text, enabled labels, titles |
| `color.subtext` | `#c9c7e0` | secondary text, disabled-but-present labels |
| `color.muted` | `#908caa` | help text, captions, placeholders, quiet icons |

**Accent & status**

| Token | Value | Role |
| --- | --- | --- |
| `color.accent` | `#c4a7e7` | primary interactive, focus ring, selection, active |
| `color.on-accent` | `#191724` | text/icon on an accent fill |
| `color.success` | `#9ccfd8` | "installed / in-sync" |
| `color.danger` | `#eb6f92` | "missing / error" |
| `color.warning` | `#f6c177` | favourite star, caution |
| `color.attention` | `#ebbcba` | "drifted / not pasted" (peach) |
| `color.info` | `#31748f` | Heroic badge, links (blue) |
| `color.mauve` | `#c4a7e7` | recipe chips, shortcut badge (== accent in R.P.) |

**Command-string syntax tokens (`color.tok.*`)** — used only in the tokenized
command preview (mono, 13px). Tuned for WCAG AA on the card background.

| Token | Value | Applies to |
| --- | --- | --- |
| `color.tok.env` | `#5597b3` | `KEY=` env-var names (overridden from `--blue` for contrast) |
| `color.tok.value` | `#e0def4` | env-var values (`{color.text}`) |
| `color.tok.wrapper` | `#c4a7e7` | wrapper program name (`{color.mauve}`) |
| `color.tok.wrapper-arg` | `#c9c7e0` | wrapper args (`{color.subtext}`) |
| `color.tok.separator` | `#908caa` | `--`, `%command%` (`{color.muted}`) |
| `color.tok.target` | `#c4a7e7` | `%command%` target (`{color.accent}`) |
| `color.tok.exe` | `#9ccfd8` | executable path (`{color.success}`) |
| `color.tok.arg` | `#ebbcba` | trailing args (`{color.attention}`) |
| `color.tok.unknown` | `#c9c7e0` | unrecognized token (`{color.subtext}`) |

### 2.3 Typography tokens

**Font families**

| Token | Value |
| --- | --- |
| `font.sans` | `Lexend, ui-sans-serif, system-ui, sans-serif` |
| `font.mono` | `"JetBrains Mono", "Fira Code", ui-monospace, monospace` |

**Font weights** (only two exist in the app)

| Token | Value |
| --- | --- |
| `font.weight.regular` | `400` |
| `font.weight.medium` | `500` |

**Font size scale** (px; ordered by real usage frequency)

| Token | Size | Typical use |
| --- | --- | --- |
| `font.size.2xs` | `10px` | tile badges, recipe chips |
| `font.size.xs-plus` | `11px` | relevance/status pills, dense metadata |
| `font.size.xs` | `12px` | help text, inputs, most secondary UI (workhorse small) |
| `font.size.command` | `13px` | monospace command body, mono labels |
| `font.size.sm` | `14px` | option labels, body, dialog description (workhorse body) |
| `font.size.base` | `16px` | rarely; default body |
| `font.size.lg` | `18px` | dialog titles, section headings |

**Line height**

| Token | Value | Use |
| --- | --- | --- |
| `line.snug` | `1.375` | multi-line labels, help text, tile titles |
| `line.normal` | `1.5` | default body |

**Text styles (composite — build as Penpot Typography tokens)**

| Style | Family / Size / Weight / Line / Color |
| --- | --- |
| `Title / Dialog` | sans · 18 · 500 · snug · `color.text` |
| `Body` | sans · 14 · 400 · normal · `color.text` |
| `Label / Option (on)` | sans · 14 · 500 · normal · `color.text` |
| `Label / Option (off)` | sans · 14 · 400 · normal · `color.subtext` |
| `Help` | sans · 12 · 400 · snug · `color.muted` |
| `Pill / Status` | sans · 11 · 400 · normal · (contextual) |
| `Badge / Tile` | sans · 10 · 500 · normal · (contextual) |
| `Mono / Command` | mono · 13 · 400 · snug · (per-token color) |
| `Mono / Input` | mono · 12 · 400 · normal · `color.text` |

### 2.4 Spacing scale (`space.*`)

Tailwind 0.25rem step. Only the values the app actually uses are listed;
`space.2` (8px) and `space.1-5` (6px) are the two most common gaps.

| Token | rem | px |
| --- | --- | --- |
| `space.0-5` | 0.125 | 2 |
| `space.1` | 0.25 | 4 |
| `space.1-5` | 0.375 | 6 |
| `space.2` | 0.5 | 8 |
| `space.2-5` | 0.625 | 10 |
| `space.3` | 0.75 | 12 |
| `space.4` | 1 | 16 |
| `space.5` | 1.25 | 20 |
| `space.6` | 1.5 | 24 |
| `space.8` | 2 | 32 |
| `space.10` | 2.5 | 40 |
| `space.12` | 3 | 48 |

### 2.5 Sizing tokens (`size.*`)

| Token | px | Use |
| --- | --- | --- |
| `size.icon.xs` | 9–12 | inline pill/chip glyphs |
| `size.icon.sm` | 13–16 | button icons, close, status dots |
| `size.icon.md` | 20 | tile status circle contents (`size-5`) |
| `size.icon.lg` | 34 | empty-tile placeholder glyph |
| `size.switch.w` | 38 | switch track width |
| `size.switch.h` | 22 | switch track height |
| `size.switch.knob` | 16 | switch knob (`size-4`) |
| `size.field.w` | 160 | inline text/select field width (`w-40`) |
| `size.navrail.w` | 240 | left navigation rail (`w-60`) |
| `size.dialog.w` | 512 | default dialog width (`32rem`) |
| `size.tile.ratio` | 2 / 3 | game tile aspect ratio (portrait) |
| `size.dot.status` | 10 | protondb tier dot (`size-2.5`) |

### 2.6 Radius tokens (`radius.*`)

| Token | px | Use |
| --- | --- | --- |
| `radius.xs` | 3 | search-match `<mark>` highlight |
| `radius.sm` | 6 | small internal (`rounded-md`) |
| `radius.md` | 8 | (`rounded-lg` is 10 in TW4; see below) |
| `radius.lg` | 10 | **workhorse** — buttons, inputs, rows, chips (`rounded-lg`) |
| `radius.xl` | 12 | tiles, option rows (`rounded-xl`) |
| `radius.popover` | 14 | popover/dropdown/drawer surfaces |
| `radius.card` | 16 | large cards + dialogs (`--radius-card`) |
| `radius.2xl` | 16 | (`rounded-2xl`) |
| `radius.full` | 9999 | pills, badges, switch, avatars, dots |

> Tailwind 4 defaults: `rounded-md`=6px, `rounded-lg`=8px… but this app treats
> `rounded-lg` as its primary control radius. In Penpot, standardize on
> `radius.lg = 10px` for buttons/inputs/chips and `radius.xl = 12px` for
> cards-in-panels to match the rendered feel; `radius.card = 16px` for top-level
> cards and dialogs.

### 2.7 Border tokens (`border.*`)

| Token | Value | Use |
| --- | --- | --- |
| `border.width.hair` | `1px` | default border everywhere |
| `border.width.ring` | `2px` | focus ring, selected/active ring, flash |
| `border.color.default` | `{color.border}` `#26233a` | inputs, dropdowns, segmented |
| `border.color.card` | `#26233a` @ 70% → `rgba(38,35,58,0.70)` | translucent card border |
| `border.color.tile` | `#26233a` @ 60% → `rgba(38,35,58,0.60)` | tile ring (`ring-border/60`) |
| `border.color.focus` | `{color.accent}` `#c4a7e7` | focus outline + accent ring |
| `border.color.input-focus` | `{color.accent}` | input border on focus |

### 2.8 Opacity & tint tokens (`alpha.*`)

`color-mix(… X%, transparent)` in the source is just an alpha channel. These are
the exact precomputed Rosé Pine tints. Build them in Penpot as color tokens with
alpha (or as opacity tokens applied to a fill).

**Accent tints (`#c4a7e7`)**

| Token | Value | Use |
| --- | --- | --- |
| `alpha.accent.07` | `rgba(196,167,231,0.07)` | command card gradient (top) |
| `alpha.accent.09` | `rgba(196,167,231,0.09)` | enabled option-row background |
| `alpha.accent.16` | `rgba(196,167,231,0.16)` | recipe chip fill (mauve 16%) |
| `alpha.accent.22` | `rgba(196,167,231,0.22)` | segmented active fill |
| `alpha.accent.25` | `rgba(196,167,231,0.25)` | search-match highlight |
| `alpha.accent.35` | `rgba(196,167,231,0.35)` | text selection (`::selection`) |

**Status tints (18% fills for installed/missing chips)**

| Token | Value |
| --- | --- |
| `alpha.success.18` | `rgba(156,207,216,0.18)` |
| `alpha.danger.18` | `rgba(235,111,146,0.18)` |
| `alpha.mauve.75` | `rgba(196,167,231,0.75)` (shortcut tile badge) |
| `alpha.info.75` | `rgba(49,116,143,0.75)` (Heroic tile badge) |

**Scrims & overlays (on `palette.black`)**

| Token | Value | Use |
| --- | --- | --- |
| `alpha.scrim.dialog` | `rgba(0,0,0,0.50)` | dialog backdrop (`bg-black/50`) |
| `alpha.scrim.badge` | `rgba(0,0,0,0.45)` | tile badge background |
| `alpha.scrim.caption` | `rgba(0,0,0,0.85)`→transparent | tile title gradient |
| `alpha.scrim.ring` | `rgba(0,0,0,0.40)` | tier-dot ring |

**White tints (on-art elements)**

| Token | Value | Use |
| --- | --- | --- |
| `alpha.white.70` | `rgba(255,255,255,0.70)` | inactive favourite star |
| `color.on-art` | `#ffffff` | tile title text, active star base |

### 2.9 Elevation tokens (`shadow.*`)

| Token | Value | Use |
| --- | --- | --- |
| `shadow.switch` | `0 1px 2px rgba(0,0,0,0.30)` (TW `shadow`) | switch knob |
| `shadow.popover` | `0 1px 0 rgba(224,222,244,0.06) inset, 0 10px 30px -10px rgba(0,0,0,0.55), 0 4px 12px -6px rgba(0,0,0,0.45)` | popover / dropdown / drawer |
| `shadow.dialog` | TW `shadow-2xl` = `0 25px 50px -12px rgba(0,0,0,0.55)` | dialogs, floating panels |

### 2.10 Effect tokens (`effect.*`)

| Token | Value | Use |
| --- | --- | --- |
| `effect.blur.card` | `blur(12px)` backdrop | translucent cards |
| `effect.blur.badge` | `blur(2px)` backdrop | on-art badges/stars |
| `effect.blur.overlay` | `blur(4px)` backdrop | dialog scrim (`backdrop-blur-sm`) |
| `effect.gradient.bg` | see below | window background |
| `effect.gradient.card` | `linear-gradient(180deg, {alpha.accent.07}, {color.mantle})` | command preview card |
| `effect.gradient.caption` | `linear-gradient(to top, rgba(0,0,0,0.85), rgba(0,0,0,0.45), transparent)` | tile title legibility |

**`effect.gradient.bg`** (the app's window background — layer these):
```
radial-gradient(1200px 600px at 80% -10%, rgba(196,167,231,0.10), transparent 60%),
radial-gradient(1000px 500px at -10% 10%, rgba(196,167,231,0.08), transparent 55%),
#191724
```
(Both radial tints use Iris in Rosé Pine, since `--mauve == --accent` here.)

### 2.11 Motion tokens (`motion.*`)

| Token | Value | Use |
| --- | --- | --- |
| `motion.fast` | `150ms` | tile hover, generic `transition` |
| `motion.base` | `200ms` | switch track/knob, color transitions |
| `motion.slow` | `300ms` | tile art scale-in |
| `motion.press` | `scale(0.95)` | button `active:` state |
| `motion.easing` | ease (default) | all |
| `motion.reduced` | `0.01ms` | honor `prefers-reduced-motion` |

---

## 3. Component library

Each component lists geometry in tokens. Build as Penpot components with
variants for the states noted. **All focus states** share the global ring:
`2px solid {color.accent}`, offset `2px`, radius = element radius.

### 3.1 Button — Primary (accent)

- Fill `{color.accent}`, text/icon `{color.on-accent}`, `font.size.xs` (12) /
  `font.weight.medium`.
- Padding `py-1.5 px-2.5` → V `space.1-5` (6), H `space.2-5` (10).
- Radius `radius.lg`. Inline-flex, `gap-1.5` (6) icon↔label, icon `size.icon.sm` (14).
- **Hover:** opacity 90%. **Active:** `scale(0.95)`. **Focus:** global ring.
- Variant `collapsible`: label hidden at narrow widths, icon-only.

### 3.2 Button — Ghost / Icon

- No fill. Text `{color.muted}` → hover `{color.text}`.
- Padding `p-1` (4). Radius `radius.lg`. Icon `size.icon.sm` (16 for close).
- Used for dialog close, toolbar actions.

### 3.3 Card (translucent panel)

- Fill `{color.surface}` (28% surface). Border `border.color.card` (1px).
- Radius `radius.card` (16). Backdrop `effect.blur.card` (12px).
- Padding typically `space.4`–`space.5` (16–20).

### 3.4 Popover / Dropdown / Drawer surface

- Fill `{color.surface-solid}` (opaque `#1f1d2e`). Border `border.color.default` (1px).
- Radius `radius.popover` (14). **No backdrop blur.** Shadow `shadow.popover`.

### 3.5 Dialog

- Scrim: full-viewport `alpha.scrim.dialog` + `effect.blur.overlay`.
- Panel: **Card** styling but fill forced to `{color.surface-solid}`, padding
  `space.5` (20), `shadow.dialog`, `max-width {size.dialog.w}` (512), centered
  horizontally, top offset `12vh`.
- Header: `flex gap-3`; Title = `Title / Dialog` text style; optional
  Description = `Body`/`color.muted` with `space.1` top margin; close button
  (Ghost/Icon) top-right.
- Body: `space.4` top margin, `max-height 70vh`, scrolls internally.

### 3.6 Switch (toggle)

- Track: `size.switch.w` × `size.switch.h` (38×22), `radius.full`.
  Off fill `{color.surface-2}`, On fill `{color.accent}`. Transition `motion.base`.
- Knob: `size.switch.knob` (16), `radius.full`, fill `palette.white`,
  `shadow.switch`. Position: `left 3px` off → `left 19px` on. Top `3px`.
- States: Off, On, Focus (global ring on track).

### 3.7 Option row

- Container: radius `radius.xl` (12), padding `px-3 py-2` (H `space.3`, V `space.2`).
- **Enabled:** background `alpha.accent.09`; label `{color.text}` weight 500.
- **Disabled/off:** transparent; hover `{color.surface-2}` @ 40%; label
  `{color.subtext}` weight 400.
- **Dimmed (irrelevant):** opacity 55%.
- **Flash (just-jumped-to):** `border.width.ring` accent ring for 1200ms.
- Layout: `flex items-center gap-3` → [Switch] [Label (truncate, flex-1)]
  [optional value field] [trailing cluster: recipe chip · action · badges · info].
- Help line: `Help` text style, top margin `space.1`, left indent `50px` (aligns
  under the label past the switch).

### 3.8 Text input

- Fill `{color.surface-2}`, border `border.color.default` (1px), radius `radius.lg`.
- Padding `px-2 py-1` (H `space.2`, V `space.1`). Width `size.field.w` (160).
- Text `Mono / Input` (mono 12) `{color.text}`; placeholder `{color.muted}`.
- **Focus:** border → `{color.accent}` (no default outline; border replaces it).

### 3.9 Select

- Same box as text input (`surface-2`, `border`, `radius.lg`, `px-2 py-1`, 160px)
  but sans text at `font.size.xs`.

### 3.10 Segmented control

- Wrapper: inline-flex, `border.color.default` (1px), `radius.lg`,
  `overflow: hidden`.
- Segment: `px-3 py-1` (H `space.3`, V `space.1`), mono `font.size.xs`.
  Divider between segments = 1px left border `border.color.default`.
- **Active:** fill `alpha.accent.22`, text `{color.accent}`, weight 500.
- **Inactive:** text `{color.muted}` → hover `{color.subtext}`.
- Focus: inset ring (offset −2) because the wrapper clips.

### 3.11 Chips & badges (pills — `radius.full`)

| Variant | Fill | Text | Size | Padding |
| --- | --- | --- | --- | --- |
| Relevance ("won't apply") | `{color.surface-2}` | `{color.muted}` | 11 | `px-2 py-0.5` |
| Installed | `alpha.success.18` | `{color.success}` | 11 | `px-2 py-0.5` + 12px check icon |
| Missing | `alpha.danger.18` | `{color.danger}` | 11 | `px-2 py-0.5` + 12px x icon |
| Recipe ("Set by …") | `alpha.accent.16` | `{color.mauve}` | 10 | `px-1.5 py-0.5` + 9px sparkle |
| Tile: shortcut | `alpha.mauve.75` | `{color.on-accent}` | 10 | `px-1.5 py-0.5`, `blur(2px)` |
| Tile: Heroic | `alpha.info.75` | `{color.on-accent}` | 10 | `px-1.5 py-0.5`, `blur(2px)` |

Pill padding: H `space.2` (8) or `space.1-5` (6), V `space.0-5` (2). Icon↔text
gap `space.1` (4).

### 3.12 Game tile

- Frame: aspect `2/3`, `radius.xl` (12), `overflow: hidden`, fill
  `{color.surface-2}`, ring 1px `border.color.tile`.
- **Hover:** translateY −4px (`-translate-y-1`), ring `border.width.ring`
  `{color.accent}`, art scales to 1.04 over `motion.slow`. Transition `motion.fast`.
- **Selected / focus-within:** ring `border.width.ring` `{color.accent}`.
- Art `<img>` cover; placeholder = centered `GameController` glyph
  `size.icon.lg` (34) in `{color.muted}`.
- Title strip: bottom, `effect.gradient.caption`, padding `px-2.5 pb-2 pt-8`,
  text `Badge/Tile`-ish → 12/500/`line.snug`/`color.on-art`, 2-line clamp.
- Top-left badge cluster: shortcut/Heroic pill + tier dot (`size.dot.status` 10,
  `radius.full`, 1px `alpha.scrim.ring` ring).
- Top-right cluster: status circle (`size-5` `size.icon.md`, `radius.full`,
  `alpha.scrim.badge` fill, `blur(2px)`, colored glyph) + favourite star button.

**Tile status semantics** (icon + color):

| State | Icon (Phosphor) | Color |
| --- | --- | --- |
| Applied / in-sync | `CheckCircle` (fill) | `{color.success}` |
| Drifted / not pasted | `WarningCircle` (fill) | `{color.attention}` |
| Saved, not in Steam | `CircleDashed` (bold) | `{color.accent}` |
| Steam has foreign opts | `Circle` (bold) | `{color.muted}` |
| Favourite (on) | `Star` (fill) | `{color.warning}` |
| Favourite (off) | `Star` (bold) | `alpha.white.70` → hover `color.on-art` |

### 3.13 Navigation rail

- Width `size.navrail.w` (240), full height, right border `{color.border}`, fill
  `alpha.mantle-30` (`rgba(31,29,46,0.30)`).
- Item: icon (16) + label (`font.size.command` 13, truncate) + optional count
  pill; `radius.lg`, padding `px-2.5 py-2` (H `space.2-5`, V `space.2`),
  item gap `space.0-5`.
- **Inactive:** text `{color.subtext}`, hover fill `{color.surface-2}`, icon
  weight `regular` (category icons in `{color.muted}`).
- **Active:** fill `alpha.accent.16`, text `{color.accent}`, icon weight `fill`.
- **Count pill:** `radius.full`, `font.size.2xs`, `px-1.5`; inactive =
  `alpha.accent.16` fill + `{color.accent}` text, active = `alpha.accent.22` fill.
- **Section labels** ("PARAMETERS", "SETUP"): `font.size.2xs`, weight 500,
  uppercase, `font.tracking.wider` (0.05em), `{color.muted}`, padding `px-2 pt-3 pb-1`.

### 3.14 Toast

- Elevated **Popover** surface (opaque, `shadow.popover`), `radius.popover`,
  compact padding (`space.3`), `Body`/`font.size.xs` text, optional status color
  bar or icon using the status palette.

### 3.15 Command preview (syntax-highlighted string)

- Card with `effect.gradient.card` fill, `radius.card`.
- Body text = `Mono / Command` (mono 13 / `line.snug`), each token painted with
  its `color.tok.*` token (§2.2). This is the app's signature surface — get the
  per-token colors exact.

---

## 4. States reference (global)

| State | Treatment |
| --- | --- |
| **Focus (keyboard)** | `2px solid {color.accent}`, offset 2px, follows radius. `:focus-visible` only — never on mouse. |
| **Hover (button)** | opacity 90% (filled) or text `muted`→`text` (ghost). |
| **Active / pressed** | `scale(0.95)`. |
| **Selected / current** | 2px accent ring and/or `alpha.accent.09`–`.22` fill. |
| **Disabled / irrelevant** | opacity 55%, or muted text + surface-2 pill. |
| **Loading art** | placeholder glyph on `surface-2`. |
| **Reduced motion** | all transitions/animations → `0.01ms`. |

---

## 5. Iconography

- **Library:** Phosphor (`phosphor-svelte`). Weights used: `regular`, `bold`,
  `fill`. Match weight to meaning (status glyphs are `fill`/`bold`).
- **Sizes:** 9 / 12 / 13 / 14 / 16 (inline & buttons), 20 (tile status), 34
  (empty placeholder). Map to `size.icon.*`.
- **Color:** inherit text color; status glyphs take the status palette.

---

## 6. Layout & structure

- **Shell:** Header (drag region + window controls, `py-1.5`/`py-2` vertical
  rhythm) over a two-panel body — builder left, Parameters right — with a 240px
  NavRail. Window is `decorations: false` (custom title bar), so the design owns
  move / min / max / close / resize affordances.
- **Background:** `effect.gradient.bg` fills the whole window; cards float over it.
- **Scrolling:** page never scrolls (`overflow: hidden` on body); inner regions
  scroll. Custom thin scrollbar: thumb `{color.overlay}` → hover `{color.muted}`,
  `radius.full`, 10px track.
- **Container queries:** several controls collapse label→icon at narrow widths
  (`@2xl`), so build responsive/variant versions where noted (`collapsible`).

---

## 7. Penpot build checklist

1. **Primitives set** — §2.1 as raw color tokens.
2. **Semantic set** — §2.2 aliasing primitives (incl. `color.tok.*`).
3. **Alpha set** — §2.8 precomputed rgba tints.
4. **Typography set** — §2.3 families/weights/sizes/line-heights + composite
   text styles.
5. **Dimension sets** — §2.4 spacing, §2.5 sizing, §2.6 radius, §2.7 borders.
6. **Elevation/effect/motion sets** — §2.9–§2.11.
7. **Theme** `Rosé Pine` — activate all sets.
8. **Components** — build §3 in order (Button → Card → Popover → Dialog → Switch
   → Option row → Inputs → Chips → Tile → Rail → Toast → Command preview),
   token-styling every property, with the variants each lists.
9. **States** — encode §4 as component variants / interaction states.
```
