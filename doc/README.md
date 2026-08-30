# protongen — Design docs (Penpot rebuild kit)

Everything needed to recreate protongen's UI in **Penpot** as a design-system
project: importable tokens, the full visual spec, and screen-by-screen
blueprints. Single theme: **Rosé Pine** (dark).

> These files describe design only. The app's *architecture* lives in the repo's
> top-level [`design.md`](../design.md) — different document, don't confuse them.

## Files

| File | What it is |
| --- | --- |
| [`tokens/protongen.tokens.json`](tokens/protongen.tokens.json) | **Import this into Penpot.** DTCG / Tokens-Studio-format token sets + the Rosé Pine theme. |
| [`design-system.md`](design-system.md) | The full spec — token tables, typography, all component recipes with states, iconography, layout. The reference. |
| [`screens.md`](screens.md) | Artboard blueprints for each app screen (Library, Builder·Advanced/Simple, Overlays, States) + suggested Penpot page structure. |

## Rebuild in Penpot — step by step

1. **New file** → open the **Tokens** tab (left panel).
2. **Import** → `tokens/protongen.tokens.json`. You'll get nine token sets
   (`primitives`, `semantic`, `alpha`, `typography`, `spacing`, `sizing`,
   `radius`, `border`, `opacity`) and one theme, **Rosé Pine**.
3. **Activate the theme** (Themes → Rosé Pine). `primitives` is the *source* set
   (raw values, not directly applied); everything else is enabled. Semantic
   tokens alias primitives, so re-theming later = swap `primitives` only.
4. **Add fonts:** upload **Lexend** (Regular 400 + Medium 500) — the TTFs live in
   the repo at [`public/fonts/`](../public/fonts/) (SIL OFL, redistributable).
   Mono is **JetBrains Mono** (fall back to any installed mono; only the command
   preview uses it).
5. **Foundations page:** drop swatches/type-ramp/rulers to verify tokens render
   (see `design-system.md` §2).
6. **Components page:** build each component in `design-system.md` §3 as a Penpot
   component, styling every property with a token (never a raw hex/px). Add the
   listed **state variants** (default/hover/active/focus/selected/disabled).
7. **Screens page:** assemble artboards per `screens.md` from those components at
   **1280 × 800**, on the gradient background.

## Notes & gotchas for Penpot

- **Alpha tints** (enabled-row fill, status chips, scrims) are precomputed as
  `rgba(…)` color tokens in the `alpha` set — Penpot can't compute CSS
  `color-mix`, so these are baked. In Rosé Pine `mauve == accent`, so both
  radial background glows and mauve chips use the iris color.
- **Composite tokens** (typography styles, box-shadows, gradients) aren't
  imported as single tokens — Penpot support is inconsistent. Build them from the
  primitive tokens: typography styles from `font.*` (recipes in §2.3), shadows
  from §2.9, gradients/blurs from §2.10. Values are all spelled out there.
- **Radius nuance:** the app leans on `rounded-lg` as its main control radius;
  this kit standardizes `radius.lg = 10px`. If you want pixel-exact Tailwind-4
  fidelity instead, set it to 8px (see `design-system.md` §2.6).
- **If a `$type` fails to import** on your Penpot version, it's usually a
  typography subtype — adjust the type string in the JSON (e.g. `fontSize` ↔
  `fontSizes`) and re-import that set. Colors, spacing, sizing, borderRadius,
  strokeWidth, opacity import reliably.

## Source of truth

Every value was extracted from the live app: `src/app.css` (token contract +
Rosé Pine palette), `src/lib/themes.ts`, and the Svelte components in
`src/lib/components/`. If the app's tokens change, re-derive from those.
