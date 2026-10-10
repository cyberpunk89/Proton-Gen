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
  App.svelte             shell: Header + Library view, or builder view (NavRail + MainPanel
                         in Advanced / SimplePanel in Simple) with CommandPreview pinned below;
                         root-mounted dialogs (CommandPalette, LogViewer, Troubleshooter, …)
  lib/state.svelte.ts    single reactive store (runes) — the source of truth
  lib/art.svelte.ts      game-art cache · lib/lookups.svelte.ts  ProtonDB / anti-cheat caches
  lib/ipc.ts + mock.ts   typed Tauri invoke + browser-dev mock fallback
  lib/types.ts           TS DTOs that MIRROR the Rust serde structs in ipc.rs
  lib/contract.ts        compile-time check of types.ts against lib/generated/ (ts-rs, from cargo test)
  lib/util.ts            irrelevance() (the relevance filter), mergeStyle(), tier colours
  lib/shell.ts           POSIX env tokenize/quote — the TS twin of builder::sh_quote
  lib/fuzzy.ts           fuzzy() matcher: palette, parameter search, library filter
  lib/markdown.ts        LLM text → plain blocks for Markdown.svelte (never {@html})
  lib/presetCode.ts      `protongen:v1:` preset share codes (strict, untrusted decode)
  lib/mangohud.ts        pure MANGOHUD_CONFIG parse/build for the overlay builder
  lib/gamescope.ts · dlloverrides.ts · compare.ts   pure builder / compare logic
  lib/*.test.ts          vitest unit tests (`pnpm test`)
  lib/components/*.svelte Library, NavRail, MainPanel, SimplePanel, CommandPreview,
                         SettingsDrawer, CommandPalette, OverlayBuilders, LogViewer, …
  app.css + lib/themes.ts design tokens + 10 themes
src-tauri/               BACKEND (Rust / Tauri)
  src/lib.rs             wiring: run() + dump(); registers plugins & commands (single-instance first)
  src/cli.rs             --list --json / --game-config / --catalog JSON (schema 1) for scripts + Nexus
  src/ipc.rs             Tauri command surface + AppState + DTOs
  src/builder.rs         PURE command assembly (Steam + umu) — the tested core
  src/params.rs          loads params.toml catalog; to_spec()
  src/recipes.rs         loads recipes.toml (profiles + troubleshooter)
  src/parser.rs          command → Config (inverse of builder)
  src/lint.rs            conflict / footgun notices
  src/store.rs           state.toml persistence + Config (corrupt file → quarantined)
  src/nexus.rs           Nexus library (games.json) → one entry per repack; nexus-cli --set-launch
  src/logs.rs            log sources (Proton/umu/PROTON_LOG_DIR, Nexus, DXVK, VKD3D) for the viewer
  src/folders.rs         a game's prefix + shader cache (paths, sizes, open)
  src/conf_merge.rs      shared merge+write core of the MangoHud/vkBasalt exports
  src/anticheat.rs       opt-in AreWeAntiCheatYet lookup (cached list)
  src/fsutil.rs          config_home(), write_atomic() (temp + rename; writes through symlinks, keeps perms),
                         write_backup() (collision-proof .bak), read_existing() — store/Heroic/overlay writes
  src/{steam,runtime,games,steamcfg}.rs   read-only discovery
  src/runtime_updates.rs "newer GE-Proton / proton-cachyos out" notice (report only)
  src/lsfg.rs            lsfg-vk (Lossless Scaling frame gen): layer, conf.toml profiles,
                         Lossless.dll — read-only; the builder is LosslessScaling.svelte + lib/lsfg.ts
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
| Frontend unit tests | `pnpm test` (vitest) |
| Rust unit tests | `cd src-tauri && cargo test` (also regenerates `src/lib/generated/`) |
| JSON for scripts / Nexus | `protongen --list --json`, `--game-config <id> [--umu]`, `--catalog --json` |
| Discovery sanity CLI | `cargo run -- --list` (or `protongen --list`) |

**Gotcha:** a plain `cargo build --release` is **not** a valid app build — it leaves the
binary in dev mode trying to reach `localhost:1420`. Use `pnpm tauri build` /
`install.sh`. `install.sh` invokes the local Tauri CLI directly (pnpm would leak the
extra `--` into cargo).

## Conventions & invariants

- **Read-only by contract.** Never write Steam config files. The only output is a string
  the user copies. App state lives only under `$XDG_CONFIG_HOME/protongen/state.toml`. Five
  named exceptions, all explicit-confirm-only and documented in `design.md` §11:
  `heroic::inject` (writes Heroic's per-game config), `optiscaler_upgrade.rs` (fetches
  the latest OptiScaler release and extracts it into a game's own folder),
  `mangohud_export.rs` (merges the MangoHud builder's overlay into the real, system-wide
  `MangoHud.conf`), `vkbasalt_export.rs` (same shape, for the vkBasalt builder's
  effect chain into the real, system-wide `vkBasalt.conf`), and `nexus::set_launch`
  (runs the user's own `nexus-cli --set-launch` — no shell, slug from discovery).
- **Nexus launches, protongen tunes.** Nexus (`~/Documents/Projects/Fitgirl`, the user's
  launcher) owns launching games; never add a "launch" feature here. Integration is
  `games.json` (read), `--game`/the JSON CLI (Nexus → us), and Apply to Nexus (us → Nexus);
  see `design.md` §6.3. A Nexus game keeps the id Nexus passes (shortcut appid → Heroic
  hash → slug hash) and *absorbs* its Steam/Heroic mirrors.
- **Pure Rust core, thin IPC.** Command assembly is a pure, deterministic, unit-tested
  function (`builder.rs`); Tauri commands are a serialization bridge. App, tests, and the
  `--list` CLI all consume the same logic.
- **Data-driven catalog.** `params.toml` / `recipes.toml` are baked in via `include_str!`
  and overridable by a user copy in `$XDG_CONFIG_HOME`. Adding an env var or recipe is a
  TOML edit — no Rust change, including a new argument-less *wrapper program* (it becomes a
  `Wrapper::Plain`; set `order` to place it). Only a wrapper that takes arguments, like
  gamescope, needs its own `Wrapper` variant.
- **Release versioning.** Semver `X.Y.Z`, with the number signalling the scope: **patch
  (`Z`)** = parameter/catalog refresh only (`params.toml`/`recipes.toml`, no code), **minor
  (`Y`)** = features / UI / backend, **major (`X`)** = milestone. Cut with
  `scripts/bump-version.sh X.Y.Z` (then `cargo update -p protongen` to sync `Cargo.lock`),
  commit `Release vX.Y.Z` on `main`, tag `vX.Y.Z`, push — the tag fires
  `.github/workflows/release.yml`, which builds + publishes the release the in-app updater reads.
- **Keep the DTO mirror in sync — now enforced.** `src/lib/types.ts` interfaces mirror the
  serde structs one-to-one. Each IPC struct derives `ts_rs::TS` under `cfg(test)`; `cargo
  test` writes `src/lib/generated/`, and `src/lib/contract.ts` fails `pnpm check` on any
  added/removed/renamed field or enum value. After changing a struct: `cargo test`, then
  `pnpm check`, and commit the regenerated files (CI checks they're fresh). A new IPC struct
  needs the `cfg_attr(test, derive(ts_rs::TS), ts(export, …))` line and a `contract.ts` row.
- **One store, debounced recompute.** `state.svelte.ts` holds all selection; a root
  `$effect` serializes to a `Config` and (debounced ~60 ms) calls `recompute` (command +
  tokens + lint + sync diff in one IPC).
- **Shell quoting has one rule, in two languages.** `builder::sh_quote` leaves a value bare
  when it is shell-safe, else double-quotes it (escaping only `"` `\` `` ` ``) so `$VAR`
  still expands; `game_args` and gamescope args are emitted verbatim (they *are* shell).
  `parser.rs` reads the same rules back and flags unquoted `; & | < > ( )` before the
  target as unmodeled. `src/lib/shell.ts` is the TS twin for custom env — change both.
- **The paste loop.** `steam_user_config` re-reads only launch options + compat tools, so
  `app.refreshSteamConfig` can run on every window focus (throttled to one read per 2 s,
  with extra re-checks for 10 min after Copy / Open in Steam). The root effect is split so
  that re-read only recomputes the sync status — it must never persist `game_memory` or
  flash "Saved".
- **Deep links into Settings** go through `app.openSettings(section?)`, which expands and
  scrolls to that section. A link inside another dialog closes its host first, and a
  confirm step inside a builder dialog is an inline panel, not a nested `<Dialog>` —
  stacked bits-ui modals are the #63 click-dead pattern.
- **Browser-mock dev path.** `ipc.ts` detects `__TAURI_INTERNALS__`; outside Tauri it
  returns `mock.ts` data so the whole UI can be iterated with `pnpm dev`. Mock data has a
  reduced catalog — FSR/large catalogs and the native file dialog only exist in the real
  shell.
- **Relevance & opt-in capabilities.** `hardware.rs` **detects only**; the filter is
  `util.ts irrelevance()` and is frontend-only by design — there is deliberately no Rust
  mirror, because the opt-in capabilities live in the store. (A mirror existed, went three
  tags stale, and became dead code.) Capabilities that can't be auto-detected are **opt-in
  store flags** surfaced in Settings: `hdr`, and `fsr4`/`rdna3`/`rdna4` derived from the
  AMD generation selector. That selector is *seeded* by `hardware.rs`'s `gpu_gen_detected`
  (PCI id → `pci.ids` → `Navi <n>` codename) but never overridden by it: `effectiveGpuGen`
  falls back to detection only when the user has declared nothing, and **`ipc::lint` must
  apply the same fallback** or the notices and the visibility filter disagree about which
  generation is in force. To hide a parameter by default, tag it `needs = ["<cap>"]` and
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
- **A stuck `body { pointer-events: none }` has a second cause, unrelated to `{...props}`
  ordering above.** bits-ui restores that lock via a bare `requestAnimationFrame` unless
  `Dialog.Content`/`AlertDialog.Content` gets an explicit `restoreScrollDelay`, and a rAF
  never fires while the window isn't actively painting a frame (minimized, occluded,
  backgrounded) — the lock then never lifts, silently, and the app goes click-dead exactly
  like #63 but from a different mechanism. `Dialog.svelte`, `CommandPalette.svelte` and
  `SettingsDrawer.svelte` all pass `restoreScrollDelay` for this reason — `0` where the
  dialog has no close transition to protect, `250` in `SettingsDrawer` to clear its own
  200ms `fly` transition first (bits-ui's own doc comment on `restoreScrollDelay`: it must
  exceed the transition duration). Any new bits-ui `Dialog`/`AlertDialog` needs the same.
- **CSP and the `art:` protocol.** `tauri.conf.json` sets a restrictive CSP (all network
  goes through Rust); `style-src` is excluded from Tauri's nonce injection so Svelte's
  inline styles keep working. Art is served by an *asynchronous* custom `art:` protocol
  (`lib.rs`) that answers only keys `game_art` registered. Don't switch to Tauri's asset
  protocol: its handler is synchronous, so on Linux it reads files on the UI thread and the
  library grid stutters (shipped in 0.27.0, fixed after).
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
- **`--game` is single-instance.** `tauri-plugin-single-instance` must stay the first
  plugin; a second `protongen --game <id>` emits `open-game` to the running window.
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
