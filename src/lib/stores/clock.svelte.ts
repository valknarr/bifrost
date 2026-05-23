// Shared "current time" tick. Cards that surface relative-time labels
// (e.g. PilotCard's "Updated 5m ago" balance staleness) need a value
// to re-derive against. Centralising the timer here means N pilot
// cards share ONE interval instead of N — at 4 pilots the difference
// is negligible, but the right shape from the start avoids "why are
// there 47 setInterval timers running" surprises later.
//
// Granularity: 15 s. Balance staleness only crosses meaningful
// thresholds at ~60 s / ~5 min / ~30 min, so re-rendering every 15 s
// is more than enough. Cheaper than 1 s, less laggy than 60 s.
//
// The single ticker stays attached for the lifetime of the page;
// browser teardown cleans it up.

import { onMount } from "svelte";

class ClockStore {
  /** Unix milliseconds. Re-assigned every TICK_MS so anything
   *  reading this in a `$derived` re-runs. */
  now = $state<number>(Date.now());

  private interval: ReturnType<typeof setInterval> | null = null;
  private refCount = 0;

  /** Increment the subscriber count and start the ticker on first
   *  subscriber. Components SHOULD call `subscribe()` inside their
   *  `onMount` and the returned unsubscribe inside the mount
   *  cleanup. The refcount means the ticker stops when nothing's
   *  watching (e.g. user is on SettingsView and there are no pilot
   *  cards visible). */
  subscribe(): () => void {
    this.refCount++;
    if (this.refCount === 1) {
      this.now = Date.now();
      this.interval = setInterval(() => {
        this.now = Date.now();
      }, 15_000);
    }
    return () => {
      this.refCount--;
      if (this.refCount === 0 && this.interval !== null) {
        clearInterval(this.interval);
        this.interval = null;
      }
    };
  }
}

export const clockStore = new ClockStore();

/** Convenience helper for components: subscribes on mount, unsubscribes
 *  on destroy. Use inside `onMount(() => useClockTick())`. */
export function useClockTick(): void {
  onMount(() => clockStore.subscribe());
}
