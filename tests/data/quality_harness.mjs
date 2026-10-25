// Runs scripts/preview_site/quality.js under node: the pure pieces, then the page logic on a fake
// page with a virtual clock. Prints JSON for tests/test_quality_tiers.py.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const root = path.resolve(here, "../..");
const spec = JSON.parse(fs.readFileSync(path.join(root, "kagra-shared/src/quality_tiers.json"), "utf8"));
const auto = spec.auto;

class El {
  constructor(tag) { this.tag = tag; this.children = []; this.listeners = {}; this.textContent = ""; this.parentNode = null; this.id = ""; }
  appendChild(c) { c.parentNode = this; this.children.push(c); return c; }
  insertBefore(c, ref) { c.parentNode = this; const i = this.children.indexOf(ref); this.children.splice(i < 0 ? this.children.length : i, 0, c); return c; }
  addEventListener(ev, fn) { (this.listeners[ev] ||= []).push(fn); }
  closest() { return this; }
  click() { for (const fn of this.listeners.click || []) fn({ stopPropagation() {}, preventDefault() {} }); }
}
const net = { calls: 0 };
for (const n of ["fetch", "XMLHttpRequest", "WebSocket"]) Object.defineProperty(globalThis, n, { value() { net.calls += 1; throw new Error("network"); }, configurable: true, writable: true });
const card = new El("div");
const soundP = new El("p"); soundP.id = "soundRow";
const sound = new El("button"); sound.id = "sound"; sound.closest = () => soundP;
soundP.appendChild(sound);
card.appendChild(soundP);
const doc = { hidden: false, getElementById: (id) => (id === "sound" ? sound : null), createElement: (t) => new El(t) };
Object.defineProperty(globalThis, "document", { value: doc, configurable: true, writable: true });
globalThis.window = globalThis;
globalThis.performance = { now: () => vt };
let vt = 5000;

const Q = require(path.join(root, "scripts/preview_site/quality.js"));
const out = {};

// ---- pure -----------------------------------------------------------------------------------
out.alias = Object.fromEntries(["auto", "AUTO", "safe", "mid", "high", "max", "low", "mobile", "ultra", "", null, "legacy"].map((w) => [String(w), Q.aliasQuality(w)]));
const flat = (n, v) => Array.from({ length: n }, () => v);
const ws = Q.windowStats(flat(100, 16.7).concat([40, 40, 40]));
out.stats = { n: ws.n, p95: Math.round(ws.p95 * 10) / 10, slowPct: Math.round(ws.slowPct * 10) / 10 };
out.good = [Q.isGood(Q.windowStats(flat(100, 16.7)), auto), Q.isGood(Q.windowStats(flat(100, 20)), auto), Q.isGood(Q.windowStats(flat(10, 16.7)), auto)];

// the probe: feed gaps for a number of seconds; frames are `gap` ms apart
function runProbe(gapFor, startTier) {
  const a = Object.assign({}, auto, startTier ? { start: startTier } : {});
  const p = Q.createProbe(a);
  const tried = [];
  let done = null, t = 0;
  while (t < 120000 && !done) {
    const gap = gapFor(p.tier, t);
    t += gap;
    const r = p.feed(gap);
    if (r && r.try) tried.push({ tier: r.try, at: Math.round(t / 1000) });
    if (r && r.done) done = { tier: r.done, at: Math.round(t / 1000) };
  }
  return { tried: tried.map((x) => x.tier), done: done && done.tier, at: done && done.at };
}
out.probe = {
  allGood: runProbe(() => 16.7),
  midBad: runProbe((tier) => (tier === "safe" ? 16.7 : 30)),
  highBad: runProbe((tier) => (tier === "high" ? 30 : 16.7)),
  allBad: runProbe(() => 40),
  stall: (() => { // a 3 s stop in the first window restarts it
    let hit = false;
    return runProbe((tier, t) => { if (!hit && t > 2000) { hit = true; return 3000; } return 16.7; });
  })(),
};

// the guard
function runGuard(gapFor, opts) {
  const o = Object.assign({ tier: "high", calm: () => false, secs: 120, moved: [] }, opts);
  const g = Q.createGuard(auto, o.tier);
  g.moved(o.tier, 0);
  const acts = [];
  let t = 0, n = 0;
  while (t < o.secs * 1000) {
    const gap = gapFor(n, t);
    t += gap; n += 1;
    const a = g.feed(gap, t, o.calm(t));
    if (a) { acts.push([a, Math.round(t / 1000)]); g.moved(a === "down" ? "mid" : "high", t); if (o.tier === "mid" && a === "up") o.tier = "high"; }
  }
  return { acts, banned: g.banned("high", t) };
}
out.guard = {
  steady: runGuard(() => 16.7).acts,
  tenPct: runGuard((n) => (n % 10 === 0 ? 40 : 16.7), { secs: 20 }).acts,
  threePct: runGuard((n) => (n % 33 === 0 ? 33.3 : 16.7), { secs: 60 }).acts,
  gap: runGuard((n, t) => (Math.floor(t / 1000) === 4 ? 2000 : 16.7), { secs: 12 }).acts,
  upCalmOnly: runGuard(() => 16.7, { tier: "mid", secs: 60, calm: () => false }).acts,
  upCalm: runGuard(() => 16.7, { tier: "mid", secs: 60, calm: () => true }).acts,
  atFloor: runGuard(() => 40, { tier: "safe", secs: 20 }).acts,
};
{
  // after a step down the tier above is banned: a calm, good stretch does not climb back for ban_secs
  const g = Q.createGuard(auto, "high");
  g.moved("high", 0);
  let t = 0, n = 0, down = null, up = null;
  while (t < 400000) {
    const bad = t < 8000;
    const gap = bad && n % 8 === 0 ? 40 : 16.7;
    t += gap; n += 1;
    const a = g.feed(gap, t, true);
    if (a === "down" && down == null) { down = Math.round(t / 1000); g.moved("mid", t); }
    else if (a === "up" && up == null) { up = Math.round(t / 1000); break; }
  }
  out.guardBan = { down, up, banSecs: auto.ban_secs };
}

// the setting
const mem = () => { const m = {}; return { m, getItem: (k) => (k in m ? m[k] : null), setItem: (k, v) => { m[k] = String(v); } }; };
{
  const s = mem();
  Q.saveSetting(s, { mode: "auto", tier: "high", ts: 123 });
  out.setting = {
    roundTrip: Q.loadSetting(s),
    key: Object.keys(s.m),
    garbage: (() => { s.m[Q.STORE_KEY] = "{nope"; return Q.loadSetting(s); })(),
    badMode: (() => { s.m[Q.STORE_KEY] = JSON.stringify({ mode: "ultra" }); return Q.loadSetting(s); })(),
    badTier: (() => { s.m[Q.STORE_KEY] = JSON.stringify({ mode: "auto", tier: "ultra", ts: 1 }); return Q.loadSetting(s); })(),
    throwing: (() => { const bad = { getItem() { throw new Error("denied"); }, setItem() { throw new Error("denied"); } }; Q.saveSetting(bad, { mode: "mid" }); return Q.loadSetting(bad); })(),
    none: Q.loadSetting(null),
  };
}

// ---- the page logic ---------------------------------------------------------------------------
function makeSession(tiers = true) {
  const s = {
    applied: [], customs: [],
    qualityJson: () => (tiers ? JSON.stringify(spec) : "{}"),
    setQualityTier: (n) => { if (!spec.tiers[n]) return false; s.applied.push(n); return true; },
    setQualityCustom: (j) => { s.customs.push(JSON.parse(j)); },
  };
  return s;
}
let rafq = [];
function page(opts) {
  const o = Object.assign({ query: "", storage: mem(), tiers: true, phase: "play" }, opts);
  delete globalThis.kagraQuality;
  soundP.parentNode.children = soundP.parentNode.children.filter((c) => c.id !== "gfxRow");
  rafq = [];
  const session = makeSession(o.tiers);
  const caps = [];
  let wallNow = o.wall || 1.7e12;
  const ctx = {
    session, params: new URLSearchParams(o.query), storage: o.storage, now: () => vt, wall: () => wallNow,
    getPhase: () => o.phase, setScaleCap: (c, n) => caps.push([c, n]), raf: (fn) => rafq.push(fn),
  };
  const kq = Q.start(ctx);
  const frame = (gap) => { vt += gap; const cbs = rafq.splice(0); for (const cb of cbs) cb(vt); };
  const run = (secs, gap = 16.7) => { const end = vt + secs * 1000; while (vt < end) frame(gap); };
  return { kq, session, caps, frame, run, o, setWall: (w) => { wallNow = w; } };
}
const row = () => card.children.find((c) => c.id === "gfxRow") || null;
const label = () => { const r = row(); return r ? r.children[0].textContent : null; };

{ // no tiers: nothing happens
  const p = page({ tiers: false });
  out.noTiers = { active: p.kq.active, row: row() !== null, applied: p.session.applied.length, hasGlobal: globalThis.kagraQuality !== undefined };
}
{ // ?quality=high: shown for the visit, not saved
  const p = page({ query: "quality=high" });
  out.asked = { applied: p.session.applied, caps: p.caps, saved: Object.keys(p.o.storage.m), mode: p.kq.mode, tier: p.kq.tier, label: label(), probing: p.kq.probing, ahead: row() === card.children[0] };
  p.run(10);
  out.asked.afterTen = p.session.applied.length;
}
{ // ?quality=mobile and low are the old words
  out.askedOld = [page({ query: "quality=mobile" }).kq.tier, page({ query: "quality=low" }).kq.tier, page({ query: "quality=zzz" }).kq.mode];
}
{ // first run: probe from mid, up to high, then trusted for a week
  const p = page({});
  out.first = { start: p.session.applied.slice(), probing: p.kq.probing, label0: label() };
  p.run(12);
  out.first.after = p.session.applied.slice();
  out.first.tier = p.kq.tier; out.first.probing2 = p.kq.probing;
  out.first.saved = JSON.parse(p.o.storage.m[Q.STORE_KEY]);
  out.first.label = label();
  out.first.caps = p.caps.map((c) => c[0]);
  // a second visit: saved, fresh -> no probe
  const q = page({ storage: p.o.storage, wall: p.o.wall });
  out.second = { applied: q.session.applied, probing: q.kq.probing, label: label() };
  // eight days on: probe again
  const w = page({ storage: p.o.storage, wall: 1.7e12 + 8 * 86400e3 });
  out.stale = { applied: w.session.applied, probing: w.kq.probing };
}
{ // the probe waits for a run: a title screen is not a measurement
  const p = page({ phase: "title" });
  p.run(30);
  const waiting = { applied: p.session.applied.slice(), probing: p.kq.probing };
  p.o.phase = "play";
  p.run(12);
  out.probeWaits = { waiting, after: p.session.applied.slice(), probing: p.kq.probing };
}
{ // a slow phone: probe falls to safe
  const p = page({});
  p.run(30, 40);
  out.slow = { applied: p.session.applied.slice(), tier: p.kq.tier, saved: JSON.parse(p.o.storage.m[Q.STORE_KEY]).tier, probing: p.kq.probing };
}
{ // the guard steps down at runtime, and a hidden tab does not count
  const p = page({ storage: (() => { const s = mem(); Q.saveSetting(s, { mode: "auto", tier: "high", ts: 1.7e12 }); return s; })() });
  p.run(3);
  const before = p.session.applied.slice();
  let n = 0;
  const end = vt + 6000;
  while (vt < end) { n += 1; p.frame(n % 6 === 0 ? 45 : 16.7); }
  out.guardPage = { before, after: p.session.applied.slice(), tier: p.kq.tier, saved: JSON.parse(p.o.storage.m[Q.STORE_KEY]).tier, changes: p.kq.status().changes.map((c) => c.why) };
  // hidden tab: huge gaps are ignored
  const h = page({ storage: (() => { const s = mem(); Q.saveSetting(s, { mode: "auto", tier: "high", ts: 1.7e12 }); return s; })() });
  doc.hidden = true; h.frame(16.7); h.frame(60000); doc.hidden = false;
  h.run(2);
  out.hidden = { applied: h.session.applied.slice() };
}
{ // a manual tier never moves; the Graphics button cycles and saves
  const p = page({});
  p.run(12);
  const btn = row().children[0];
  const seen = [label()];
  for (let i = 0; i < 6; i += 1) { btn.click(); seen.push(label()); }
  out.cycle = { seen, saved: JSON.parse(p.o.storage.m[Q.STORE_KEY]), applied: p.session.applied.slice(-6), mode: p.kq.mode };
  btn.click(); // -> back around
  const q = page({ query: "quality=safe" });
  q.run(40, 40);
  out.manualFixed = { applied: q.session.applied, tier: q.kq.tier };
}
{ // hold pauses everything; custom / show / restore for the recorder
  const p = page({});
  p.kq.hold(true);
  p.run(30);
  const heldApplied = p.session.applied.slice();
  p.kq.custom({ scale: 1.25, atmo: 0.5 });
  const afterCustom = { tier: p.kq.tier, customs: p.session.customs.slice(), cap: p.caps[p.caps.length - 1] };
  p.kq.show("safe");
  const afterShow = { tier: p.kq.tier, cap: p.caps[p.caps.length - 1] };
  p.kq.restore();
  p.kq.hold(false);
  out.hold = { heldApplied, afterCustom, afterShow, restored: p.kq.tier, restoredIsProbeTier: p.session.applied[p.session.applied.length - 1], showUnknown: p.kq.show("ultra") };
}
out.net = net.calls;
console.log(JSON.stringify(out));
