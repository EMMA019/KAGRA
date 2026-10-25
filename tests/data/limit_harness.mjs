// Runs the recorder robustness and "Find my limit" parts of scripts/preview_site/perf.js under
// node with a virtual clock; prints JSON for tests/test_phone_measure.py.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const root = path.resolve(here, "../..");

let vt = 1000;
const rafs = [];
const timers = [];
let timerId = 1;
const define = (k, v) => Object.defineProperty(globalThis, k, { value: v, configurable: true, writable: true });
define("performance", { now: () => vt });
define("setInterval", (fn, ms) => { const id = timerId++; timers.push({ id, fn, ms, due: vt + ms, every: true }); return id; });
define("setTimeout", (fn, ms) => { const id = timerId++; timers.push({ id, fn, ms, due: vt + ms, every: false }); return id; });
define("clearInterval", (id) => { const i = timers.findIndex((t) => t.id === id); if (i >= 0) timers.splice(i, 1); });
define("clearTimeout", globalThis.clearInterval);
define("requestAnimationFrame", (fn) => { rafs.push(fn); return rafs.length; });

class El {
  constructor(tag) { this.tag = tag; this.style = {}; this.children = []; this.listeners = {}; this.hidden = false; this.textContent = ""; this.disabled = false; }
  append(...c) { this.children.push(...c); }
  appendChild(c) { this.children.push(c); return c; }
  addEventListener() {}
  setAttribute() {}
  remove() { this.removed = true; }
  click() { this.onclick && this.onclick(); }
}
const body = new El("body");
const nav = { userAgent: "Mozilla/5.0 (Linux; Android 17; Pixel 10) Chrome/155", platform: "Linux armv8l", hardwareConcurrency: 8, maxTouchPoints: 5,
  gpu: { requestAdapter: async () => ({ info: { vendor: "imagination", architecture: "d-series", device: "", description: "PowerVR" }, features: new Set(["timestamp-query"]), isFallbackAdapter: false }) } };
define("navigator", nav);
const docObj = { body, hidden: false, head: new El("head"), createElement: (t) => new El(t) };
define("document", docObj);
define("screen", { width: 1080, height: 2424 });
globalThis.devicePixelRatio = 2.625;
globalThis.innerWidth = 412; globalThis.innerHeight = 915;
globalThis.window = globalThis;
globalThis.location = { href: "https://example.invalid/play.html" };

const perf = require(path.join(root, "scripts/preview_site/perf.js"));
const spec = JSON.parse(fs.readFileSync(path.join(root, "kagra-shared/src/quality_tiers.json"), "utf8"));
const out = {};

// ---- pure: the ladder -----------------------------------------------------------------
const names = (dpr) => perf.planLimit(spec, { dpr }).map((s) => s.name + (s.tierDone ? `[${s.tierDone}]` : ""));
out.ladder3 = names(2.625);
out.ladder1 = names(1);
out.ladderFirst = perf.planLimit(spec, { dpr: 2.625 })[0];
const lad = perf.planLimit(spec, { dpr: 3 });
out.suggest = {
  safe: perf.suggestTier(spec, lad[0].reach, 3),
  mid: perf.suggestTier(spec, lad.find((s) => s.tierDone === "mid").reach, 3),
  high: perf.suggestTier(spec, lad.find((s) => s.tierDone === "high").reach, 3),
  max: perf.suggestTier(spec, lad[lad.length - 1].reach, 3),
  midButNoBloom: perf.suggestTier(spec, Object.assign({}, lad.find((s) => s.tierDone === "mid").reach, { bloom_scale: 0 }), 3),
  dprOne: perf.suggestTier(spec, { ...lad.find((s) => s.tierDone === "mid").reach, scale: 1 }, 1),
};
out.pass = [
  perf.limitPass({ n: 300, p95: 16.9, p99: 17.5 }),
  perf.limitPass({ n: 300, p95: 17.4, p99: 17.5 }),
  perf.limitPass({ n: 300, p95: 16.9, p99: 21 }),
  perf.limitPass({ n: 30, p95: 16.9, p99: 17 }),
  perf.limitPass({ n: 300, p95: 16.9, p99: 17, stalled: true }),
];

// ---- a fake game on a fake page -----------------------------------------------------------
function makeGame(opts) {
  const o = Object.assign({ bot: true, runMs: 6000, stallAt: null, fail: null }, opts);
  const sw = { fx_post: true, fx_atmo: true };
  if (o.bot) sw.autoplay = false;
  const g = { flow: "title", flowSince: vt, runs: 0, over: {}, tier: "mid", resumes: 0, log: [], paused: false };
  const cost = () => {
    const sc = Math.min(globalThis.devicePixelRatio, g.over.scale != null ? g.over.scale : (spec.tiers[g.tier] || spec.tiers.mid).scale);
    const atmo = g.over.atmo != null ? g.over.atmo : (spec.tiers[g.tier] || spec.tiers.mid).atmo;
    return 3 + 4 * sc * sc + 2 * atmo;
  };
  const sess = {
    switchesJson: () => JSON.stringify(sw),
    setSwitch: (k, v) => { if (!(k in sw)) return false; sw[k] = v; g.log.push(`${k}=${v ? 1 : 0}`); return true; },
    pictureStatus: () => "wgpu ok",
    gameJson: () => JSON.stringify({ phase: g.flow }),
    resumeRun: () => { g.resumes += 1; g.flow = "play"; g.flowSince = vt; },
    render() { vt += 0.5; g.renderMs = cost(); },
    requestFrame() { vt += 0.2; },
  };
  const kqLog = [];
  const quality = {
    spec, mode: "auto", tier: "mid", holds: 0,
    show(n) { g.tier = n; g.over = {}; quality.tier = n; kqLog.push(`show ${n}`); return true; },
    custom(over) { g.over = Object.assign({}, g.over, over); quality.tier = "custom"; kqLog.push(`custom ${JSON.stringify(over.scale)}`); return over.scale || 1; },
    restore() { g.tier = "mid"; g.over = {}; quality.tier = "mid"; kqLog.push("restore"); },
    hold(on) { quality.holds += on ? 1 : -1; kqLog.push(on ? "hold" : "release"); },
  };
  // a game clock tied to the virtual display: runs end after runMs; the bot restarts a run
  const tickGame = (dtMs) => {
    if (o.bot && sw.autoplay === true) {
      if (g.flow !== "play" && vt - g.flowSince > 750 && g.flow !== "pause") { g.flow = "play"; g.flowSince = vt; g.runs += 1; }
    }
    if (g.flow === "play" && vt - g.flowSince > o.runMs) { g.flow = "result"; g.flowSince = vt; }
  };
  return { g, sw, sess, quality, kqLog, tickGame, cost };
}

function runFrames(ms, game, opts) {
  const o = Object.assign({ stall: null }, opts);
  const end = vt + ms;
  let frame = vt;
  let stalled = false;
  while (frame < end) {
    let step = 1000 / 60;
    const c = game ? game.cost() : 0;
    if (game && c > step) step = c;
    if (o.stall && !stalled && frame >= o.stall.at) { step = o.stall.ms; stalled = true; }
    frame += step;
    if (vt < frame) vt = frame;
    for (const t of timers.slice()) {
      while (t.due <= frame && timers.includes(t)) { t.fn(); if (t.every) t.due += t.ms; else timers.splice(timers.indexOf(t), 1); }
    }
    if (game) game.tickGame(step);
    const cbs = rafs.splice(0);
    for (const cb of cbs) cb(frame);
    if (game && game.page) game.page();
  }
}

// a fake WebGL2 canvas with the timer extension
function makeGl(available) {
  const q = [];
  const gl = {
    QUERY_RESULT_AVAILABLE: 1, QUERY_RESULT: 2,
    getExtension: (n) => (available && n === "EXT_disjoint_timer_query_webgl2" ? { TIME_ELAPSED_EXT: 7, GPU_DISJOINT_EXT: 9 } : null),
    createQuery: () => ({ ms: 0 }),
    beginQuery: () => {}, endQuery: () => {},
    deleteQuery: () => {},
    getParameter: () => false,
    getQueryParameter: (qq, what) => (what === 1 ? true : 4.5e6),
  };
  return gl;
}

// ---- 1. the plain recorder keeps a run going and counts a stall ---------------------------------
{
  const game = makeGame({ bot: true, runMs: 5000 });
  const canvas = { width: 1080, height: 2424, clientWidth: 412, clientHeight: 915, getContext: () => null };
  const params = new URLSearchParams("game=hop_beat&dev=1");
  game.page = () => { game.sess.requestFrame(); game.sess.render(); };
  delete globalThis.kagraPerf;
  const tool = perf.start({ session: game.sess, params, build: "t", getCanvas: () => canvas, getBackend: () => "webgpu", getScale: () => ({ cap: 2, step: 0 }), getQuality: () => "mid" });
  await new Promise((r) => setImmediate(r));
  runFrames(2000, game);
  out.rec1 = { started: tool.record({ stepMs: 8000 }) };
  runFrames(8000 * 4 + 2000, game, { stall: { at: vt + 12000, ms: 2500 } });
  const rep = tool.report();
  out.rec1.steps = rep.steps.map((s) => ({ name: s.name, n: s.n, wallMs: s.wallMs, coverage: s.coverage, stalls: s.stalls, stallMs: s.stallMs, stalled: s.stalled, runs: s.runs, runEnds: s.runEnds, playPct: s.playPct, bot: s.bot, note: s.note || null }));
  out.rec1.sw = game.sw;
  out.rec1.log = game.g.log.filter((x) => x.startsWith("autoplay"));
  out.rec1.limitInReport = rep.limit;
  out.rec1.game = { runs: game.g.runs };
  tool.stop();
}

// ---- 2. a game with no bot: the run ends and the note says so ---------------------------------------
{
  const game = makeGame({ bot: false, runMs: 3000 });
  const canvas = { width: 1, height: 1, clientWidth: 1, clientHeight: 1, getContext: () => null };
  game.g.flow = "play"; game.g.flowSince = vt;
  game.page = () => { game.sess.requestFrame(); game.sess.render(); };
  delete globalThis.kagraPerf;
  const tool = perf.start({ session: game.sess, params: new URLSearchParams("dev=1"), build: "t", getCanvas: () => canvas, getBackend: () => "webgpu" });
  await new Promise((r) => setImmediate(r));
  runFrames(1500, game);
  tool.record({ stepMs: 8000 });
  runFrames(8000 * 3 + 2000, game);
  const rep = tool.report();
  out.nobot = { notes: rep.steps.map((s) => s.note || null), runEnds: rep.steps.map((s) => s.runEnds), bot: rep.steps.map((s) => s.bot), limitWithoutTiers: tool.findLimit({}) };
  tool.stop();
}

// ---- 3. Find my limit: stops at the first step that misses, reports the last that held ----------------
async function limitRun(label, opts, extra) {
  const game = makeGame(opts);
  const gl = makeGl(Boolean(extra && extra.gpu));
  const canvas = { width: 1080, height: 2424, clientWidth: 412, clientHeight: 915, getContext: (k) => (k === "webgl2" && !(extra && extra.webgpu) ? gl : null) };
  game.page = () => { game.sess.requestFrame(); game.sess.render(); };
  delete globalThis.kagraPerf;
  delete globalThis.kagraQuality;
  if (!(extra && extra.noTiers)) globalThis.kagraQuality = game.quality;
  const tool = perf.start({ session: game.sess, params: new URLSearchParams("dev=1"), build: "t", getCanvas: () => canvas, getBackend: () => "gl", getScale: () => ({ cap: 2, step: 0 }), getQuality: () => game.quality.tier });
  await new Promise((r) => setImmediate(r));
  runFrames(2000, game);
  const started = tool.findLimit({ stepMs: 8000 });
  const probe = { started, again: tool.findLimit({ stepMs: 8000 }) };
  if (started) runFrames(8000 * 20 + 4000, game, extra && extra.stall ? { stall: { at: vt + 12000, ms: 3000 } } : null);
  const rep = tool.report();
  const L = rep.limit;
  const res = {
    probe, recording: tool.recording,
    steps: L ? L.steps.map((s) => [s.name, s.pass, s.p95, s.note || null]) : null,
    lastGood: L && L.lastGood, firstFail: L && L.firstFail, complete: L && L.complete, suggested: L && L.suggestedTier, gpu: L && L.gpu,
    gpuMs: L ? L.steps.map((s) => s.gpuMs ?? null) : null,
    kq: game.kqLog, holds: game.quality.holds, autoplay: game.sw.autoplay, switchLog: game.g.log,
    keys: Object.keys(rep).sort(), criteria: L && L.criteria,
    row0: L && L.steps[0] && Object.keys(L.steps[0]).sort(),
    btn: body.children.length ? true : false,
  };
  tool.stop();
  return res;
}
out.limit = await limitRun("plain", {});
out.limitGpu = await limitRun("gpu", {}, { gpu: true });
out.limitWebgpu = await limitRun("webgpu", {}, { webgpu: true });
out.limitStall = await limitRun("stall", {}, { stall: true });
out.limitNoTiers = await limitRun("notiers", {}, { noTiers: true });
console.log(JSON.stringify(out));
