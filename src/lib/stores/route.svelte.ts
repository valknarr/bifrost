// Tiny route store so views can navigate without prop-drilling.
//
// We use plain string routes ("riders" / "settings") — nothing here
// warrants a full router. Layout binds its nav buttons to `current`,
// App reads `current` to decide which view to render, and any view
// can call `go(...)` to jump (e.g. the missing-dependencies banner on
// the Riders view jumps to Settings).

export type Route = "riders" | "settings";

class RouteStore {
  current = $state<Route>("riders");

  go(to: Route) {
    this.current = to;
  }
}

export const routeStore = new RouteStore();
