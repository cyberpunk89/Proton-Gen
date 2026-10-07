import { ipc } from "./ipc";

// Game art, loaded lazily and cached for the session. Split out of the app
// store: nothing here depends on the builder's state, and every tile reads it.

export type ArtKind = "portrait" | "hero" | "header";

/** Simultaneous art lookups. Bounded because the grid requests art for every
 *  tile that scrolls near the viewport instead of a fixed first-N. */
const ART_CONCURRENCY = 12;

class ArtStore {
  // key `${source}:${appId}:${kind}` →
  //   undefined = not requested/loading · null = none found · string = art: URL
  cache = $state<Record<string, string | null>>({});
  /** Bumped when `retryFailed` forgets requests, so visible tiles — whose
   *  `inView` has already fired — re-ask for their art. */
  epoch = $state(0);

  #requested = new Set<string>();
  /** Fetches awaiting IPC, and the backlog behind them. Each is a Tauri
   *  round-trip (plus a disk or CDN read), so scrolling fast through a few
   *  thousand tiles would otherwise fan out into thousands of requests. */
  #inFlight = 0;
  #queue: Array<{ key: string; run: () => void }> = [];

  #key(appId: number, source: string, kind: ArtKind): string {
    return `${source}:${appId}:${kind}`;
  }

  /** Cached art (`art:` URL), `null` if none found, `undefined` if not loaded. */
  for(appId: number, source: string, kind: ArtKind): string | null | undefined {
    return this.cache[this.#key(appId, source, kind)];
  }

  /**
   * Lazily fetch a game's art once; the result lands in `cache` reactively.
   * A Heroic sideload's `art_url` hint is looked up by the backend itself.
   */
  request(appId: number, source: string, kind: ArtKind) {
    const key = this.#key(appId, source, kind);
    if (this.#requested.has(key)) return;
    this.#requested.add(key);

    // "Local + online fallback" per the user's choice: allow the CDN backstop.
    const run = () => {
      this.#inFlight++;
      ipc
        .gameArt(appId, source, kind, true)
        .then((url) => (this.cache[key] = url))
        .catch(() => (this.cache[key] = null))
        .finally(() => {
          this.#inFlight--;
          this.#queue.shift()?.run();
        });
    };

    if (this.#inFlight < ART_CONCURRENCY) run();
    else this.#queue.push({ key, run });
  }

  /**
   * Retry art that previously came back empty.
   *
   * `#requested` is a permanent "don't ask twice" set, which is right for the
   * steady state but means a lookup that failed once stays blank for the rest
   * of the session. A library refresh is the natural moment to try again — but
   * only for the failures: successes stay cached so a refresh doesn't re-fetch
   * every tile the user can already see.
   */
  retryFailed() {
    for (const [key, value] of Object.entries(this.cache)) {
      if (value === null) {
        delete this.cache[key];
        this.#requested.delete(key);
      }
    }
    // Anything still queued was scheduled against the pre-refresh library.
    // Forget those too, or `request` would treat them as already asked for
    // and the tile would stay blank for the rest of the session.
    for (const { key } of this.#queue) this.#requested.delete(key);
    this.#queue.length = 0;
    this.epoch++;
  }
}

export const art = new ArtStore();
