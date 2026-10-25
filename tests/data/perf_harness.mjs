// Runs scripts/preview_site/perf.js under node with a fake page and a virtual clock,
// and prints the results as JSON for tests/test_phone_measure.py.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

// ---- a virtual page ------------------------------------------------------------
let vt = 1000;
const net = { calls: 0 };
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
for (const n of ["fetch", "XMLHttpRequest", "WebSocket", "EventSource"]) define(n, function () { net.calls += 1; throw new Error("network"); });

class El {
  constructor(tag) { this.tag = tag; this.style = {}; this.children = []; this.listeners = {}; this.hidden = false; this.textContent = ""; this.value = ""; this.disabled = false; }
  append(...c) { this.children.push(...c); }
  appendChild(c) { this.children.push(c); return c; }
  addEventListener(ev, fn) { (this.listeners[ev] ||= []).push(fn); }
  setAttribute() {}
  remove() { this.removed = true; }
  focus() {}
  select() { this.selected = true; }
  click() { this.onclick && this.onclick(); }
}
const body = new El("body");
const docState = { exec: 0 };
const nav = { userAgent: "Mozilla/5.0 (Linux; Android 16; Pixel 10) Chrome/140", platform: "Linux armv8l", hardwareConcurrency: 8, deviceMemory: 8, maxTouchPoints: 5,
  userAgentData: { platform: "Android", mobile: true, getHighEntropyValues: async () => ({ model: "Pixel 10", platformVersion: "16.0.0", architecture: "" }) },
  gpu: { requestAdapter: async () => ({ info: { vendor: "arm", architecture: "immortalis", device: "", description: "Mali-G925" }, isFallbackAdapter: false }) },
  clipboard: { writeText: async (t) => { nav.clip = t; } } };
define("navigator", nav);
define("document", { body, head: new El("head"), createElement: (t) => new El(t), execCommand: () => { docState.exec += 1; return true; } });
define("screen", { width: 1080, height: 2424 });
globalThis.devicePixelRatio = 2.625;
globalThis.innerWidth = 412; globalThis.innerHeight = 915;
globalThis.window = globalThis;
globalThis.location = { href: "https://example.invalid/play.html?game=hop_beat&dev=1&secret=1" };

const perf = require(path.resolve(here, "../../scripts/preview_site/perf.js"));

// virtual time: one display frame at a time
function runFrames(ms, interval, onFrame) {
  const end = vt + ms;
  let frame = vt;
  while (frame < end) {
    frame += interval;
    if (vt < frame) vt = frame;
    for (const t of timers.slice()) {
      while (t.due <= frame && timers.includes(t)) { t.fn(); if (t.every) t.due += t.ms; else timers.splice(timers.indexOf(t), 1); }
    }
    const cbs = rafs.splice(0);
    for (const cb of cbs) cb(frame);
    onFrame && onFrame();
  }
}

const out = {};

// ---- pure ------------------------------------------------------------------------
const flat = Array.from({ length: 600 }, () => 16.67);
const s = perf.summarize(flat, 16.67);
out.flat = { fps: s.fps, p50: s.p50, p95: s.p95, jank: s.jank, drop: s.dropPct, grade: s.grade };
const spiky = flat.slice(); spiky[10] = 70; spiky[20] = 40; spiky[30] = 34;
const sp = perf.summarize(spiky, 16.67);
out.spiky = { jank: sp.jank, hitches: sp.hitches, drop: sp.dropPct, max: sp.max, grade: sp.grade };
out.slow = perf.summarize(Array.from({ length: 300 }, () => 33.3), 16.67).grade;
out.empty = perf.summarize([], 16.67);
const jit = (base) => Array.from({ length: 200 }, (_, i) => base * (1 + ((i * 7) % 10 - 5) / 200));
out.refresh = { hz60: perf.estimateRefresh(jit(16.67)).hz, hz90: perf.estimateRefresh(jit(11.1)).hz, hz120: perf.estimateRefresh(jit(8.33)).hz, few: perf.estimateRefresh([16, 16, 16]),
  odd: perf.estimateRefresh(jit(13.3)).hz, withHitch: perf.estimateRefresh(jit(8.33).concat([300, 500, 90])).hz };
const bk = (n, f) => Array.from({ length: n }, (_, i) => ({ dt: 16.7 * f(i), cpu: 4 * f(i) }));
out.thermal = { few: perf.thermalGuess(bk(4, () => 1)).label, flat: perf.thermalGuess(bk(8, () => 1)).label,
  warm: perf.thermalGuess(bk(8, (i) => (i < 4 ? 1 : 1.15))).label, hot: perf.thermalGuess(bk(8, (i) => (i < 4 ? 1 : 1.4))),
  faster: perf.thermalGuess(bk(8, (i) => (i < 4 ? 1.3 : 1))).label };
out.plan = {
  two: perf.planSteps({ fx_post: true, fx_atmo: true }).map((p) => [p.name, p.sw]),
  none: perf.planSteps({}).map((p) => p.name),
  one: perf.planSteps({ fx_post: true }).map((p) => p.name),
  skip: perf.planSteps({ fx_post: true, fx_atmo: true }, { skipStep: true }).map((p) => p.name),
  ms: perf.planSteps({ fx_post: true, fx_atmo: true }).map((p) => p.ms),
};
out.device = Object.keys(perf.deviceInfo({ navigator: nav, screen, window: globalThis })).sort();
out.table = perf.formatTable([{ name: "all-off", fps: 119.8, p50: 8.3, p95: 8.9, p99: 9.5, jank: 0, dropPct: 0.1, cpuMs: 3.2, rendersPerSec: 120, grade: "good" }]).split("\n");

// ---- the tool on a fake page --------------------------------------------------------
function makeSession() {
  const sw = { fx_post: true, fx_atmo: true };
  const log = [];
  const sess = {
    calls: { render: 0, step: 0 },
    switchesJson: () => JSON.stringify(sw),
    setSwitch: (k, v) => { if (!(k in sw)) return false; sw[k] = v; log.push(`${k}=${v ? 1 : 0}`); return true; },
    pictureStatus: () => "wgpu ok",
    render() { sess.calls.render += 1; vt += sess.renderCost(sw); },
    requestFrame() { sess.calls.step += 1; vt += 0.3; },
    renderCost: (s) => (s.fx_post ? 2 : 1) + (s.fx_atmo ? 1 : 0),
  };
  return { sess, sw, log };
}
const canvas = { width: 1080, height: 2424, clientWidth: 412, clientHeight: 915 };
let renderSkip = false;
const { sess, sw, log } = makeSession();
const params = new URLSearchParams("game=hop_beat&dev=1&secret=1&quality=mobile");
const tool = perf.start({
  session: sess, params, build: "abc123abc123",
  getCanvas: () => canvas, getBackend: () => "webgpu", getScale: () => ({ cap: 2, step: 0 }), getQuality: () => "mobile",
  getRenderSkip: () => renderSkip, setRenderSkip: (on) => { renderSkip = on; },
});
await new Promise((r) => setImmediate(r)); // let the adapter lookup settle
// the page's loop: sim steps at 60 Hz of a 120 Hz screen; render_skip draws only on step frames
let n = 0;
const pageFrame = () => {
  n += 1;
  const stepped = n % 2 === 0 ? 1 : 0;
  if (stepped) sess.requestFrame();
  if (!(renderSkip && !stepped)) sess.render();
};
runFrames(3000, 1000 / 120, pageFrame);
out.refreshLive = tool.refresh;
out.overlayText = body.children.length === 1 ? body.children[0].children[0].textContent : null;
out.recordStarted = tool.record({ stepMs: 4000 });
out.recordTwice = tool.record({ stepMs: 4000 });
runFrames(4000 * 5 + 1000, 1000 / 120, pageFrame);
out.recordingAfter = tool.recording;
const rep = tool.report();
out.report = {
  steps: rep.steps.map((x) => ({ name: x.name, skip: x.skip, sw: x.sw, n: x.n, fps: x.fps, p50: x.p50, rps: x.rendersPerSec, cpu: x.cpuMs, grade: x.grade, note: x.note })),
  hz: rep.display.hz, tool: rep.tool, v: rep.v, page: rep.page, device: rep.device, adapter: rep.adapter, canvas: rep.canvas, quality: rep.quality, thermal: rep.thermal,
  keys: Object.keys(rep).sort(),
};
out.switchLog = log;
out.restored = { sw: { ...sw }, renderSkip };
out.copy = {};
const root = body.children[0];
const bar = root.children[2];
const [bRec, bCopy] = bar.children;
out.copy.btnAfter = { rec: bRec.textContent, copyHidden: bCopy.hidden };
bCopy.click();
await new Promise((r) => setImmediate(r));
out.copy.clipboard = nav.clip ? JSON.parse(nav.clip).tool : null;
out.copy.clipboardBytes = nav.clip ? nav.clip.length : 0;
out.copy.hasNoUrl = nav.clip ? !/example\.invalid|secret/.test(nav.clip) : null;
// fallback: no clipboard API
delete nav.clipboard;
bCopy.click();
await new Promise((r) => setImmediate(r));
const area = root.children[3];
out.copy.fallback = { shown: area.hidden === false, parses: (() => { try { return JSON.parse(area.value).tool; } catch { return null; } })(), selected: Boolean(area.selected), exec: docState.exec };
out.net = net.calls;
out.timerLeak = timers.length;
tool.stop();
out.removed = root.removed === true;
console.log(JSON.stringify(out));
