/* quality.js - graphics tiers for the page (docs/quality_tiers.md).
 *
 * The tier definitions come from the wasm session (`qualityJson`, Rust `quality.rs`), so
 * nothing about a tier is written here. This file owns the player's setting (auto or one of
 * safe / mid / high / max), the first-run probe, the runtime guard and the Graphics row on the
 * overlay card. It makes no network request; its one storage key is `kagra.quality`, read and
 * written inside try/catch (a private tab or a sandboxed iframe has none).
 *
 * Genre-neutral: a game takes part when its kit says so (`qualityJson` is `{}` otherwise).
 * Plain script (no module) so the same file runs in the page and under node (tests).
 */
(function (root, factory) {
  const api = factory(root);
  if (typeof module === "object" && module.exports) module.exports = api;
  if (root) root.KagraQuality = api;
})(typeof self !== "undefined" ? self : globalThis, function () {
  "use strict";

  const TIERS = ["safe", "mid", "high", "max"];
  const STORE_KEY = "kagra.quality";
  const WEEK_MS = 7 * 24 * 3600 * 1000;
  const CALM = ["title", "pause", "result"];

  const idx = (name) => TIERS.indexOf(name);
  const r1 = (v) => (Number.isFinite(v) ? Math.round(v * 10) / 10 : null);

  /** The `?quality=` word as a setting: "auto" or a tier name. The older picture words map
   *  low -> safe, mobile -> mid. Anything else: null (the page ignores it). */
  function aliasQuality(word) {
    const w = String(word || "").trim().toLowerCase();
    if (w === "auto") return "auto";
    if (TIERS.includes(w)) return w;
    if (w === "low") return "safe";
    if (w === "mobile") return "mid";
    return null;
  }

  function percentile(sorted, q) {
    const n = sorted.length;
    if (!n) return NaN;
    const pos = Math.min(1, Math.max(0, q)) * (n - 1);
    const lo = Math.floor(pos);
    const hi = Math.min(n - 1, lo + 1);
    return sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo);
  }

  /** p95, p99 and the share of slow frames (over 1.5 x the 60 Hz interval) of rAF gaps. */
  function windowStats(dts) {
    const s = Float64Array.from(dts).sort();
    let slow = 0;
    for (let i = 0; i < dts.length; i += 1) if (dts[i] > 25) slow += 1;
    return {
      n: dts.length,
      p95: percentile(s, 0.95),
      p99: percentile(s, 0.99),
      slowPct: dts.length ? (100 * slow) / dts.length : 0,
    };
  }

  const isGood = (st, auto) => st.n >= 30 && st.p95 <= auto.good_p95_ms && st.p99 <= auto.good_p99_ms;

  // ------------------------------------------------------------------ setting --
  /** The saved setting: { mode, tier, ts } or null. */
  function loadSetting(storage) {
    try {
      const raw = storage && storage.getItem(STORE_KEY);
      if (!raw) return null;
      const v = JSON.parse(raw);
      const mode = v && (v.mode === "auto" || TIERS.includes(v.mode)) ? v.mode : null;
      if (!mode) return null;
      return { mode, tier: TIERS.includes(v.tier) ? v.tier : null, ts: Number(v.ts) || 0 };
    } catch (_) { return null; }
  }
  function saveSetting(storage, s) {
    try { if (storage) storage.setItem(STORE_KEY, JSON.stringify({ mode: s.mode, tier: s.tier || null, ts: s.ts || 0 })); } catch (_) { /* no storage */ }
  }

  // -------------------------------------------------------------------- probe --
  /** The first-run probe as a pure state machine. Feed it rAF gaps (ms); it answers
   *  { try: tier } when the page should switch and keep feeding, or { done: tier }.
   *  It starts at `auto.start`, climbs while a window is good up to `auto.ceiling`, stops at
   *  the first failure, and falls toward `safe` when even the start tier is bad. */
  function createProbe(auto) {
    const ceil = idx(auto.ceiling);
    let i = idx(auto.start);
    let lastGood = -1;
    let dirUp = true;
    let elapsed = 0; // ms since the window began
    let dts = [];
    const reset = () => { elapsed = 0; dts = []; };
    return {
      get tier() { return TIERS[i]; },
      feed(dt) {
        if (!(dt > 0)) return null;
        if (dt > 1000) { reset(); return null; } // a hidden tab or a stop: start the window again
        elapsed += dt;
        if (elapsed < auto.settle_secs * 1000) return null;
        dts.push(dt);
        if (elapsed < (auto.settle_secs + auto.probe_secs) * 1000) return null;
        const good = isGood(windowStats(dts), auto);
        reset();
        if (good) {
          lastGood = i;
          if (dirUp && i < ceil) { i += 1; return { try: TIERS[i] }; }
          return { done: TIERS[lastGood] };
        }
        if (lastGood >= 0) return { done: TIERS[lastGood] };
        if (i > 0) { dirUp = false; i -= 1; return { try: TIERS[i] }; }
        return { done: TIERS[0] };
      },
    };
  }

  // -------------------------------------------------------------------- guard --
  /** The runtime guard as a pure state machine. `feed(dt, tMs, calm)` returns "down", "up" or
   *  null; the caller applies it and calls `moved(tier, tMs)`. Down: this many slow frames in
   *  the last `down_secs`. Up: only at a calm moment, after `up_secs` without a change and a
   *  good window over that time, and never into a tier that was stepped down from in the last
   *  `ban_secs`. */
  function createGuard(auto, tier) {
    const ceil = idx(auto.ceiling);
    let cur = idx(tier);
    let lastChange = -1e12;
    let settleUntil = 0;
    let samples = []; // { t, dt }
    let lastCheck = 0;
    const bans = {};
    const keep = Math.max(auto.up_secs, auto.down_secs) * 1000 + 2000;
    return {
      get tier() { return TIERS[cur]; },
      moved(name, t) {
        cur = idx(name);
        lastChange = t;
        settleUntil = t + 1500;
        samples = [];
      },
      feed(dt, t, calm) {
        if (!(dt > 0)) return null;
        if (dt > 1000) { samples = []; return null; }
        if (t < settleUntil) return null;
        samples.push({ t, dt });
        while (samples.length && t - samples[0].t > keep) samples.shift();
        if (t - lastCheck < 500) return null;
        lastCheck = t;
        if (t - lastChange < auto.cooldown_secs * 1000) return null;
        // down
        const dsec = auto.down_secs * 1000;
        const recent = samples.filter((s) => t - s.t <= dsec);
        if (cur > 0 && recent.length >= 0.7 * (dsec / 16.7)) {
          let slow = 0;
          for (const s of recent) if (s.dt > 25) slow += 1;
          if ((100 * slow) / recent.length >= auto.down_drop_pct) {
            bans[cur] = t + auto.ban_secs * 1000;
            return "down";
          }
        }
        // up
        const usec = auto.up_secs * 1000;
        if (calm && cur < ceil && t - lastChange >= usec && !(bans[cur + 1] > t)) {
          const win = samples.filter((s) => t - s.t <= usec);
          if (win.length && t - win[0].t >= usec * 0.9 && isGood(windowStats(win.map((s) => s.dt)), auto)) return "up";
        }
        return null;
      },
      banned(name, t) { return bans[idx(name)] > t; },
    };
  }

  // --------------------------------------------------------------------- page --
  const FALLBACK_TEXT = {
    "gfx.label": "Graphics",
    "gfx.auto": "Auto",
    "gfx.auto.now": "Auto ({tier})",
    "gfx.safe": "Safe",
    "gfx.mid": "Mid",
    "gfx.high": "High",
    "gfx.max": "Max",
    "gfx.hint": "Auto picks for your phone and steps down if it stutters. Max uses more battery.",
  };

  /** Start on a page. `ctx`:
   *    session           the wasm session (qualityJson / setQualityTier / setQualityCustom)
   *    params            URLSearchParams of the page
   *    setScaleCap(cap)  the render-scale cap the tier asks for (the page clamps to the display)
   *    getPhase()        "title" | "play" | "pause" | "result"
   *    storage, now(), raf(fn), t(key, vars)   (optional, for tests)
   *  Returns the control object (also on window.kagraQuality), or { active: false }. */
  function start(ctx) {
    const win = root;
    const doc = win && win.document;
    const session = ctx.session;
    let spec = null;
    try { spec = session.qualityJson ? JSON.parse(session.qualityJson()) : null; } catch (_) { spec = null; }
    if (!spec || !spec.tiers || !spec.auto) return { active: false };
    if (win && win.kagraQuality && win.kagraQuality.active) return win.kagraQuality;

    const storage = ctx.storage !== undefined ? ctx.storage : (() => { try { return win.localStorage; } catch (_) { return null; } })();
    const now = ctx.now || (() => win.performance.now());
    const wall = ctx.wall || (() => Date.now());
    const raf = ctx.raf || ((fn) => win.requestAnimationFrame(fn));
    const tr = (key, vars) => {
      const f = ctx.t || (win && win.kagraT);
      const s = f ? f(key, vars) : key;
      if (s !== key) return s;
      let fb = FALLBACK_TEXT[key] || key;
      if (vars) fb = fb.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
      return fb;
    };
    const auto = spec.auto;
    const phase = () => (ctx.getPhase ? ctx.getPhase() : "play");

    // ---- the setting: ?quality= wins for this visit and is not saved
    const asked = ctx.params ? aliasQuality(ctx.params.get("quality")) : null;
    const saved = loadSetting(storage);
    let mode = asked || (saved && saved.mode) || "auto";
    let tier = null; // what is applied now
    let autoTier = null; // what auto chose
    let probe = null;
    let guard = null;
    let hold = 0;
    let probeTier = null;
    let changes = [];
    const log = (why, to) => {
      changes.push({ at: r1(now() / 1000), why, tier: to });
      if (changes.length > 40) changes.shift();
    };
    const scaleOf = (name) => (spec.tiers[name] ? spec.tiers[name].scale : 1);

    function apply(name, why) {
      if (!session.setQualityTier(name)) return false;
      tier = name;
      if (ctx.setScaleCap) ctx.setScaleCap(scaleOf(name), name);
      if (guard) guard.moved(name, now());
      log(why || "set", name);
      paintRow();
      return true;
    }
    function persist() {
      if (asked && !userChose) return; // a link's ?quality= does not overwrite the player's own choice
      saveSetting(storage, { mode, tier: autoTier, ts: wall() });
    }
    let userChose = false;

    // ---- the Graphics row
    let btn = null;
    function label() {
      const m = mode === "auto" ? tr("gfx.auto.now", { tier: tr(`gfx.${tier || auto.start}`) }) : tr(`gfx.${mode}`);
      return `${tr("gfx.label")}: ${m}`;
    }
    function paintRow() { if (btn) btn.textContent = label(); }
    function mountRow() {
      if (!doc || ctx.row === false) return;
      const anchor = doc.getElementById("sound") || doc.getElementById("keys");
      if (!anchor) return;
      const p = doc.createElement("p");
      p.id = "gfxRow";
      btn = doc.createElement("button");
      btn.id = "gfx";
      btn.type = "button";
      btn.title = tr("gfx.hint");
      btn.addEventListener("pointerdown", (e) => e.stopPropagation());
      btn.addEventListener("click", (e) => { e.preventDefault(); e.stopPropagation(); cycle(); });
      p.appendChild(btn);
      const host = anchor.closest ? anchor.closest("p") || anchor : anchor;
      host.parentNode.insertBefore(p, host);
      paintRow();
    }

    function setMode(next, byPlayer) {
      if (next !== "auto" && !TIERS.includes(next)) return false;
      mode = next;
      if (byPlayer) userChose = true;
      probe = null;
      guard = null;
      if (mode === "auto") {
        const fresh = saved && saved.mode === "auto" && saved.tier && wall() - saved.ts < WEEK_MS && !byPlayer;
        const base = autoTier || (fresh ? saved.tier : null);
        if (base) {
          autoTier = clampAuto(base);
          apply(autoTier, "auto");
          guard = createGuard(auto, autoTier);
          guard.moved(autoTier, now());
        } else {
          apply(auto.start, "probe");
          probeTier = auto.start;
          probe = createProbe(auto);
        }
      } else {
        apply(mode, "manual");
      }
      persist();
      paintRow();
      return true;
    }
    const clampAuto = (name) => (idx(name) > idx(auto.ceiling) ? auto.ceiling : name);
    function cycle() {
      const order = ["auto", ...TIERS];
      const next = order[(order.indexOf(mode) + 1) % order.length];
      // Going back to auto keeps what auto had learned; a new visit re-probes after a week.
      setMode(next, true);
    }

    // ---- the frame loop for probe and guard
    let lastT = 0;
    function onFrame(t) {
      raf(onFrame);
      const dt = lastT ? t - lastT : 0;
      lastT = t;
      if (!(dt > 0) || hold > 0 || (doc && doc.hidden)) {
        if (doc && doc.hidden) lastT = 0;
        return;
      }
      if (probe) {
        const r = probe.feed(dt);
        if (r && r.try) { probeTier = r.try; apply(r.try, "probe"); }
        else if (r && r.done) {
          autoTier = clampAuto(r.done);
          probe = null;
          apply(autoTier, "probe-done");
          guard = createGuard(auto, autoTier);
          guard.moved(autoTier, now());
          persist();
        }
      } else if (guard && mode === "auto") {
        const a = guard.feed(dt, t, CALM.includes(phase()));
        if (a === "down" && idx(tier) > 0) {
          autoTier = TIERS[idx(tier) - 1];
          apply(autoTier, "guard-down");
          persist();
        } else if (a === "up" && idx(tier) < idx(auto.ceiling)) {
          autoTier = TIERS[idx(tier) + 1];
          apply(autoTier, "guard-up");
          persist();
        }
      }
    }

    // ---- for the recorder / Find my limit: one-off steps over the current tier
    function custom(over) {
      session.setQualityCustom(JSON.stringify(over));
      tier = "custom";
      const sc = over.scale != null ? over.scale : scaleOf(autoOrMode());
      if (ctx.setScaleCap) ctx.setScaleCap(sc, "custom");
      return sc;
    }
    const autoOrMode = () => (mode === "auto" ? (autoTier || auto.start) : mode);
    function restore() {
      apply(probe && probeTier ? probeTier : autoOrMode(), "restore");
    }
    /** Show a named tier for a moment without touching the saved setting (Find my limit's first step). */
    function show(name) {
      if (!TIERS.includes(name)) return false;
      return apply(name, "show");
    }

    setMode(mode, false);
    mountRow();
    raf(onFrame);
    const api = {
      active: true,
      spec,
      get mode() { return mode; },
      get tier() { return tier; },
      get probing() { return Boolean(probe); },
      setMode: (m) => setMode(m, true),
      cycle,
      custom,
      show,
      restore,
      /** Pause probe and guard (the recorder holds while it measures). */
      hold(on) { hold = Math.max(0, hold + (on ? 1 : -1)); },
      status() { return { mode, tier, autoTier, probing: Boolean(probe), changes: changes.slice(-12) }; },
    };
    if (win) win.kagraQuality = api;
    return api;
  }

  return { TIERS, aliasQuality, windowStats, isGood, createProbe, createGuard, loadSetting, saveSetting, start, STORE_KEY };
});
