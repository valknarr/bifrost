<script lang="ts">
  /**
   * Procedural multi-galaxy background.
   *
   * The previous iteration (image-driven halftone) wasn't carrying the
   * EVE-Frontier feel: the user reference is a *rendered* universe —
   * three galaxies that merged together to form Frontier's playable
   * space — not a stylised photo. So this version generates that
   * universe procedurally.
   *
   * Approach
   * --------
   * Each galaxy is a logarithmic spiral with N arms. Stars are sampled
   *   - radially: r = pow(u, 2) * R   — concentrates mass near the
   *     core, matches the look of the in-game render.
   *   - angularly: θ = arm_offset + tightness * ln(r+1) + jitter
   *     — logarithmic spiral is the classic galaxy parametrization.
   * Colour by radius: white-yellow core → orange mid → deep red halo,
   * a palette that matches the in-game star-field render and our
   * existing brand orange.
   *
   * Three galaxies are placed with overlapping centers + different
   * radii / rotation speeds / arm rotations. The overlap gives the
   * "merged" feel from EVE Frontier lore.
   *
   * Performance shape
   * -----------------
   * Each galaxy is baked to its own offscreen canvas ONCE on resize.
   * Per frame we just do `drawImage` with a rotation transform —
   * three sub-millisecond blits per frame, irrespective of star
   * count. Pauses entirely on `document.hidden`.
   *
   * Future: ef-map.com publishes a real EVE Frontier system map.
   * Swapping the procedural sampler for actual system coordinates is
   * a follow-up — the renderer is structured so the stars[] array
   * could come from anywhere.
   */

  import { onMount } from "svelte";

  let canvas: HTMLCanvasElement;

  type RGB = { r: number; g: number; b: number };

  /** One star. Cartesian relative to its galaxy's centre. */
  type Star = {
    x: number;
    y: number;
    size: number; // px (1..3)
    color: RGB;
    alpha: number;
  };

  /** One galaxy.
   *
   *  Main motion: the disc spins on its own axis (`angle` / `rotSpeed`).
   *  That's what the user reads as "the galaxy is alive".
   *
   *  Secondary motion: an extremely slow radial drift toward the
   *  system barycenter — galaxies edge closer along the line they
   *  already sit on, with no angular orbital component. Floored at
   *  `INSPIRAL_FLOOR` × initial distance so they never actually
   *  merge during a session (or across many sessions).
   *
   *  `baked` is the pre-rendered offscreen we blit each frame after
   *  applying the disc rotation. `bakedDpr` is recorded so a
   *  window-move to a different-DPI monitor can detect the mismatch
   *  and re-bake at the new pixel density. */
  type Galaxy = {
    cx: number; // current position (drifts slowly toward barycenter)
    cy: number;
    radius: number; // px; outer halo extent
    angle: number; // disc spin angle
    rotSpeed: number; // disc spin: radians per second (×dt/1000)
    initialDistFromBarycenter: number; // pinned at build; sets drift floor
    baked: HTMLCanvasElement; // pre-rendered galaxy
    bakedDpr: number; // dpr at the time of bake
  };

  /** Sample a colour along the white-yellow → orange → deep-red ramp
   *  by normalised radius. Matches the in-game palette and our brand
   *  orange family without resorting to literal Fenris Creations
   *  assets. */
  function colourForRadius(tNorm: number): RGB {
    if (tNorm < 0.12) {
      return { r: 255, g: 240, b: 200 }; // hot core
    }
    if (tNorm < 0.45) {
      const k = (tNorm - 0.12) / 0.33;
      return {
        r: 255,
        g: Math.round(220 - k * 90),
        b: Math.round(160 - k * 100),
      };
    }
    if (tNorm < 0.75) {
      const k = (tNorm - 0.45) / 0.3;
      return {
        r: Math.round(255 - k * 35),
        g: Math.round(130 - k * 60),
        b: Math.round(60 - k * 30),
      };
    }
    // Far halo — deep dim red.
    const k = Math.min(1, (tNorm - 0.75) / 0.25);
    return {
      r: Math.round(220 - k * 60),
      g: Math.round(70 - k * 40),
      b: Math.round(30 - k * 20),
    };
  }

  /** Generate a galaxy's star list using a 2-arm logarithmic spiral. */
  function generateStars(radius: number, count: number, seed: number): Star[] {
    // A tiny seeded RNG so re-renders look the same. Mulberry32.
    let s = seed >>> 0;
    const rng = () => {
      s = (s + 0x6d2b79f5) >>> 0;
      let t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };

    const stars: Star[] = [];
    const arms = 2;
    const tightness = 4.5; // higher = more wound

    for (let i = 0; i < count; i++) {
      // Concentrate mass near the centre — pow(u, 2) gives ~70 % of
      // stars in the inner half of the radius.
      const u = rng();
      const r = Math.pow(u, 1.8) * radius;
      const tNorm = r / radius;

      // Spiral parametrization.
      const arm = i % arms;
      const armOffset = arm * ((Math.PI * 2) / arms);
      const spiral = tightness * Math.log(1 + r / 80);
      // Perpendicular jitter falls off near the centre + halo so the
      // arms read as distinct in the mid-galaxy and dissolve in the
      // bulge / outer dust.
      const jitterScale = 0.35 * (1 - Math.abs(tNorm - 0.5) * 1.4);
      const jitter = (rng() - 0.5) * jitterScale * (Math.PI * 2);
      const theta = armOffset + spiral + jitter;

      const x = Math.cos(theta) * r;
      const y = Math.sin(theta) * r;

      // Size + alpha decline gently with radius so the core glows
      // brighter and the halo is a fine dust.
      const size = tNorm < 0.2 ? 1.5 + rng() * 1.2 : 0.8 + rng() * 1.0;
      const alpha = tNorm < 0.2 ? 0.7 + rng() * 0.3 : 0.35 + rng() * 0.4;

      stars.push({ x, y, size, color: colourForRadius(tNorm), alpha });
    }
    return stars;
  }

  /** Paint a galaxy to a fresh offscreen canvas of size 2R × 2R.
   *  Star coords are relative to the galaxy centre, so we translate
   *  by R to land them inside the canvas bounds. */
  function bakeGalaxy(
    radius: number,
    stars: Star[],
    dpr: number,
  ): HTMLCanvasElement {
    const off = document.createElement("canvas");
    const side = radius * 2;
    off.width = Math.round(side * dpr);
    off.height = Math.round(side * dpr);
    const oc = off.getContext("2d");
    if (!oc) return off;
    oc.setTransform(dpr, 0, 0, dpr, 0, 0);

    // Core glow — large radial gradient. Painted first so stars sit
    // on top of the warm bloom rather than under it.
    const grad = oc.createRadialGradient(
      radius,
      radius,
      0,
      radius,
      radius,
      radius * 0.85,
    );
    grad.addColorStop(0, "rgba(255, 220, 150, 0.32)");
    grad.addColorStop(0.18, "rgba(255, 140, 60, 0.16)");
    grad.addColorStop(0.5, "rgba(180, 50, 30, 0.05)");
    grad.addColorStop(1, "rgba(0, 0, 0, 0)");
    oc.fillStyle = grad;
    oc.fillRect(0, 0, side, side);

    // Stars.
    for (let i = 0; i < stars.length; i++) {
      const s = stars[i];
      oc.fillStyle = `rgba(${s.color.r},${s.color.g},${s.color.b},${s.alpha.toFixed(3)})`;
      const half = s.size * 0.5;
      oc.fillRect(radius + s.x - half, radius + s.y - half, s.size, s.size);
    }

    return off;
  }

  onMount(() => {
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) {
      console.warn("[BackgroundField] 2D context unavailable, skipping");
      return;
    }
    const g = ctx;
    const c = canvas;

    let vw = 0;
    let vh = 0;
    let dpr = 1;
    let galaxies: Galaxy[] = [];
    /** System barycenter — galaxies orbit around this point and slowly
     *  inspiral toward it. Recomputed on resize from the spec anchors. */
    let barycenter = { x: 0, y: 0 };
    let rafId = 0;
    let paused = document.hidden;
    let lastT = performance.now();

    /** Multiplicative shrink per ms applied to each galaxy's distance
     *  from the system barycenter. 2e-8 ≈ −0.12 %/minute, or about
     *  7 % closer per hour. Slow enough that within a single play
     *  session the motion is barely perceptible; over many sessions
     *  the field perpetually feels like it's "about to collide". */
    const INFALL_PER_MS = 2e-8;
    /** Floor: galaxies can shrink to this fraction of their initial
     *  distance and no further. Stops them merging across many runs. */
    const INSPIRAL_FLOOR = 0.5;
    /** Reference dimension galaxies are sized against, in CSS px. Fixed
     *  rather than viewport-relative — the user reads the field as a
     *  real backdrop (a window onto the universe), so resizing the
     *  Bifrost window shouldn't rescale the galaxies. A wider window
     *  reveals more empty space around them; a narrower one crops the
     *  outer halos. 1100 fits comfortably on most laptop screens and
     *  leaves a generous halo on larger monitors. */
    const REF_DIM = 1100;

    function buildGalaxies(): void {
      // Three galaxies — sized in absolute pixels (REF_DIM) rather
      // than as fractions of the viewport. Centres are anchored to
      // the viewport centre but offset by fixed amounts, so resizing
      // the Bifrost window doesn't rescale the galaxies — it just
      // changes how much black space sits around them. Rotation
      // speeds differ slightly so the merged field never resolves to
      // a single static pattern.
      const cx = vw * 0.5;
      const cy = vh * 0.5;

      const specs: {
        cx: number;
        cy: number;
        radius: number;
        count: number;
        seed: number;
        rotSpeed: number;
      }[] = [
        {
          cx: cx - REF_DIM * 0.18,
          cy: cy - REF_DIM * 0.05,
          radius: REF_DIM * 0.46,
          count: 1800,
          seed: 0x10c9_3a17,
          rotSpeed: 0.00018,
        },
        {
          cx: cx + REF_DIM * 0.14,
          cy: cy + REF_DIM * 0.04,
          radius: REF_DIM * 0.38,
          count: 1400,
          seed: 0x44ab_77c3,
          rotSpeed: -0.00012,
        },
        {
          cx: cx + REF_DIM * 0.02,
          cy: cy + REF_DIM * 0.22,
          radius: REF_DIM * 0.32,
          count: 1100,
          seed: 0x9bd1_205f,
          rotSpeed: 0.00009,
        },
      ];

      // Barycenter = unweighted mean of the spec anchors. Galaxies
      // drift along the line connecting their position to this point.
      barycenter = {
        x: specs.reduce((a, s) => a + s.cx, 0) / specs.length,
        y: specs.reduce((a, s) => a + s.cy, 0) / specs.length,
      };

      // Preserve as much state as possible across rebuilds. Galaxy
      // radii / star layout are now fixed (no longer scale with the
      // viewport), so a resize doesn't actually need to re-bake the
      // offscreen canvases — it only needs to re-anchor positions to
      // the new viewport centre. We:
      //
      //   - Reuse `baked` when its radius + DPR match (the common
      //     case — only the window size changed, not the screen).
      //   - Preserve `angle` so the disc rotation continues smoothly
      //     instead of jumping (this was the main visible bug).
      //
      // The seed array drives star layout, so index-based preservation
      // is safe — galaxy 0 is always the same logical galaxy across
      // rebuilds.
      const prior = galaxies;
      galaxies = specs.map((spec, i) => {
        const dx = spec.cx - barycenter.x;
        const dy = spec.cy - barycenter.y;
        const priorGal = prior[i];
        const canReuseBaked =
          priorGal !== undefined &&
          priorGal.radius === spec.radius &&
          priorGal.bakedDpr === dpr;
        const baked = canReuseBaked
          ? priorGal.baked
          : bakeGalaxy(
              spec.radius,
              generateStars(spec.radius, spec.count, spec.seed),
              dpr,
            );
        return {
          cx: spec.cx,
          cy: spec.cy,
          radius: spec.radius,
          angle: priorGal?.angle ?? Math.random() * Math.PI * 2,
          rotSpeed: spec.rotSpeed,
          initialDistFromBarycenter: Math.hypot(dx, dy),
          baked,
          bakedDpr: dpr,
        };
      });
    }

    /** Cheap part of resize: update the canvas backing store + DPR
     *  transform. Runs on every resize event so the canvas stays
     *  aligned with the window during a drag. */
    function resizeCanvas(): void {
      vw = window.innerWidth;
      vh = window.innerHeight;
      dpr = window.devicePixelRatio || 1;

      c.width = Math.round(vw * dpr);
      c.height = Math.round(vh * dpr);
      c.style.width = `${vw}px`;
      c.style.height = `${vh}px`;
      g.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    /** Expensive part: regenerate stars + bake each galaxy's
     *  offscreen canvas. Debounced from the live resize event so we
     *  don't bake ~5 000 stars × 3 galaxies on every frame of a
     *  window-edge drag. */
    let rebuildTimer: ReturnType<typeof setTimeout> | null = null;
    function scheduleRebuild(): void {
      if (rebuildTimer !== null) clearTimeout(rebuildTimer);
      rebuildTimer = setTimeout(() => {
        rebuildTimer = null;
        buildGalaxies();
        // Reset the dt accumulator so the rebuild work doesn't show
        // up as a one-shot animation jump on the next frame.
        lastT = performance.now();
      }, 150);
    }

    function onResize(): void {
      resizeCanvas();
      scheduleRebuild();
    }

    function frame(now: number): void {
      // Cap dt at 50 ms (~3 frames at 60 fps). If the main thread
      // was blocked — by a resize storm, a long task, an OS hiccup —
      // we don't want the next frame to apply that whole gap to the
      // motion integrators in one go and produce a visible jump.
      const dt = Math.min(50, Math.max(0, now - lastT));
      lastT = now;

      if (paused) {
        rafId = requestAnimationFrame(frame);
        return;
      }

      g.clearRect(0, 0, vw, vh);

      // Per galaxy: main motion is the disc spinning on its own axis.
      // Secondary motion is a very slow radial drift toward the
      // barycenter along the line each galaxy already sits on (no
      // angular component — galaxies don't orbit each other). The
      // user reads "spinning galaxies, almost imperceptibly closing
      // over time". composite=lighter so overlapping galaxies brighten
      // where they intersect — luminous gas merging.
      g.globalCompositeOperation = "lighter";
      for (const gal of galaxies) {
        // Disc self-spin — the main motion. `* 0.06` is the legacy
        // 60-fps tuning factor.
        gal.angle += gal.rotSpeed * dt * 0.06;

        // Radial inspiral. Vector from barycenter to galaxy, shrink
        // its length toward the floor multiplicatively. Angular
        // component is preserved (we only rescale; no rotation),
        // so the galaxy stays on the same ray from the barycenter.
        const vx = gal.cx - barycenter.x;
        const vy = gal.cy - barycenter.y;
        const currentDist = Math.hypot(vx, vy);
        if (currentDist > 0.001) {
          const minDist = gal.initialDistFromBarycenter * INSPIRAL_FLOOR;
          const newDist = Math.max(
            minDist,
            currentDist * (1 - INFALL_PER_MS * dt),
          );
          const scale = newDist / currentDist;
          gal.cx = barycenter.x + vx * scale;
          gal.cy = barycenter.y + vy * scale;
        }

        g.save();
        g.translate(gal.cx, gal.cy);
        g.rotate(gal.angle);
        g.drawImage(
          gal.baked,
          -gal.radius,
          -gal.radius,
          gal.radius * 2,
          gal.radius * 2,
        );
        g.restore();
      }
      g.globalCompositeOperation = "source-over";

      rafId = requestAnimationFrame(frame);
    }

    function onVisibility(): void {
      paused = document.hidden;
      lastT = performance.now(); // avoid a big dt spike on resume
    }

    // Initial setup: size the canvas + build galaxies once
    // synchronously, then wire the (debounced) resize handler for
    // subsequent window changes.
    resizeCanvas();
    buildGalaxies();
    rafId = requestAnimationFrame(frame);
    window.addEventListener("resize", onResize);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      cancelAnimationFrame(rafId);
      if (rebuildTimer !== null) clearTimeout(rebuildTimer);
      window.removeEventListener("resize", onResize);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  });
</script>

<!-- z-index: -10 keeps the canvas between html's radial gradient and
     Layout's normal-flow content. body + #app are transparent (see
     app.css) so the canvas isn't covered by their backgrounds. -->
<canvas
  bind:this={canvas}
  class="pointer-events-none fixed inset-0 -z-10"
  aria-hidden="true"
></canvas>
