import { ipc } from "./ipc";
import type { AntiCheat, Tier } from "./types";

// Per-game facts fetched from third-party sites — ProtonDB's tier and
// AreWeAntiCheatYet's verdict — cached for the session. Split out of the app
// store: neither depends on the builder's state.

class LookupStore {
  /**
   * ProtonDB tiers. Switching between two games used to refetch both every
   * time, which is needless load on an unofficial third-party API of unknown
   * rate limits.
   *
   * Session-only on purpose. A persisted TTL cache needs a `Store` field, a
   * clock, an eviction policy and `Tier: Deserialize` — worth measuring for
   * first, and it would have to respect the wholesale-overwrite hazard in #43.
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
}

export const lookups = new LookupStore();
