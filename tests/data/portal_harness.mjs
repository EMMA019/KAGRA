// Runs scripts/preview_site/portal.js and i18n.js under node with fake SDKs, a fake
// DOM and a fake clock. Prints one JSON object of results for tests/test_portal_shim.py.
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const Portal = require("../../scripts/preview_site/portal.js");
const I18n = require("../../scripts/preview_site/i18n.js");

// ---- fakes ---------------------------------------------------------------------

const makeClock = () => {
  const c = { t: 0, q: [], seq: 0 };
  c.now = () => c.t;
  c.setTimeout = (fn, ms) => { const h = ++c.seq; c.q.push({ h, at: c.t + ms, fn }); return h; };
  c.clearTimeout = (h) => { c.q = c.q.filter((x) => x.h !== h); };
  c.advance = async (ms) => {
    const end = c.t + ms;
    for (;;) {
      await tick();
      const next = c.q.filter((x) => x.at <= end).sort((a, b) => a.at - b.at || a.h - b.h)[0];
      if (!next) break;
      c.q = c.q.filter((x) => x !== next);
      c.t = next.at;
      next.fn();
    }
    c.t = end;
    await tick();
  };
  return c;
};
const tick = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };

class El {
  constructor(tag) {
    this.tag = tag; this.children = []; this.handlers = {}; this.attrs = {}; this.textContent = "";
    this.classes = new Set(); this.id = ""; this.dataset = {};
    this.classList = {
      add: (c) => this.classes.add(c), remove: (c) => this.classes.delete(c),
      toggle: (c, on) => (on ? this.classes.add(c) : this.classes.delete(c)),
      contains: (c) => this.classes.has(c),
    };
  }
  appendChild(c) { this.children.push(c); c.parent = this; return c; }
  append(...cs) { for (const c of cs) this.appendChild(c); }
  setAttribute(k, v) { this.attrs[k] = v; }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  addEventListener(t, f) { (this.handlers[t] ||= []).push(f); }
  removeEventListener(t, f) { this.handlers[t] = (this.handlers[t] || []).filter((x) => x !== f); }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter((x) => x !== this); }
}
const fire = (target, type, extra = {}) => {
  const e = { type, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {}, ...extra };
  for (const f of [...(target.handlers[type] || [])]) f(e);
  return e;
};

const makeEnv = ({ search = "", metas = {}, navLangs = ["en-US"] } = {}) => {
  const clock = makeClock();
  const head = new El("head");
  const body = new El("body");
  const scripts = [];
  const doc = new El("document");
  doc.head = head; doc.body = body; doc.documentElement = new El("html"); doc.hidden = false;
  doc.createElement = (tag) => new El(tag);
  doc.getElementById = (id) => (id === "app" ? null : null);
  doc.querySelector = (sel) => {
    const m = /meta\[name="([^"]+)"\]/.exec(sel);
    return m && m[1] in metas ? { getAttribute: () => metas[m[1]] } : null;
  };
  // A script element "loads" when the test says so.
  head.appendChild = (el) => {
    El.prototype.appendChild.call(head, el);
    if (el.tag === "script") scripts.push(el);
    return el;
  };
  const win = new El("window");
  win.location = { search };
  win.navigator = { languages: navLangs, language: navLangs[0] };
  win.console = { warn() {} };
  win.setTimeout = clock.setTimeout; win.clearTimeout = clock.clearTimeout;
  win.document = doc;
  return { clock, doc, win, scripts, head, body };
};

const makeCrazySdk = (calls, behavior = {}) => ({
  init: async () => { calls.push("init"); },
  game: {
    loadingStart: () => calls.push("loadingStart"), loadingStop: () => calls.push("loadingStop"),
    gameplayStart: () => calls.push("gameplayStart"), gameplayStop: () => calls.push("gameplayStop"),
  },
  ad: {
    requestAd: (type, cb) => {
      calls.push(`requestAd:${type}`);
      behavior.last = cb;
      if (behavior.auto === "ok") { cb.adStarted(); behavior.finish = () => cb.adFinished(); }
      if (behavior.auto === "error") cb.adError(new Error("no fill"));
    },
  },
});

const booted = async (name, ads, env, extra = {}) => {
  const host = Portal.createHost({ name, ads, win: env.win, doc: env.doc, now: env.clock.now,
    setTimeout: env.clock.setTimeout, clearTimeout: env.clock.clearTimeout, ...extra });
  return host;
};
const loadSdk = async (env, define) => {
  const s = env.scripts[env.scripts.length - 1];
  define();
  s.onload();
  await tick();
};

const out = {};

// ---- selection and spec --------------------------------------------------------

out.pick = {
  default: Portal.pickPortal("", ""),
  query: Portal.pickPortal("?portal=crazygames", ""),
  query_gm: Portal.pickPortal("?portal=GameMonetize", ""),
  junk: Portal.pickPortal("?portal=evil", ""),
  meta_wins: Portal.pickPortal("?portal=gamemonetize", "crazygames"),
  meta_none_defers: Portal.pickPortal("?portal=crazygames", "none"),
  empty_none: Portal.pickPortal("?portal=none", ""),
};
out.ads_default = Portal.normalizeAds(undefined);
out.ads_clean = Portal.normalizeAds({
  commercial_on: ["retry", "bogus", "retry", "game_over"],
  rewarded: [{ slot: "revive", at: "title", effect: 5, per_run: 99 }, { at: "title" }, null, { slot: "x", per_run: 0 }],
  min_interval_secs: 99999,
});
out.ads_junk = Portal.normalizeAds("nope");

// ---- the default build asks for nothing ---------------------------------------

{
  const env = makeEnv();
  const host = await booted("none", undefined, env);
  host.loadingStart(); await host.init(); host.loadingFinished(); host.gameplayStart();
  const res = await host.commercial("retry");
  const rw = await host.rewarded("revive");
  out.none = { status: host.status, scripts: env.scripts.length, res, rw, offer: host.canOfferRewarded("revive") };
  const page = Portal.fromPage(env.win, env.doc, undefined);
  await page.init();
  out.none_from_page = { name: page.name, scripts: env.scripts.length };
}
{
  const env = makeEnv({ search: "?portal=crazygames" });
  const host = Portal.fromPage(env.win, env.doc, undefined);
  out.query_selects = { name: host.name, scriptsBeforeInit: env.scripts.length };
  host.init();
  out.query_selects.scriptUrl = env.scripts[0] && env.scripts[0].src;
}

// ---- CrazyGames ---------------------------------------------------------------

{
  const env = makeEnv();
  const calls = [];
  const host = await booted("crazygames", { commercial_on: ["retry"], min_interval_secs: 60,
    rewarded: [{ slot: "revive", at: "game_over", per_run: 1 }] }, env);
  host.loadingStart();
  const ready = host.init();
  out.cg_before_load = { status: host.status, scripts: env.scripts.length, url: env.scripts[0].src, calls: calls.slice() };
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls) }; });
  await ready;
  host.loadingFinished();
  host.gameplayStart(); host.gameplayStart();
  host.interrupt("focus");      // tab hidden: CrazyGames wants no gameplayStop
  host.resumed("focus");
  host.interrupt("pause");      // the player paused: stop
  host.resumed("pause");
  host.gameplayStop(); host.gameplayStop();
  out.cg_events = { status: host.status, calls: calls.slice() };
}
{ // midgame ads at the right breaks
  const env = makeEnv();
  const calls = []; const beh = { auto: "ok" }; const seen = [];
  const host = await booted("crazygames", { commercial_on: ["retry"], min_interval_secs: 60 }, env,
    { onBusy: (b) => seen.push(`busy:${b}`), onAd: (a, k) => seen.push(`ad:${a}:${k}`) });
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  host.gameplayStart();
  const off = await host.commercial("game_over");
  const p = host.commercial("retry");
  const during = { busy: host.busy, active: host.adActive, playing: host.playing, again: await host.commercial("retry") };
  beh.finish();
  const res = await p;
  const after = { busy: host.busy, active: host.adActive, playing: host.playing };
  const soon = await host.commercial("retry");
  await env.clock.advance(61000);
  const later = commercialShown(await (async () => { const q = host.commercial("retry"); beh.finish(); return q; })());
  out.cg_midgame = { off, during, res, after, soon, later, calls, seen };
}
function commercialShown(r) { return r.shown; }
{ // ad error: nothing blocks
  const env = makeEnv();
  const calls = []; const beh = { auto: "error" };
  const host = await booted("crazygames", { commercial_on: ["retry"] }, env);
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  host.gameplayStart();
  const res = await host.commercial("retry");
  out.cg_error = { res, busy: host.busy, active: host.adActive, calls, playing: host.playing };
  await tick();
  host.gameplayStart();
  out.cg_error.replay = host.playing;
}
{ // an ad that never starts or answers
  const env = makeEnv();
  const calls = []; const beh = {};
  const host = await booted("crazygames", { commercial_on: ["retry"] }, env);
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  const p = host.commercial("retry");
  const mid = host.busy;
  await env.clock.advance(13000);
  out.cg_timeout = { mid, res: await p, busy: host.busy };
}
{ // an ad that starts and then never ends
  const env = makeEnv();
  const calls = []; const beh = { auto: "ok" };
  const host = await booted("crazygames", { commercial_on: ["retry"] }, env);
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  const p = host.commercial("retry");
  await env.clock.advance(181000);
  out.cg_stuck = { res: await p, busy: host.busy, active: host.adActive };
}
{ // rewarded
  const env = makeEnv();
  const calls = []; const beh = { auto: "ok" };
  const host = await booted("crazygames", { rewarded: [{ slot: "revive", at: "game_over", per_run: 1 }] }, env);
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  const offered = host.canOfferRewarded("revive");
  const unknown = await host.rewarded("nope");
  const p = host.rewarded("revive");
  beh.finish();
  const first = await p;
  const offeredAfter = host.canOfferRewarded("revive");
  host.newRun();
  out.cg_rewarded = { offered, unknown, first, offeredAfter, offeredNewRun: host.canOfferRewarded("revive"), calls };
}
{ // rewarded, but the ad errors: no reward
  const env = makeEnv();
  const calls = []; const beh = { auto: "error" };
  const host = await booted("crazygames", { rewarded: [{ slot: "revive", at: "game_over", per_run: 1 }] }, env);
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  const r = await host.rewarded("revive");
  out.cg_rewarded_error = { r, offeredAgain: host.canOfferRewarded("revive") };
}
{ // SDK blocked (ad blocker) and SDK that hangs in init
  const env = makeEnv();
  const host = await booted("crazygames", { commercial_on: ["retry"] }, env);
  host.loadingStart();
  const ready = host.init();
  env.scripts[0].onerror();
  await ready;
  host.gameplayStart(); host.gameplayStop(); host.loadingFinished();
  const res = await host.commercial("retry");
  out.cg_blocked = { status: host.status, res };
  const env2 = makeEnv();
  const host2 = await booted("crazygames", { commercial_on: ["retry"] }, env2, { loadTimeoutMs: 5000 });
  const ready2 = host2.init();
  env2.scripts[0].onload();
  env2.win.CrazyGames = { SDK: { init: () => new Promise(() => {}), game: {}, ad: {} } };
  await env2.clock.advance(5001);
  await ready2;
  out.cg_hang = { status: host2.status, res: await host2.commercial("retry") };
  const env3 = makeEnv();
  const host3 = await booted("crazygames", {}, env3);
  const ready3 = host3.init();
  env3.scripts[0].onload(); // script ran but no global
  await ready3;
  out.cg_no_global = { status: host3.status };
}

// ---- GameMonetize ----------------------------------------------------------------

{
  const env = makeEnv();
  const seen = [];
  const host = await booted("gamemonetize", { commercial_on: ["retry"], min_interval_secs: 0,
    rewarded: [{ slot: "revive" }] }, env, { portalId: "abc123", onAd: (a) => seen.push(a) });
  const ready = host.init();
  const opts = env.win.SDK_OPTIONS;
  out.gm_options = { gameId: opts.gameId, hasEvent: typeof opts.onEvent, url: env.scripts[0].src };
  await loadSdk(env, () => { env.win.sdk = { showBanner: () => { out.gm_banner = true; } }; });
  await ready;
  opts.onEvent({ name: "SDK_READY" });
  const p = host.commercial("retry");
  opts.onEvent({ name: "SDK_GAME_PAUSE" });
  const mid = { busy: host.busy, active: host.adActive };
  opts.onEvent({ name: "SDK_GAME_START" });
  const res = await p;
  out.gm = { status: host.status, mid, res, seen, banner: out.gm_banner, offer: host.canOfferRewarded("revive"),
    rew: await host.rewarded("revive") };
}

// ---- stub ------------------------------------------------------------------------

{
  const env = makeEnv();
  const log = [];
  const host = Portal.createHost({ provider: Portal.createStubProvider("poki", (...a) => log.push(a.join(" "))),
    ads: { commercial_on: ["retry"] }, win: env.win, doc: env.doc, now: env.clock.now,
    setTimeout: env.clock.setTimeout, clearTimeout: env.clock.clearTimeout });
  await host.init();
  host.loadingStart(); host.gameplayStart();
  const res = await host.commercial("retry");
  host.gameplayStop(); host.loadingFinished();
  out.stub = { name: host.name, res, log, scripts: env.scripts.length };
}

// ---- the page glue ------------------------------------------------------------

const pageFixture = async (portalOpts = {}) => {
  const env = makeEnv();
  const calls = []; const beh = { auto: "ok" };
  const host = await booted("crazygames", { commercial_on: ["retry", "game_over"], min_interval_secs: 0 }, env);
  const ready = host.init();
  await loadSdk(env, () => { env.win.CrazyGames = { SDK: makeCrazySdk(calls, beh) }; });
  await ready;
  let phase = "title";
  const session = { log: [], pause() { this.log.push("pause"); }, resume() { this.log.push("resume"); } };
  const audio = { muted: portalOpts.muted === true, log: [],
    setMuted(on) { this.muted = on; this.log.push(`muted:${on}`); },
    hidden(on) { this.log.push(`hidden:${on}`); } };
  const page = Portal.bindPage({ win: env.win, doc: env.doc, host, session, audio, t: (k) => k, getPhase: () => phase });
  return { env, calls, beh, host, session, audio, page, setPhase: (p) => { phase = p; page.phase(p); } };
};

{ // pause rules
  const f = await pageFixture();
  f.env.doc.hidden = true; fire(f.env.doc, "visibilitychange");
  const title = { log: f.session.log.slice(), paused: f.page.paused };
  f.setPhase("playing");
  const startCalls = f.calls.slice();
  f.env.doc.hidden = true; fire(f.env.doc, "visibilitychange");
  const hidden = { paused: f.page.paused, log: f.session.log.slice(), audio: f.audio.log.slice(), calls: f.calls.slice() };
  f.env.doc.hidden = false; fire(f.env.doc, "visibilitychange");
  const back = { paused: f.page.paused, still: f.session.log.slice() };
  const card = f.env.body.children.find((c) => c.id === "kagra-pause");
  fire(card, "pointerup");
  const resumed = { paused: f.page.paused, log: f.session.log.slice(), audio: f.audio.log.slice(), cardVisible: card.classes.has("visible") };
  fire(f.env.win, "blur");
  const blur = f.page.paused;
  const escRes = fire(f.env.win, "keydown", { key: "Escape" });
  const afterEsc = { paused: f.page.paused, prevented: escRes.defaultPrevented };
  fire(f.env.win, "keydown", { key: "p" });
  const afterP = f.page.paused;
  fire(f.env.win, "keydown", { key: "P" });
  const afterP2 = f.page.paused;
  out.page_pause = { title, startCalls, hidden, back, resumed, blur, afterEsc, afterP, afterP2, calls: f.calls.slice() };
}
{ // scroll and gesture guards
  const f = await pageFixture();
  const wheel = fire(f.env.win, "wheel");
  const pgdn = fire(f.env.win, "keydown", { key: "PageDown" });
  const ctx = fire(f.env.doc, "contextmenu");
  const gesture = fire(f.env.doc, "gesturestart");
  const arrow = fire(f.env.win, "keydown", { key: "ArrowLeft" });
  out.page_guards = { wheel: wheel.defaultPrevented, pagedown: pgdn.defaultPrevented, context: ctx.defaultPrevented,
    gesture: gesture.defaultPrevented, arrowUntouched: !arrow.defaultPrevented };
}
{ // a run ends, then the player retries through a midgame ad
  const f = await pageFixture({ muted: false });
  f.setPhase("playing");
  f.setPhase("complete");
  const resultAdBefore = f.calls.filter((c) => c.startsWith("requestAd")).length;
  await f.env.clock.advance(901);
  const gameOverAd = f.calls.filter((c) => c.startsWith("requestAd")).length;
  f.beh.finish();
  await tick();
  let went = 0;
  f.page.start("complete", () => { went += 1; });
  const midRetry = { blocked: f.page.blocked(), went, adCard: f.env.body.children.find((c) => c.id === "kagra-ad").classes.has("visible"),
    audio: f.audio.log.slice(), session: f.session.log.slice() };
  f.page.start("complete", () => { went += 100; }); // a second tap while waiting
  f.beh.finish();
  await tick();
  out.page_retry = { resultAdBefore, gameOverAd, midRetry, wentAfter: went, blocked: f.page.blocked(),
    audio: f.audio.log.slice(), session: f.session.log.slice(), calls: f.calls.slice() };
}
{ // ad while playing mutes and pauses, then restores the player's own mute choice
  const f = await pageFixture({ muted: true });
  f.setPhase("playing");
  const p = f.host.commercial("retry");
  const during = { muted: f.audio.muted, session: f.session.log.slice(), audio: f.audio.log.slice(), paused: f.page.adPaused };
  f.beh.finish();
  await p;
  out.page_ad_playing = { during, muted: f.audio.muted, session: f.session.log.slice(), audio: f.audio.log.slice(), paused: f.page.adPaused };
  const g = await pageFixture({ muted: false });
  g.setPhase("playing");
  const q = g.host.commercial("retry");
  const m1 = g.audio.muted;
  g.beh.finish();
  await q;
  out.page_ad_unmuted = { during: m1, after: g.audio.muted };
}
{ // start from the title never waits; start is ignored while an ad is up
  const f = await pageFixture();
  let n = 0;
  f.page.start("title", () => { n += 1; });
  out.page_start_title = n;
}

// ---- language ---------------------------------------------------------------------

out.lang = {
  default_en: I18n.pick("", ["en-US"]),
  ja_nav: I18n.pick("", ["ja-JP", "en"]),
  ja_second: I18n.pick("", ["en-US", "ja"]),
  query_ja: I18n.pick("?lang=ja", ["en-US"]),
  query_en_over_nav: I18n.pick("?lang=en", ["ja-JP"]),
  junk: I18n.pick("?lang=fr", ["fr-FR"]),
  none: I18n.pick("", []),
};
{
  const en = I18n.create("en"); const ja = I18n.create("ja");
  const keysEn = Object.keys(I18n.TABLE.en).sort();
  const keysJa = Object.keys(I18n.TABLE.ja).sort();
  out.i18n = {
    only_en: keysEn.filter((k) => !keysJa.includes(k)),
    only_ja: keysJa.filter((k) => !keysEn.includes(k)),
    vars: [en.t("goal.coins", { n: 40 }), ja.t("goal.coins", { n: 40 })],
    unknown: en.t("no.such.key"),
    death_en: en.tryT("death.CRATE"), death_ja: ja.tryT("death.CRATE"),
    placeholders_match: keysEn.every((k) => {
      const ph = (s) => (s.match(/\{\w+\}/g) || []).sort().join();
      return ph(I18n.TABLE.en[k]) === ph(I18n.TABLE.ja[k] ?? I18n.TABLE.en[k]);
    }),
    empty: keysEn.filter((k) => !I18n.TABLE.en[k].trim()),
  };
  const env = makeEnv({ navLangs: ["ja-JP"] });
  const applied = [{ attrs: { "data-i18n": "start" }, textContent: "x" }, { attrs: { "data-i18n": "home" }, textContent: "y" }];
  for (const a of applied) a.getAttribute = (k) => a.attrs[k];
  env.doc.querySelectorAll = () => applied;
  const api = I18n.install(env.win);
  out.i18n.install = { lang: api.lang, winLang: env.win.kagraLang, applied: applied.map((a) => a.textContent), htmlLang: env.doc.documentElement.lang };
}

console.log(JSON.stringify(out));
