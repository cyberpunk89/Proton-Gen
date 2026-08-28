# CLAUDE.md

Guidance for Claude Code when working in this repo. For the full architecture, read
[`design.md`](design.md) — this file is the quick operational map.

## What this is

**protongen** — a polished desktop app that builds **Steam launch commands** and
**umu-launcher** commands for Proton on CachyOS. It scans installed Proton runtimes,
Steam games, and non-Steam shortcuts; lets you toggle a categorized, searchable catalog
of env-vars / wrappers (badged installed/missing, hardware-relevance filtered); and
previews + copies the resulting launch string live.

**Stack:** Tauri 2 shell · Rust 2024 backend (`protongen_lib`) · Svelte 5 (runes) +
TypeScript + Tailwind 4 frontend · Vite 6. UI primitives from `bits-ui`, icons from
`phosphor-svelte`.

## Layout

```
src/                     FRONTEND (Svelte 5 + TS + Tailwind)
  App.svelte             shell: Header + two-panel (builder left / Parameters right)
  lib/state.svelte.ts    single reactive store (runes) — the source of truth
  lib/ipc.ts + mock.ts   typed Tauri invoke + browser-dev mock fallback
  lib/types.ts           TS DTOs that MIRROR the Rust serde structs in ipc.rs
  lib/util.ts            irrelevance() (mirrors hardware.rs), matches(), tier colours
  lib/mangohud.ts        pure MANGOHUD_CONFIG parse/build for the overlay builder
  lib/components/*.svelte Hero, GamePicker, RuntimePicker, UmuFields, Parameters, …
  app.css + lib/themes.ts design tokens + 10 themes
src-tauri/               BACKEND (Rust / Tauri)
  src/lib.rs             wiring: run() + dump(); registers plugins & commands
  src/ipc.rs             Tauri command surface + AppState + DTOs
  src/builder.rs         PURE command assembly (Steam + umu) — the tested core
  src/params.rs          loads params.toml catalog; to_spec()
  src/recipes.rs         loads recipes.toml (profiles + troubleshooter)
  src/parser.rs          command → Config (inverse of builder)
  src/lint.rs            conflict / footgun notices
  src/store.rs           state.toml persistence + Config
  src/{steam,runtime,games,steamcfg}.rs   read-only discovery
  src/hardware.rs        GPU/session/ntsync detection + relevance
  params.toml            data-driven parameter catalog (single source of truth)
  recipes.toml           profiles + troubleshooter recipes
  capabilities/default.json   minimal Tauri permission set
```

## Commands

| Task | Command |
| --- | --- |
| UI dev (browser, **mock data**) | `pnpm dev` → Vite on :1420 (no Rust backend) |
| Full app dev (real backend) | `pnpm tauri dev` |
| Type check (TS↔Rust contract) | `pnpm check` (svelte-check) |
| FE build + check | `pnpm build` |
| Production bundle (.deb/AppImage) | `pnpm tauri build` |
| User-level install (no sudo) | `./install.sh` (uses `tauri build --no-bundle`) |
| Rust unit tests | `cd src-tauri && cargo test` |
| Discovery sanity CLI | `cargo run -- --list` (or `protongen --list`) |

**Gotcha:** a plain `cargo build --release` is **not** a valid app build — it leaves the
binary in dev mode trying to reach `localhost:1420`. Use `pnpm tauri build` /
`install.sh`. `install.sh` invokes the local Tauri CLI directly (pnpm would leak the
extra `--` into cargo).

## Conventions & invariants

- **Read-only by contract.** Never write Steam config files. The only output is a string
  the user copies. App state lives only under `$XDG_CONFIG_HOME/protongen/state.toml`. Three
  named exceptions, all explicit-confirm-only and documented in `design.md` §11:
  `heroic::inject` (writes Heroic's per-game config), `optiscaler_upgrade.rs` (fetches
  the latest OptiScaler release and extracts it into a game's own folder), and
  `mangohud_export.rs` (merges the MangoHud builder's overlay into the real, system-wide
  `MangoHud.conf`).
- **Pure Rust core, thin IPC.** Command assembly is a pure, deterministic, unit-tested
  function (`builder.rs`); Tauri commands are a serialization bridge. App, tests, and the
  `--list` CLI all consume the same logic.
- **Data-driven catalog.** `params.toml` / `recipes.toml` are baked in via `include_str!`
  and overridable by a user copy in `$XDG_CONFIG_HOME`. Adding an env var or recipe is a
  TOML edit — no Rust change (a new *wrapper program* does need a `Wrapper` enum variant).
- **Release versioning.** Semver `X.Y.Z`, with the number signalling the scope: **patch
  (`Z`)** = parameter/catalog refresh only (`params.toml`/`recipes.toml`, no code), **minor
  (`Y`)** = features / UI / backend, **major (`X`)** = milestone. Cut with
  `scripts/bump-version.sh X.Y.Z` (then `cargo update -p protongen` to sync `Cargo.lock`),
  commit `Release vX.Y.Z` on `main`, tag `vX.Y.Z`, push — the tag fires
  `.github/workflows/release.yml`, which builds + publishes the release the in-app updater reads.
- **Keep the DTO mirror in sync.** `src/lib/types.ts` interfaces mirror the serde structs
  in `src-tauri/src/ipc.rs` one-to-one. `pnpm check` enforces the FE side; update both.
- **One store, debounced recompute.** `state.svelte.ts` holds all selection; a root
  `$effect` serializes to a `Config` and (debounced ~60 ms) calls `build_command` + `lint`.
- **Browser-mock dev path.** `ipc.ts` detects `__TAURI_INTERNALS__`; outside Tauri it
  returns `mock.ts` data so the whole UI can be iterated with `pnpm dev`. Mock data has a
  reduced catalog — FSR/large catalogs and the native file dialog only exist in the real
  shell.
- **Relevance & opt-in capabilities.** `hardware.rs` **detects only**; the filter is
  `util.ts irrelevance()` and is frontend-only by design — there is deliberately no Rust
  mirror, because the opt-in capabilities live in the store. (A mirror existed, went three
  tags stale, and became dead code.) Capabilities that can't be auto-detected are **opt-in
  store flags** surfaced in Settings: `hdr`, and `fsr4`/`rdna3`/`rdna4` derived from the
  AMD generation selector. To hide a parameter by default, tag it `needs = ["<cap>"]` and
  thread the cap through `store.rs`, `types.ts`, `state.svelte.ts` (`EMPTY_STORE` +
  `hwCaps` + setter), `util.ts`, `mock.ts`, and a `SettingsDrawer.svelte` toggle. **Also
  add it to `params::KNOWN_NEEDS`** — `irrelevance()` treats an unhandled tag as *relevant*,
  so a half-added capability filters nothing and reports no error (that is exactly how
  `rdna4` shipped in the selector but never in the filter); the `bundled_needs_tags_are_known`
  test is the guard. Hidden options stay revealable via "Show all".
  The same six layers apply to any new `Store` field — `store.paths` is a nested struct,
  so it also needs a "partial table still loads" test, and remember `save_store`
  overwrites the store wholesale (#43): anything the frontend doesn't round-trip is lost.
- **Never write a bare attribute after `{...props}`.** bits-ui's `child` snippets hand
  back a merged prop bag: a **style string** carrying `pointer-events: auto` (its modal
  layers set `body { pointer-events: none }`), `onkeydown` for the Tab focus trap, and
  load-bearing layout style on `Select.Content`. A literal `style=`/`on*=` written after
  the spread is a later key in the same compiled object literal, so it replaces that
  value silently — no error, no warning, `pnpm check` clean. It made every modal in the
  app click-dead once (#63). Route extra inline styles through `mergeStyle()` in
  `util.ts`. Enforced by `scripts/check-props-spread.sh`, which `pnpm check` runs.
  `bits-ui` is **pinned exactly** for the same reason: the app depends on internal prop
  shapes that a minor bump has already changed once.
- **Minimal Tauri capabilities** (`capabilities/default.json`): core, opener, clipboard,
  dialog. Adding a plugin = `Cargo.toml` dep + `.plugin(...)` in `lib.rs` + a capability
  permission + the JS `@tauri-apps/plugin-*` package. `opener:default` only scopes
  `mailto:`/`tel:`/`http(s):`, so the `steam://` deep links carry an explicit
  `opener:allow-open-url` entry — deliberately narrowed to `steam://gameproperties/*`
  and `steam://nav/*`. Never widen it to `steam://*`: that would also permit mutating
  verbs like `steam://uninstall/<id>`, against the read-only invariant.
- **`decorations: false` means the CSD owns four things**, not one: move
  (`data-tauri-drag-region` in `Header.svelte`), minimize/maximize/close (its buttons),
  and **resize** (`ResizeGrips.svelte` → `core:window:allow-start-resize-dragging`).
  Resize was missing until #64. Re-enabling decorations means removing all four.
- **umu / Proton GE.** umu mode emits `PROTONPATH=<runtime path>`. A synthetic
  "GE-Proton (latest · umu auto-download)" runtime uses `path="GE-Proton"` (the codename
  umu resolves & auto-downloads), so it always targets the newest GE-Proton with no version
  pin. Non-Steam shortcuts prefill the umu exe.
- **Verifying UI changes:** run `pnpm dev` and drive it with the Preview MCP tools
  (`.claude/launch.json` defines the `protongen-web` server on :1420). Note Svelte re-renders
  in a microtask, so when scripting via `preview_eval`, click in one call and read state in
  the next.

## Skills in this repo

- **`update-proton-params`** (`.claude/skills/update-proton-params/SKILL.md`) — refreshes
  `src-tauri/params.toml` from upstream docs (Proton README, proton-cachyos
  changelog/README, CachyOS wiki, vkd3d-proton, DXVK). Use it when Proton ships new env
  vars, after a version bump, when the user says "update proton params" / "refresh the
  catalog", or when the in-app "catalog stale" banner appears. It updates entries +
  relevance hints + the `[meta]` build/date, never touches the user's XDG override, and
  verifies with `cargo test` + `cargo run -- --list`.
```
