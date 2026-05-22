// Tiny route store so views can navigate without prop-drilling.
//
// We use plain string routes ("pilots" / "settings") — nothing here
// warrants a full router. Layout binds its nav buttons to `current`,
// App reads `current` to decide which view to render, and any view
// can call `go(...)` to jump (e.g. the missing-dependencies banner on
// the Pilots view jumps to Settings).

export type Route = "pilots" | "settings";

class RouteStore {
  current = $state<Route>("pilots");

  go(to: Route) {
    this.current = to;
  }
}

export const routeStore = new RouteStore();
