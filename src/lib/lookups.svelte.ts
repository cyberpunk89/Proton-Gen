import { ipc } from "./ipc";
import type { AntiCheat, GameScan, Tier } from "./types";

// Per-game facts fetched from third-party sites — ProtonDB's tier and
// AreWeAntiCheatYet's verdict — cached for the session. Split out of the app
// store: neither depends on the builder's state.

class LookupStore {
  /**
   * ProtonDB tiers. Switching between two games used to refetch both every
   * time, which is needless load on an unofficial third-party API of unknown
   * rate limits.
   *
   * Session-level here; the backend also keeps a 7-day disk cache
   * (protondb.rs), so a fresh launch doesn't re-ask either.
   */
  tierCache = $state<Record<string, Tier | null>>({});
  /** Appids with a fetch in flight, so the chip can show a spinner. */
  tierLoading = $state<Record<string, boolean>>({});
  #tierRequested = new Set<number>();

  /** Cached tier, `null` if the lookup failed, `undefined` if never requested. */
  tierFor(appId: number): Tier | null | undefined {
    return this.tierCache[String(appId)];
  }

  /** Fetch a tier at most once per session. Safe to call repeatedly. */
  requestTier(appId: number) {
    if (this.#tierRequested.has(appId)) return;
    this.#tierRequested.add(appId);
    this.tierLoading[String(appId)] = true;
    ipc
      .protondbFetch(appId)
      .then((t) => (this.tierCache[String(appId)] = t))
      .catch((e) => {
        console.error("protondbFetch failed", appId, e);
        this.tierCache[String(appId)] = null;
      })
      .finally(() => (this.tierLoading[String(appId)] = false));
  }

  /** Drop a failed lookup so the chip's Retry can try again. */
  retryTier(appId: number) {
    this.#tierRequested.delete(appId);
    delete this.tierCache[String(appId)];
    this.requestTier(appId);
  }

  /** AreWeAntiCheatYet entries: appid -> entry, null when it has none, absent
   *  while unknown. The list itself is cached by the backend. */
  anticheatCache = $state<Record<string, AntiCheat | null>>({});
  #anticheatRequested = new Set<number>();

  /** Look a game up, once per session, when the opt-in is on. */
  requestAnticheat(appId: number, enabled: boolean) {
    if (!enabled || this.#anticheatRequested.has(appId)) return;
    this.#anticheatRequested.add(appId);
    ipc
      .anticheatLookup(appId)
      .then((a) => (this.anticheatCache[String(appId)] = a))
      .catch((e) => {
        console.error("anticheatLookup failed", appId, e);
        // Forget it so the next visit retries — the list may be reachable then.
        this.#anticheatRequested.delete(appId);
      });
  }

  /** Folder scans: appid -> scan, null when the game has no install folder,
   *  absent while unknown. Backend-cached; `rescan` forces a fresh walk. */
  scanCache = $state<Record<string, GameScan | null>>({});
  #scanRequested = new Set<number>();

  scanFor(appId: number | null): GameScan | null | undefined {
    return appId == null ? undefined : this.scanCache[String(appId)];
  }

  requestScan(appId: number, fresh = false) {
    if (!fresh && this.#scanRequested.has(appId)) return;
    this.#scanRequested.add(appId);
    ipc
      .gameScan(appId, fresh)
      .then((s) => (this.scanCache[String(appId)] = s))
      .catch((e) => {
        console.error("gameScan failed", appId, e);
        this.#scanRequested.delete(appId);
      });
  }
}

export const lookups = new LookupStore();
