// Runs scripts/preview_site/meta.js under node (no DOM) with a fake clock, storage and
// ad host. argv[2] = normalized `meta` block (JSON file, from kagra/meta_spec.py),
// argv[3] = kagra-shared/tests/fixtures/meta_vectors.json. Prints one JSON object of
// results for tests/test_meta_js.py.
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const require = createRequire(import.meta.url);
const M = require("../../scripts/preview_site/meta.js");
const cfg = JSON.parse(readFileSync(process.argv[2], "utf8"));
const V = JSON.parse(readFileSync(process.argv[3], "utf8"));
const out = {};
const DAY = 86400000;
const D0 = 20751; // 2026-10-25 (UTC)

// ---- the shared vectors
const bad = [];
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
for (const r of V.date) {
  if (M.dayNumber(r.ms) !== r.day || M.dateString(r.day) !== r.date) bad.push(["date", r]);
}
for (const r of V.seed) {
  if (M.dailySeed(r.game, r.day) !== r.seed) bad.push(["seed", r]);
  if (M.pickModifier(r.game, r.day, r.mods) !== r.pick) bad.push(["pick", r]);
}
for (const r of V.streak) {
  const n = M.streakNext(r.last, r.streak, r.today);
  if (n.streak !== r.next || n.counted !== r.counted) bad.push(["streak", r]);
}
for (const r of V.reward) {
  if (M.streakReward({ base: r.base, per_day: r.per_day, max: r.max }, r.streak) !== r.coins) bad.push(["reward", r]);
}
for (const r of V.earn) {
  if (M.earn({ base: r.base, per: r.per, cap: r.cap }, r.run) !== r.coins) bad.push(["earn", r]);
}
for (const r of V.patch) {
  if (M.patchAllowed(r.patch, r.allow) !== r.allowed) bad.push(["allowed", r]);
  if (!eq(M.applyPatch(r.doc, r.patch, r.allow), r.result)) bad.push(["patch", r]);
}
out.vector_mismatches = bad;
out.vector_counts = Object.fromEntries(Object.keys(V).filter((k) => k !== "_note").map((k) => [k, V[k].length]));

// ---- fakes
const memStore = (init) => {
  const m = new Map(Object.entries(init || {}));
  return {
    m,
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => { m.set(k, String(v)); },
  };
};
const world = () => ({
  genre: "hop_beat",
  game: { id: "hop_beat", skin: "classic", enemies: { stomp_score: 150 }, look: { hero: [255, 214, 120], slab: [58, 52, 92], glow: [96, 232, 255], accent: [255, 92, 170] },
    course: { seed: 7, ramp_bars: 40, rest_every_bars: 8 }, audio: { master: 1 }, lives: { count: 3 } },
});
let clock = D0 * DAY + 5 * 3600 * 1000;
const make = (store, extra) => M.create({ config: cfg, gameId: "hop_beat", title: "HOP-BEAT", url: "https://example.invalid/play", storage: store,
  now: () => clock, lang: "en", doc: null, clipboard: null, ...(extra || {}) });
const run = (m, score, combo, perfect, revives) => {
  m.phase("playing", { revives: revives || 0 });
  m.phase("complete", { revives: revives || 0, result: { run: { score, combo, perfect } } });
};

// ---- inert
out.inert = (() => {
  const m = M.create({ config: null });
  return { enabled: m.enabled, same: eq(m.prepare({ a: 1 }), { a: 1 }), items: m.items().length };
})();

// ---- a wallet over runs, a revive and a double
out.wallet = (() => {
  const store = memStore();
  const m = make(store);
  const seen = {};
  seen.start = m.state().coins;
  run(m, 1234, 40, 10);
  seen.run1 = m.state().coins;
  seen.run1_award = m.lastAward();
  // a Continue: same run, higher score, pays only the difference
  m.phase("playing", { revives: 1 });
  m.phase("complete", { revives: 1, result: { run: { score: 2000, combo: 55, perfect: 20 } } });
  seen.after_revive = m.state().coins;
  seen.runs_after_revive = m.state().stats.runs;
  // showing the same result again changes nothing
  m.phase("complete", { revives: 1, result: { run: { score: 2000, combo: 55, perfect: 20 } } });
  seen.same_again = m.state().coins;
  // a fresh run
  run(m, 500, 5, 2);
  seen.run2 = m.state().coins;
  seen.runs = m.state().stats.runs;
  seen.best_score = m.state().stats.best_score;
  // a run that scored nothing earns nothing
  run(m, 0, 0, 0);
  seen.zero = m.state().coins;
  seen.saved = JSON.parse(store.getItem("kagra.meta.hop_beat")).coins;
  seen.reloaded = make(store).state().coins;
  return seen;
})();

// ---- shop: buy, equip, rules, prepare()
out.shop = (() => {
  const store = memStore();
  const m = make(store);
  const seen = {};
  const states = () => Object.fromEntries(m.items().map((i) => [i.id, i.state]));
  seen.initial = states();
  seen.poor = m.buy("hero.mint");
  seen.locked = m.buy("hero.ember");
  seen.prepare_default = M.sanitize({}, cfg).equipped;
  // earn enough: five runs of 1000 score
  for (let i = 0; i < 12; i += 1) run(m, 1000, 20, 10);
  seen.coins = m.state().coins;
  seen.runs = m.state().stats.runs;
  seen.after_runs = states();
  const before = m.state().coins;
  seen.buy = m.buy("hero.mint");
  seen.spent = before - m.state().coins;
  seen.buy_again = m.buy("hero.mint");
  seen.equip_unowned = m.equip("hero.gold");
  let reloads = 0;
  const m2 = make(store, { reload: () => { reloads += 1; } });
  seen.equip = m2.equip("hero.mint");
  const w = m2.prepare(world());
  seen.hero = w.game.skin;
  seen.slab_untouched = w.game.look.slab;
  seen.seed_free = w.game.course.seed;
  seen.world_not_mutated = world().game.skin;
  seen.ice = m2.state().stats.best_combo >= 16 ? states().x : null;
  return seen;
})();

// ---- daily
out.daily = (() => {
  const store = memStore();
  let reloads = 0;
  const m = make(store, { reload: () => { reloads += 1; } });
  const seen = {};
  m.setMode("daily");
  seen.mode = m.state().mode;
  const w = m.prepare(world());
  seen.seed = w.game.course.seed;
  seen.expect_seed = M.dailySeed("hop_beat", D0);
  seen.info = m.dailyInfo();
  // run 1 today
  run(m, 1500, 30, 12);
  const s1 = m.state();
  seen.day1 = { streak: s1.daily.streak, best: s1.daily.best_score, coins: s1.coins, played: s1.stats.daily_played, award: m.lastAward() };
  // run 2 same day, lower: best stays, no second bonus
  run(m, 900, 10, 3);
  const s2 = m.state();
  seen.day1b = { streak: s2.daily.streak, best: s2.daily.best_score, today: s2.daily.today, played: s2.stats.daily_played };
  // next day
  clock += DAY;
  const m2 = make(store, { reload: () => {} });
  m2.prepare(world());
  seen.next_day_seed_differs = m2.prepare(world()).game.course.seed !== seen.seed;
  run(m2, 800, 8, 2);
  seen.day2 = { streak: m2.state().daily.streak, best_today: m2.state().daily.today.score, best_all: m2.state().daily.best_score, bonus: m2.lastAward().bonus };
  // skip two days
  clock += 3 * DAY;
  const m3 = make(store, { reload: () => {} });
  m3.prepare(world());
  seen.info_after_gap = m3.dailyInfo().streak;
  run(m3, 700, 7, 1);
  seen.day5 = { streak: m3.state().daily.streak, bonus: m3.lastAward().bonus, best_streak: m3.state().stats.best_streak };
  // the page stayed open past midnight: the run no longer counts as the daily
  clock += DAY;
  const before = m3.state().daily.streak;
  run(m3, 5000, 90, 40);
  seen.stale = { streak: m3.state().daily.streak === before, info_stale: m3.dailyInfo().stale, daily_best_unchanged: m3.state().daily.best_score === 1500 };
  seen.reloads = reloads;
  seen.share = m3.shareText();
  // modifier for the day is stable and patches only the allowed paths
  const mm = make(memStore(), { reload: () => {} });
  mm.setMode("daily");
  const wd = mm.prepare(world());
  seen.mod = mm.dailyInfo().modifier;
  seen.mod_audio_untouched = wd.game.audio.master;
  seen.lives_untouched = wd.game.lives.count;
  return seen;
})();

// ---- share
out.share = await (async () => {
  const written = [];
  const store = memStore();
  const m = make(store, { clipboard: { writeText: async (t) => { written.push(t); } }, mode: "daily" });
  m.prepare(world());
  run(m, 1234, 40, 5);
  const r = await m.share();
  const free = make(memStore(), { clipboard: { writeText: async (t) => { written.push(t); } } });
  run(free, 77, 3, 1);
  const f = await free.share();
  const nofail = make(memStore(), { clipboard: { writeText: async () => { throw new Error("denied"); } }, doc: null });
  run(nofail, 50, 2, 0);
  const nf = await nofail.share();
  const ja = make(memStore(), { lang: "ja", clipboard: null });
  run(ja, 10, 1, 0);
  return { ok: r.ok, text: r.text, written, free_ok: f.ok, free_text: f.text, denied_ok: nf.ok, denied_text: nf.text, ja: ja.shareText() };
})();

// ---- rewarded: double
out.rewarded = await (async () => {
  const calls = [];
  const host = {
    canOfferRewarded: (slot) => { calls.push(["can", slot]); return slot === "double_coins"; },
    rewarded: async (slot) => { calls.push(["rw", slot]); return { rewarded: true }; },
  };
  const m = make(memStore(), { host });
  run(m, 1234, 40, 10);
  const c0 = m.state().coins;
  const can1 = m.canDouble();
  const ok = await m.doubleCoins();
  const c1 = m.state().coins;
  const can2 = m.canDouble();
  const again = await m.doubleCoins();
  // a refused ad pays nothing and the run still keeps its coins
  const host2 = { canOfferRewarded: () => true, rewarded: async () => ({ rewarded: false }) };
  const m2 = make(memStore(), { host: host2 });
  run(m2, 1234, 40, 10);
  const d = await m2.doubleCoins();
  // no host (a portal without rewarded ads): no offer
  const m3 = make(memStore(), { host: { canOfferRewarded: () => false } });
  run(m3, 1234, 40, 10);
  return { c0, c1, can1, can2, ok, again, refused: d, refused_coins: m2.state().coins === c0, none_can: m3.canDouble(), calls };
})();

// ---- corrupt, tampered, old and future saves
out.corrupt = (() => {
  const key = "kagra.meta.hop_beat";
  const res = {};
  const tries = {
    garbage: "{not json",
    empty: "",
    array: "[1,2,3]",
    number: "42",
    nulls: "null",
    strings: JSON.stringify({ v: "x", coins: "lots", stats: "no", owned: "yes", equipped: 5, daily: [] }),
    negative: JSON.stringify({ v: 1, coins: -50, lifetime: -1, stats: { runs: -3, best_score: 1e30 } }),
    nan_like: JSON.stringify({ v: 1, coins: null, stats: { runs: "7" } }),
    unknown_ids: JSON.stringify({ v: 1, owned: { "hero.mint": true, "nope": true, "hero.gold": "yes" }, equipped: { hero: "hero.gold", look: "nope" } }),
    owned_list_v0: JSON.stringify({ coins: 40, owned: ["hero.mint", "look.dusk", 7], equipped: { hero: "hero.mint" } }),
    future: JSON.stringify({ v: 9, coins: 12, brand_new_field: { x: 1 }, owned: { "hero.mint": true }, equipped: { hero: "hero.mint" } }),
    proto: '{"__proto__":{"polluted":true},"owned":{"__proto__":true},"v":1}',
    bad_daily: JSON.stringify({ v: 1, daily: { last_day: "yesterday", streak: 5, today: { day: -4, score: "x" } }, mode: "daily" }),
  };
  for (const [name, text] of Object.entries(tries)) {
    const store = memStore({ [key]: text });
    let threw = null;
    let m = null;
    try {
      m = make(store);
      m.prepare(world());
      run(m, 100, 3, 1);
      m.items();
      m.dailyInfo();
    } catch (e) { threw = String(e && e.stack || e); }
    const s = m ? m.state() : null;
    res[name] = {
      threw,
      coins: s && s.coins,
      runs: s && s.stats.runs,
      best_score: s && s.stats.best_score,
      equipped: s && s.equipped,
      owned: s && Object.keys(s.owned).sort(),
      v: s && s.v,
      streak: s && s.daily.streak,
      mode: s && s.mode,
      bad_kept: store.m.has(`${key}.bad`),
    };
  }
  res.polluted = {}.polluted === undefined;
  // storage that throws on every call, and no storage at all
  const hostile = { getItem() { throw new Error("blocked"); }, setItem() { throw new Error("full"); } };
  const h = make(hostile);
  run(h, 1000, 10, 5);
  res.hostile = { coins: h.state().coins, persistent: h.persistent() };
  const none = make(null);
  run(none, 1000, 10, 5);
  res.nostore = { coins: none.state().coins, persistent: none.persistent() };
  return res;
})();

// ---- prepare() ignores a catalogue patch that strays outside `patchable`
out.guard = (() => {
  const evil = JSON.parse(JSON.stringify(cfg));
  evil.catalog.find((i) => i.id === "hero.mint").patch = { audio: { master: 0 } };
  const store = memStore();
  const m = M.create({ config: evil, gameId: "hop_beat", storage: store, now: () => clock, doc: null });
  for (let i = 0; i < 12; i += 1) run(m, 1000, 20, 10);
  m.buy("hero.mint");
  m.equip("hero.mint");
  const w = m.prepare(world());
  return { audio: w.game.audio.master, hero: w.game.skin };
})();

console.log(JSON.stringify(out));
