/* KAGRA portal host shim.
 *
 * A classic script, loaded by play.html next to i18n.js. It sits between the
 * page and a web-game portal's SDK:
 *
 *   page  --events-->  host (this file)  --calls-->  provider (SDK adapter)
 *
 * The page never talks to an SDK. The host owns the rules every portal asks
 * for: gameplay start/stop without duplicates, ads only at the breaks the
 * game spec allows (`platform.ads`), the game muted and blocked while an ad
 * is up, and a game that still runs when the SDK is missing or blocked.
 *
 * Providers: `none` (default, no-op), `crazygames` (SDK v3), `gamemonetize`
 * (SDK_OPTIONS + showBanner), and `createStubProvider(name)` for any other
 * portal's first test run. Pick one with ?portal=crazygames|gamemonetize|none
 * or the build flag <meta name="kagra-portal" content="...">; the flag wins
 * when it is set. The SDK script is only requested after a provider is
 * picked, so the default build makes no external request.
 *
 * tests/test_portal_shim.py runs this file under node with fake SDKs.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.KagraPortal = api;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  "use strict";

  const PORTALS = ["none", "crazygames", "gamemonetize"];
  const SDK_URL = {
    crazygames: "https://sdk.crazygames.com/crazygames-sdk-v3.js",
    gamemonetize: "https://api.gamemonetize.com/sdk.js",
  };
  /** Breaks where a midgame ad may be requested. `retry`: the player taps retry on
   *  the result card (heading back into play). `game_over`: the result card has
   *  just come up. */
  const COMMERCIAL_SLOTS = ["game_over", "retry"];
  const REWARDED_AT = ["game_over", "title"];
  const DEFAULT_ADS = { commercial_on: ["retry"], rewarded: [], min_interval_secs: 90 };

  // ---- ads spec -----------------------------------------------------------

  /** `platform.ads` from the bundle, made safe. Never throws; bad entries are dropped. */
  const normalizeAds = (raw) => {
    const ads = raw && typeof raw === "object" ? raw : {};
    const on = Array.isArray(ads.commercial_on)
      ? ads.commercial_on.filter((s) => COMMERCIAL_SLOTS.includes(s))
      : DEFAULT_ADS.commercial_on.slice();
    const rewarded = [];
    for (const r of Array.isArray(ads.rewarded) ? ads.rewarded : []) {
      if (!r || typeof r !== "object" || typeof r.slot !== "string" || !r.slot) continue;
      rewarded.push({
        slot: r.slot,
        at: REWARDED_AT.includes(r.at) ? r.at : "game_over",
        effect: typeof r.effect === "string" ? r.effect : "none",
        per_run: Math.max(1, Math.min(9, Math.floor(Number(r.per_run) || 1))),
      });
    }
    const gap = Number(ads.min_interval_secs);
    return {
      commercial_on: Array.from(new Set(on)),
      rewarded,
      min_interval_secs: Number.isFinite(gap) ? Math.max(0, Math.min(3600, gap)) : DEFAULT_ADS.min_interval_secs,
    };
  };

  // ---- provider selection -------------------------------------------------

  /** Which portal. The build flag wins when it names a portal; otherwise ?portal=. */
  const pickPortal = (search, metaPortal) => {
    const clean = (v) => String(v || "").trim().toLowerCase();
    const built = clean(metaPortal);
    if (PORTALS.includes(built) && built !== "none") return built;
    let asked = "";
    try { asked = clean(new URLSearchParams(search || "").get("portal")); } catch {}
    return PORTALS.includes(asked) ? asked : "none";
  };

  // ---- providers ----------------------------------------------------------
  // A provider: { name, sdkUrl, stopOnFocusLoss, requestTimeoutMs,
  //   load(env) -> Promise, init() -> Promise, loadingStart(), loadingStop(),
  //   gameplayStart(), gameplayStop(), requestAd(kind, {started, finished, error}) }
  // Every method may throw; the host catches.

  const noneProvider = () => ({
    name: "none",
    sdkUrl: "",
    stopOnFocusLoss: true,
    requestTimeoutMs: 0,
    load: () => Promise.resolve(),
    init: () => Promise.resolve(),
    loadingStart() {},
    loadingStop() {},
    gameplayStart() {},
    gameplayStop() {},
    requestAd(kind, hooks) { hooks.error(new Error("no portal")); },
  });

  /** CrazyGames SDK v3 (docs.crazygames.com/sdk/intro). */
  const crazyGamesProvider = () => {
    let sdk = null;
    return {
      name: "crazygames",
      sdkUrl: SDK_URL.crazygames,
      // "Don't call gameplayStop when the user switches focus" (their Game docs).
      stopOnFocusLoss: false,
      // Unfilled requests answer adError by themselves.
      requestTimeoutMs: 12000,
      load(env) { return env.loadScript(SDK_URL.crazygames); },
      async init(env) {
        const cg = env.win.CrazyGames && env.win.CrazyGames.SDK;
        if (!cg) throw new Error("CrazyGames SDK missing");
        await cg.init();
        sdk = cg;
      },
      loadingStart() { if (sdk && sdk.game.loadingStart) sdk.game.loadingStart(); },
      loadingStop() { if (sdk && sdk.game.loadingStop) sdk.game.loadingStop(); },
      gameplayStart() { if (sdk) sdk.game.gameplayStart(); },
      gameplayStop() { if (sdk) sdk.game.gameplayStop(); },
      requestAd(kind, hooks) {
        if (!sdk) { hooks.error(new Error("sdk not ready")); return; }
        sdk.ad.requestAd(kind === "rewarded" ? "rewarded" : "midgame", {
          adStarted: () => hooks.started(),
          adFinished: () => hooks.finished(),
          adError: (e) => hooks.error(e),
        });
      },
    };
  };

  /** GameMonetize HTML5 SDK (GameMonetize.com-SDK on GitHub).
   *  Midgame only: it has no rewarded API. The game id is the hash from the
   *  dashboard, given as <meta name="kagra-portal-id"> or ?portal_id=. */
  const gameMonetizeProvider = () => {
    let hooks = null;
    let ready = false;
    return {
      name: "gamemonetize",
      sdkUrl: SDK_URL.gamemonetize,
      stopOnFocusLoss: true,
      // showBanner() returns nothing and an unfilled ad raises no event: stop waiting early.
      requestTimeoutMs: 3000,
      load(env) {
        env.win.SDK_OPTIONS = {
          gameId: env.portalId || "",
          onEvent(a) {
            const name = a && a.name;
            if (name === "SDK_READY") ready = true;
            else if (name === "SDK_GAME_PAUSE" && hooks) hooks.started();
            else if (name === "SDK_GAME_START" && hooks) hooks.finished();
          },
        };
        return env.loadScript(SDK_URL.gamemonetize);
      },
      async init(env) {
        if (!env.win.sdk) throw new Error("GameMonetize SDK missing");
      },
      loadingStart() {},
      loadingStop() {},
      gameplayStart() {},
      gameplayStop() {},
      requestAd(kind, h) {
        const s = this._env && this._env.win.sdk;
        if (kind === "rewarded") { h.error(new Error("rewarded ads are not supported")); return; }
        if (!s || typeof s.showBanner !== "function") { h.error(new Error("sdk not ready")); return; }
        hooks = h;
        s.showBanner();
      },
      get ready() { return ready; },
    };
  };

  /** A logging stand-in for a portal whose SDK is not wired yet. Ads never show. */
  const createStubProvider = (name, log) => {
    const note = (...a) => { if (log) log(`[portal:${name}]`, ...a); };
    return {
      name,
      sdkUrl: "",
      stopOnFocusLoss: true,
      requestTimeoutMs: 0,
      load: () => Promise.resolve(),
      init: () => Promise.resolve(),
      loadingStart: () => note("loadingStart"),
      loadingStop: () => note("loadingStop"),
      gameplayStart: () => note("gameplayStart"),
      gameplayStop: () => note("gameplayStop"),
      requestAd(kind, hooks) { note("requestAd", kind); hooks.error(new Error("stub portal has no ads")); },
    };
  };

  const makeProvider = (name) =>
    name === "crazygames" ? crazyGamesProvider() : name === "gamemonetize" ? gameMonetizeProvider() : noneProvider();

  // ---- the host -----------------------------------------------------------

  /**
   * opts: { name, ads (platform.ads), provider (override), win, doc, portalId,
   *         now() ms, setTimeout/clearTimeout, loadTimeoutMs, onBusy(bool), onAd(active, kind), log }
   */
  const createHost = (opts = {}) => {
    const win = opts.win || (typeof window !== "undefined" ? window : {});
    const doc = opts.doc || (typeof document !== "undefined" ? document : null);
    const now = opts.now || (() => Date.now());
    const timers = {
      set: opts.setTimeout || ((f, ms) => setTimeout(f, ms)),
      clear: opts.clearTimeout || ((h) => clearTimeout(h)),
    };
    const log = opts.log || (() => {});
    const provider = opts.provider || makeProvider(PORTALS.includes(opts.name) ? opts.name : "none");
    const ads = normalizeAds(opts.ads);
    const loadTimeoutMs = opts.loadTimeoutMs == null ? 8000 : opts.loadTimeoutMs;

    const loadScript = (url) => new Promise((resolve, reject) => {
      if (!doc || !doc.createElement) { reject(new Error("no document")); return; }
      const el = doc.createElement("script");
      el.src = url;
      el.async = true;
      el.onload = () => resolve();
      el.onerror = () => reject(new Error(`could not load ${url}`));
      (doc.head || doc.documentElement).appendChild(el);
    });
    const env = { win, doc, loadScript, portalId: opts.portalId || "" };
    provider._env = env;

    let status = provider.name === "none" ? "none" : "idle";
    let adBusy = false; // an ad is requested or showing: the UI is blocked
    let adActive = false; // an ad is actually playing: game muted and paused
    let playing = false; // what the portal has been told
    let wantPlaying = false; // what the page says (applied once an ad is over)
    let loading = false;
    let runRewards = new Map();
    // The first break may show an ad at once: one only comes after a finished run.
    let lastAdAt = now() - ads.min_interval_secs * 1000;
    let lastAdKind = "";
    let initPromise = null;
    // Who hears about ads: the page glue (bindPage) or the caller's own callbacks.
    const cb = { onBusy: opts.onBusy, onAd: opts.onAd };

    const safe = (fn, label) => {
      try { return fn(); } catch (err) { log(`portal ${provider.name}: ${label} failed`, err); return undefined; }
    };
    const usable = () => status === "ready";

    const init = () => {
      if (initPromise) return initPromise;
      if (provider.name === "none") { initPromise = Promise.resolve(); return initPromise; }
      status = "loading";
      let timer = null;
      const timeout = new Promise((_, rej) => {
        timer = timers.set(() => rej(new Error("portal SDK timed out")), loadTimeoutMs);
      });
      // load() runs now, not on a later tick: the script tag goes in the page at once.
      const work = new Promise((resolve) => resolve(provider.load(env))).then(() => provider.init(env));
      initPromise = Promise.race([work, timeout])
        .then(() => {
          status = "ready";
          if (loading) safe(() => provider.loadingStart(), "loadingStart");
        })
        .catch((err) => {
          // Ad blocker, offline, wrong host: the game runs without the portal.
          status = "unavailable";
          log(`portal ${provider.name}: unavailable`, err);
        })
        .then(() => { timers.clear(timer); });
      return initPromise;
    };

    const syncGameplay = () => {
      if (adActive || !usable()) return;
      if (wantPlaying && !playing) { playing = true; safe(() => provider.gameplayStart(), "gameplayStart"); }
      else if (!wantPlaying && playing) { playing = false; safe(() => provider.gameplayStop(), "gameplayStop"); }
    };

    const loadingStart = () => {
      if (loading) return;
      loading = true;
      if (usable()) safe(() => provider.loadingStart(), "loadingStart");
    };
    const loadingFinished = () => {
      if (!loading) return;
      loading = false;
      if (usable()) safe(() => provider.loadingStop(), "loadingStop");
    };
    const gameplayStart = () => { wantPlaying = true; syncGameplay(); };
    const gameplayStop = () => { wantPlaying = false; syncGameplay(); };
    /** The page paused. `focus` pauses (tab hidden, window blur) are not reported to
     *  portals that handle focus themselves (CrazyGames). */
    const interrupt = (reason) => {
      if (reason === "focus" && !provider.stopOnFocusLoss) return;
      gameplayStop();
    };
    const resumed = (reason) => {
      if (reason === "focus" && !provider.stopOnFocusLoss) return;
      gameplayStart();
    };
    const newRun = () => { runRewards = new Map(); };

    const setBusy = (on) => {
      if (adBusy === on) return;
      adBusy = on;
      if (cb.onBusy) safe(() => cb.onBusy(on), "onBusy");
    };
    const setActive = (on, kind) => {
      if (adActive === on) return;
      adActive = on;
      if (cb.onAd) safe(() => cb.onAd(on, kind), "onAd");
      if (!on) syncGameplay();
    };

    /** One ad round trip. Always resolves, never rejects. */
    const runAd = (kind) => new Promise((resolve) => {
      let started = false;
      let finished = false;
      let settled = false;
      let t1 = null;
      let t2 = null;
      const done = (ok, reason) => {
        if (settled) return;
        settled = true;
        timers.clear(t1);
        timers.clear(t2);
        setActive(false, kind);
        setBusy(false);
        syncGameplay(); // an ad that never played still stopped gameplay for the portal
        lastAdAt = now();
        lastAdKind = kind;
        resolve({ shown: started, completed: ok && finished, reason: reason || "" });
      };
      setBusy(true);
      // The portal stops hearing "playing" while the ad is up.
      if (playing) { playing = false; safe(() => provider.gameplayStop(), "gameplayStop"); }
      const hooks = {
        started: () => {
          if (settled || started) return;
          started = true;
          timers.clear(t1);
          setActive(true, kind);
          t2 = timers.set(() => done(false, "stuck"), 180000);
        },
        finished: () => { finished = true; done(true, "finished"); },
        error: (e) => done(false, e && e.message ? String(e.message) : "error"),
      };
      t1 = timers.set(() => { if (!started) done(false, "timeout"); }, provider.requestTimeoutMs || 8000);
      try { provider.requestAd(kind, hooks); }
      catch (err) { done(false, err && err.message ? String(err.message) : "throw"); }
    });

    const idle = () => ({ shown: false, completed: false, reason: "" });

    /** Midgame ad at a break. Resolves {shown}. Respects `commercial_on` and the interval. */
    const commercial = (slot) => {
      if (!usable()) return Promise.resolve({ ...idle(), reason: "no_portal" });
      if (!ads.commercial_on.includes(slot)) return Promise.resolve({ ...idle(), reason: "slot_off" });
      if (adBusy) return Promise.resolve({ ...idle(), reason: "busy" });
      if (now() - lastAdAt < ads.min_interval_secs * 1000) return Promise.resolve({ ...idle(), reason: "too_soon" });
      return runAd("midgame");
    };

    const rewardedSlot = (slot) => ads.rewarded.find((r) => r.slot === slot) || null;
    /** Could the page offer this rewarded slot right now? (A button should only show if so.) */
    const canOfferRewarded = (slot) => {
      const r = rewardedSlot(slot);
      if (!r || !usable() || adBusy) return false;
      if (provider.name === "gamemonetize") return false;
      return (runRewards.get(slot) || 0) < r.per_run;
    };
    /** Rewarded ad the player asked for. `rewarded` is true only after adFinished. */
    const rewarded = async (slot) => {
      if (!canOfferRewarded(slot)) return { ...idle(), rewarded: false, reason: "not_offered" };
      const res = await runAd("rewarded");
      const ok = !!res.completed;
      if (ok) runRewards.set(slot, (runRewards.get(slot) || 0) + 1);
      return { ...res, rewarded: ok };
    };

    return {
      name: provider.name,
      ads,
      provider,
      init,
      loadingStart,
      loadingFinished,
      gameplayStart,
      gameplayStop,
      interrupt,
      resumed,
      newRun,
      commercial,
      rewarded,
      canOfferRewarded,
      /** Replace the ad callbacks: { onBusy(bool), onAd(active, kind) }. */
      bind(callbacks) { Object.assign(cb, callbacks); },
      get status() { return status; },
      get busy() { return adBusy; },
      get adActive() { return adActive; },
      get playing() { return playing; },
      get lastAdKind() { return lastAdKind; },
    };
  };

  // ---- page glue ----------------------------------------------------------

  const CSS = `
html, body { -webkit-user-select: none; -moz-user-select: none; user-select: none; -webkit-touch-callout: none; }
#kagra-pause, #kagra-ad { position: fixed; inset: 0; display: none; place-items: center; text-align: center;
  background: rgba(6, 4, 14, 0.78); color: #f4f1ea; font: 700 16px/1.5 system-ui, "Hiragino Sans", "Noto Sans JP", sans-serif; }
#kagra-pause { z-index: 8; cursor: pointer; }
#kagra-ad { z-index: 9; }
#kagra-pause.visible, #kagra-ad.visible { display: grid; }
#kagra-pause h2, #kagra-ad h2 { margin: 0 0 6px; font-size: clamp(26px, 8vw, 40px); letter-spacing: 0.12em; color: #8cffff; }
#kagra-pause p, #kagra-ad p { margin: 0; opacity: 0.8; font-weight: 500; font-size: 14px; }
`;

  /**
   * Wire a host into the page.
   * o: { win, doc, host, session, audio, t, getPhase() }
   * Returns { phase(next), start(phase, go), blocked(), pause(reason), resume(), paused, destroy() }.
   */
  const bindPage = (o) => {
    const { win, doc, host, session, audio } = o;
    const t = o.t || ((k) => k);
    const getPhase = o.getPhase || (() => "title");
    let paused = false;
    let pauseReason = "";
    let adPaused = false;
    let adMuteWas = false;
    let starting = false;
    let overTimer = null;
    const cleanup = [];
    const listen = (target, type, fn, opt) => {
      target.addEventListener(type, fn, opt);
      cleanup.push(() => target.removeEventListener(type, fn, opt));
    };

    const style = doc.createElement("style");
    style.textContent = CSS;
    (doc.head || doc.documentElement).appendChild(style);
    const card = (id, title, sub) => {
      const el = doc.createElement("div");
      el.id = id;
      el.setAttribute("role", "status");
      const h = doc.createElement("h2");
      h.textContent = title;
      const p = doc.createElement("p");
      p.textContent = sub;
      const box = doc.createElement("div");
      box.append(h, p);
      el.appendChild(box);
      (doc.getElementById("app") || doc.body).appendChild(el);
      return el;
    };
    const pauseEl = card("kagra-pause", t("paused"), t("paused.tap"));
    const adEl = card("kagra-ad", t("ad.wait"), t("ad.sub"));

    const audioDown = (on) => { if (audio && typeof audio.hidden === "function") audio.hidden(on); };

    const pause = (reason) => {
      if (paused || host.busy || getPhase() !== "playing") return;
      paused = true;
      pauseReason = reason || "pause";
      try { session.pause(); } catch {}
      pauseEl.classList.add("visible");
      audioDown(true);
      host.interrupt(pauseReason);
    };
    const resume = () => {
      if (!paused) return;
      paused = false;
      pauseEl.classList.remove("visible");
      try { session.resume(); } catch {}
      if (!doc.hidden) audioDown(false);
      host.resumed(pauseReason);
    };

    // Pause when the page loses the player. The player resumes with a tap.
    listen(doc, "visibilitychange", () => { if (doc.hidden) pause("focus"); });
    listen(win, "pagehide", () => pause("focus"));
    listen(win, "blur", () => pause("focus"));
    listen(pauseEl, "pointerup", (e) => { e.preventDefault(); e.stopPropagation(); resume(); });
    listen(pauseEl, "pointerdown", (e) => { e.preventDefault(); e.stopPropagation(); });
    listen(win, "keydown", (e) => {
      const k = e.key;
      if (k === "Escape" || k === "p" || k === "P") {
        if (paused) resume(); else pause("pause");
        e.preventDefault();
      } else if (paused && (k === " " || k === "Enter")) {
        resume();
        e.preventDefault();
      }
    }, true);

    // No page scroll from the wheel, the keys or a pinch, and no long-press menu.
    listen(win, "wheel", (e) => e.preventDefault(), { passive: false });
    listen(win, "keydown", (e) => {
      if (["PageUp", "PageDown", "Home", "End"].includes(e.key)) e.preventDefault();
    });
    for (const type of ["gesturestart", "gesturechange", "contextmenu"]) {
      listen(doc, type, (e) => e.preventDefault());
    }

    // An ad: block the UI from the request, mute and pause once it plays.
    host.bind({
      onBusy: (on) => adEl.classList.toggle("visible", on),
      onAd: (active) => {
        if (active) {
          adMuteWas = !!(audio && audio.muted);
          if (audio && typeof audio.setMuted === "function") audio.setMuted(true);
          audioDown(true);
          if (getPhase() === "playing" && !paused) { adPaused = true; try { session.pause(); } catch {} }
        } else {
          if (audio && typeof audio.setMuted === "function") audio.setMuted(adMuteWas);
          if (!paused && !doc.hidden) audioDown(false);
          if (adPaused) { adPaused = false; try { session.resume(); } catch {} }
        }
      },
    });

    const blocked = () => host.busy || host.adActive || paused;

    const phase = (next) => {
      if (overTimer) { win.clearTimeout(overTimer); overTimer = null; }
      if (next === "playing") {
        host.newRun();
        host.gameplayStart();
      } else {
        host.gameplayStop();
        if (next === "complete") {
          // The result card first; the break comes a beat later, never over a tap.
          overTimer = win.setTimeout(() => {
            overTimer = null;
            if (getPhase() === "complete") host.commercial("game_over");
          }, 900);
        }
      }
    };

    /** The player asked to start or retry. A midgame ad may come first (retry only). */
    const start = (from, go) => {
      if (blocked() || starting) return;
      if (from !== "complete") { go(); return; }
      starting = true;
      host.commercial("retry").then(() => { starting = false; go(); }, () => { starting = false; go(); });
    };

    return {
      phase,
      start,
      blocked,
      pause,
      resume,
      get paused() { return paused; },
      get adPaused() { return adPaused; },
      destroy() { for (const fn of cleanup.splice(0)) fn(); style.remove && style.remove(); },
    };
  };

  /** The page's one call: read the flags from the URL and the <meta> tags, build the host. */
  const fromPage = (win, doc, ads) => {
    const meta = (name) => {
      const el = doc && doc.querySelector ? doc.querySelector(`meta[name="${name}"]`) : null;
      return el ? el.getAttribute("content") || "" : "";
    };
    const search = (win.location && win.location.search) || "";
    const name = pickPortal(search, meta("kagra-portal"));
    let portalId = meta("kagra-portal-id");
    if (!portalId) { try { portalId = new URLSearchParams(search).get("portal_id") || ""; } catch {} }
    const host = createHost({
      name, ads, win, doc, portalId,
      log: (...a) => { if (win.console) win.console.warn(...a); },
    });
    return host;
  };

  const isPortalBuild = (doc) => {
    const el = doc && doc.querySelector ? doc.querySelector('meta[name="kagra-build"]') : null;
    return !!el && el.getAttribute("content") === "portal";
  };

  return {
    PORTALS, SDK_URL, COMMERCIAL_SLOTS, REWARDED_AT, DEFAULT_ADS,
    normalizeAds, pickPortal, createHost, createStubProvider, fromPage, bindPage, isPortalBuild,
  };
});
