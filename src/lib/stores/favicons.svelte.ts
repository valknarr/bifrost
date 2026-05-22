// Favicon store. Backed by the Rust-side `get_favicon` command which
// owns the HTTP fetch + on-disk cache + negative-cache sentinel.
// This store is a thin frontend cache:
//
//   - One IPC call per URL per process lifetime (subsequent calls
//     within the Rust cache TTL serve from disk and skip the network).
//   - A `SvelteMap` (from `svelte/reactivity`) gives us per-key
//     fine-grained reactivity that the plain $state<Record<…>> proxy
//     would drop for dynamic string keys.
//   - The in-flight `pending` set dedupes concurrent calls so a
//     rapid render storm doesn't fan out N parallel IPCs for the
//     same URL.

import { SvelteMap } from "svelte/reactivity";

import { api } from "../tauri";

class FaviconStore {
  /** Resolved favicon data URLs. A `null` value means "we tried and
   *  Rust said no favicon" — consumers fall back to the user-defined
   *  letter glyph and don't keep retrying. */
  cache = new SvelteMap<string, string | null>();

  private pending = new Set<string>();

  /** Idempotent fetch + cache. No-op if `url` is already in the
   *  cache or already in flight. Errors are stored as `null` so the
   *  consumer renders its fallback and we don't retry within the
   *  session — manual retry would clear the entry first. */
  async load(url: string): Promise<void> {
    if (this.cache.has(url) || this.pending.has(url)) return;
    this.pending.add(url);
    try {
      const result = await api.getFavicon(url);
      this.cache.set(url, result);
    } catch {
      this.cache.set(url, null);
    } finally {
      this.pending.delete(url);
    }
  }
}

export const faviconStore = new FaviconStore();
