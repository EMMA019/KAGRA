/*
 * KagraMeta: the retention layer every game page shares.
 *
 *   - a coin wallet earned from finished runs (formula from the manifest `meta.earn`)
 *   - an unlock catalogue (hero skins, backgrounds, music later) with prices and rules
 *   - a daily run: same seed for everyone on a UTC date, own best, a streak, a share card
 *   - run modifiers (spec patches) the daily rotates through
 *   - a shop screen and a small bar for the title / result card, EN + JA
 *
 * It never touches the simulation. It edits the *spec* a game is loaded with
 * (`prepare(world)`), reads each finished run from the game's result, and keeps its
 * own save in localStorage. All rules are pure and mirrored in kagra/meta_spec.py and
 * kagra-shared/src/game_core/meta.rs (kagra-shared/tests/fixtures/meta_vectors.json
 * is replayed by all three). See docs/meta_core.md.
 *
 * No network. The only outside call is the page's own rewarded-ad host, and only when
 * the player taps the optional "double" button.
 */
(function (root, factory) {
  if (typeof module === "object" && module.exports) module.exports = factory();
  else root.KagraMeta = factory();
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  "use strict";

  const SCHEMA = 1;
  const DAY_MS = 86400000;
  const BIG = 1000000000;
  const SLOTS = ["hero", "look", "music"];
  const STATS = ["runs", "best_score", "best_combo", "best_streak", "daily_played"];

  // ---------------------------------------------------------------- pure rules --

  const utf8 = (s) => {
    if (typeof TextEncoder !== "undefined") return new TextEncoder().encode(s);
    const out = [];
    const e = unescape(encodeURIComponent(s));
    for (let i = 0; i < e.length; i += 1) out.push(e.charCodeAt(i));
    return out;
  };
  const fnv1a32 = (text) => {
    let h = 0x811c9dc5;
    const bytes = utf8(String(text));
    for (let i = 0; i < bytes.length; i += 1) {
      h ^= bytes[i];
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h >>> 0;
  };
  const dayNumber = (ms) => Math.floor(ms / DAY_MS);
  const dateString = (day) => new Date(day * DAY_MS).toISOString().slice(0, 10);
  const dailySeed = (gameId, day) => fnv1a32(`${gameId}:${dateString(day)}`);
  const pickModifier = (gameId, day, count) =>
    count > 0 ? fnv1a32(`${gameId}:${dateString(day)}:mod`) % count : null;

  const streakNext = (lastDay, streak, today) => {
    if (lastDay === null || lastDay === undefined) return { streak: 1, counted: true };
    if (today === lastDay) return { streak: Math.max(streak, 1), counted: false };
    if (today === lastDay + 1) return { streak: Math.max(streak, 0) + 1, counted: true };
    if (today > lastDay + 1) return { streak: 1, counted: true };
    return { streak, counted: false };
  };
  const streakReward = (reward, streak) =>
    streak < 1 ? 0 : Math.min(reward.max, reward.base + reward.per_day * (streak - 1));

  const earn = (spec, run) => {
    const num = (k) => {
      const v = run ? run[k] : undefined;
      return typeof v === "number" && Number.isFinite(v) ? v : null;
    };
    const score = num("score");
    if (score === null || score <= 0) return 0;
    let total = Number(spec.base) || 0;
    for (const key of Object.keys(spec.per || {}).sort()) {
      const v = num(key);
      if (v !== null) total += spec.per[key] * Math.max(v, 0);
    }
    return Math.min(spec.cap, Math.max(0, Math.floor(total)));
  };

  const isObj = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
  const clone = (v) => JSON.parse(JSON.stringify(v));
  const mergePatch = (doc, patch) => {
    if (!isObj(patch)) return clone(patch);
    const out = isObj(doc) ? Object.assign({}, doc) : {};
    for (const key of Object.keys(patch)) {
      if (key === "__proto__" || key === "constructor" || key === "prototype") continue;
      if (patch[key] === null) delete out[key];
      else out[key] = mergePatch(out[key], patch[key]);
    }
    return out;
  };
  const patchPaths = (patch, prefix) => {
    const pre = prefix || "";
    if (isObj(patch) && Object.keys(patch).length) {
      let out = [];
      for (const k of Object.keys(patch)) out = out.concat(patchPaths(patch[k], pre ? `${pre}.${k}` : k));
      return out;
    }
    return pre ? [pre] : [];
  };
  const patchAllowed = (patch, allow) => {
    if (!isObj(patch)) return false;
    const paths = patchPaths(patch);
    return paths.length > 0 && paths.every((p) => allow.some((a) => p === a || p.startsWith(`${a}.`)));
  };
  const applyPatch = (doc, patch, allow) => (patchAllowed(patch, allow) ? mergePatch(doc, patch) : clone(doc));

  // ---------------------------------------------------------------- the save ---

  const toInt = (v, max) => {
    const n = typeof v === "number" ? v : Number(v);
    if (!Number.isFinite(n)) return 0;
    return Math.max(0, Math.min(max === undefined ? BIG : max, Math.floor(n)));
  };
  const toDay = (v) => {
    if (typeof v !== "number" || !Number.isFinite(v)) return null;
    const d = Math.floor(v);
    return d >= 0 && d <= 3000000 ? d : null;
  };

  const freshState = () => ({
    v: SCHEMA,
    coins: 0,
    lifetime: 0,
    stats: { runs: 0, best_score: 0, best_combo: 0, best_streak: 0, daily_played: 0 },
    owned: {},
    equipped: {},
    mode: "free",
    daily: {
      best_score: 0,
      best_combo: 0,
      last_day: null,
      streak: 0,
      today: { day: null, score: 0, combo: 0, runs: 0 },
    },
  });

  /** Old saves: step each version up to SCHEMA. v0 is "no version field". */
  const migrate = (raw) => {
    const src = isObj(raw) ? raw : {};
    let v = toInt(src.v, 1000);
    let cur = src;
    while (v < SCHEMA) {
      // v0 -> v1: `owned` used to be allowed as a list of ids.
      if (v === 0 && Array.isArray(cur.owned)) {
        const owned = {};
        for (const id of cur.owned) if (typeof id === "string") owned[id] = true;
        cur = Object.assign({}, cur, { owned });
      }
      v += 1;
    }
    return cur;
  };

  const defaultFor = (cfg, slot) =>
    (cfg.catalog || []).find((i) => i.slot === slot && i.price === 0 && !i.requires) || null;

  /** Anything a player (or a bad browser) put in storage -> a state that is safe to use. */
  const sanitize = (raw, cfg) => {
    const src = migrate(raw);
    const out = freshState();
    const ids = new Map((cfg.catalog || []).map((i) => [i.id, i]));
    out.coins = toInt(src.coins);
    out.lifetime = Math.max(out.coins, toInt(src.lifetime));
    const st = isObj(src.stats) ? src.stats : {};
    for (const k of STATS) out.stats[k] = toInt(st[k]);
    const owned = isObj(src.owned) ? src.owned : {};
    for (const id of Object.keys(owned)) if (ids.has(id) && owned[id] === true) out.owned[id] = true;
    const eq = isObj(src.equipped) ? src.equipped : {};
    for (const slot of SLOTS) {
      const want = typeof eq[slot] === "string" ? ids.get(eq[slot]) : null;
      const def = defaultFor(cfg, slot);
      if (want && want.slot === slot && out.owned[want.id]) out.equipped[slot] = want.id;
      else if (def) out.equipped[slot] = def.id;
    }
    out.mode = src.mode === "daily" && cfg.daily && cfg.daily.enabled ? "daily" : "free";
    const d = isObj(src.daily) ? src.daily : {};
    out.daily.best_score = toInt(d.best_score);
    out.daily.best_combo = toInt(d.best_combo);
    out.daily.last_day = toDay(d.last_day);
    out.daily.streak = out.daily.last_day === null ? 0 : toInt(d.streak, 100000);
    const t = isObj(d.today) ? d.today : {};
    out.daily.today = {
      day: toDay(t.day),
      score: toInt(t.score),
      combo: toInt(t.combo),
      runs: toInt(t.runs, 100000),
    };
    out.stats.best_streak = Math.max(out.stats.best_streak, out.daily.streak);
    return out;
  };

  /** A requirement is met when the stat reached the number. */
  const reqMet = (state, req) => !req || (state.stats[req.stat] || 0) >= req.at_least;

  // ---------------------------------------------------------------- text -------

  const STR = {
    en: {
      coins: "Coins",
      shop: "Shop",
      close: "Close",
      equip: "Equip",
      equipped: "Equipped",
      buy: "Buy {n}",
      need: "Need {n} more",
      locked: "Locked",
      apply: "Applying...",
      slot_hero: "Hero",
      slot_look: "Look",
      slot_music: "Music",
      unlock_runs: "Play {n} runs",
      unlock_best_score: "Reach a score of {n}",
      unlock_best_combo: "Reach a combo of {n}",
      unlock_best_streak: "Reach a {n}-day streak",
      unlock_daily_played: "Play {n} daily runs",
      daily: "Daily",
      daily_on: "Daily run",
      daily_off: "Free play",
      daily_switch_daily: "Play today's daily",
      daily_switch_free: "Back to free play",
      daily_line: "Daily {date}",
      streak: "Streak {n}",
      best: "Best {n}",
      today_best: "Today {n}",
      stale: "A new daily is ready.",
      refresh: "Refresh",
      earned: "+{n} coins",
      bonus: "Streak bonus +{n}",
      doubled: "Doubled!",
      double: "Double coins (watch an ad)",
      share: "Share result",
      copied: "Copied!",
      copy_failed: "Copy this text:",
      share_daily: "{title} Daily {date} score {score} combo {combo}",
      share_free: "{title} score {score} combo {combo}",
      no_coins: "Not enough coins",
      mod: "Today's twist: {name}",
    },
    ja: {
      coins: "コイン",
      shop: "ショップ",
      close: "閉じる",
      equip: "そうび",
      equipped: "そうび中",
      buy: "{n} で買う",
      need: "あと {n}",
      locked: "ロック中",
      apply: "反映中...",
      slot_hero: "キャラ",
      slot_look: "背景",
      slot_music: "音楽",
      unlock_runs: "{n} 回あそぶ",
      unlock_best_score: "スコア {n} に到達",
      unlock_best_combo: "コンボ {n} に到達",
      unlock_best_streak: "{n} 日連続でプレイ",
      unlock_daily_played: "デイリーを {n} 回あそぶ",
      daily: "デイリー",
      daily_on: "デイリー挑戦",
      daily_off: "フリープレイ",
      daily_switch_daily: "今日のデイリーに挑戦",
      daily_switch_free: "フリープレイにもどる",
      daily_line: "デイリー {date}",
      streak: "連続 {n} 日",
      best: "ベスト {n}",
      today_best: "今日 {n}",
      stale: "新しいデイリーが出ています。",
      refresh: "更新",
      earned: "+{n} コイン",
      bonus: "連続ボーナス +{n}",
      doubled: "2倍になりました！",
      double: "コインを2倍（広告を見る）",
      share: "結果をシェア",
      copied: "コピーしました！",
      copy_failed: "このテキストをコピーしてください:",
      share_daily: "{title} デイリー {date} スコア {score} コンボ {combo}",
      share_free: "{title} スコア {score} コンボ {combo}",
      no_coins: "コインが足りません",
      mod: "今日のひとクセ: {name}",
    },
  };
  const fmt = (s, vars) => String(s).replace(/\{(\w+)\}/g, (_, k) => (vars && k in vars ? String(vars[k]) : ""));

  const pickLang = (lang) => {
    const l = lang || (typeof globalThis !== "undefined" && globalThis.kagraLang) ||
      (typeof navigator !== "undefined" && navigator.language) || "en";
    return String(l).toLowerCase().startsWith("ja") ? "ja" : "en";
  };

  // ---------------------------------------------------------------- the module -

  const inert = () => ({
    enabled: false,
    prepare: (world) => world,
    attach() {},
    phase() {},
    state: () => freshState(),
    items: () => [],
  });

  /**
   * opts: { config, gameId, title, url, storage, now, lang, host, mockAds,
   *         doc, clipboard, reload }
   *   config   the normalized manifest `meta` block (bundle.meta); falsy = inert
   *   host     KagraPortal host (canOfferRewarded / rewarded) for the double-coins button
 *   mode     "daily" | "free": force the mode (a shared `?daily=1` link)
   *   reload   () => void; applies a changed equip / mode. Default: location.reload().
   */
  const create = (opts) => {
    const o = opts || {};
    const cfg = o.config;
    if (!cfg || cfg.version !== SCHEMA || !Array.isArray(cfg.catalog)) return inert();
    const gameId = String(o.gameId || "game");
    const now = o.now || (() => Date.now());
    const storage = o.storage !== undefined ? o.storage : (() => { try { return globalThis.localStorage; } catch { return null; } })();
    const lang = pickLang(o.lang);
    const T = (key, vars) => fmt((STR[lang] && STR[lang][key]) || STR.en[key] || key, vars);
    const KEY = `kagra.meta.${gameId}`;
    const allow = cfg.patchable || [];
    const title = String(o.title || gameId);
    const listeners = [];

    // ---- persistence
    let persistent = !!storage;
    const load = () => {
      let text = null;
      try { text = storage ? storage.getItem(KEY) : null; } catch { persistent = false; }
      let raw = null;
      if (text) {
        try { raw = JSON.parse(text); } catch {
          try { storage.setItem(`${KEY}.bad`, String(text).slice(0, 20000)); } catch {}
        }
      }
      return sanitize(raw, cfg);
    };
    let st = load();
    if (o.mode === "daily" || o.mode === "free") {
      st.mode = o.mode === "daily" && cfg.daily && cfg.daily.enabled ? "daily" : "free";
    }
    const grantRules = () => {
      for (const it of cfg.catalog) {
        if (!st.owned[it.id] && it.price === 0 && reqMet(st, it.requires)) st.owned[it.id] = true;
      }
      for (const slot of SLOTS) {
        const d = defaultFor(cfg, slot);
        if (d) st.owned[d.id] = true;
        if (d && !st.equipped[slot]) st.equipped[slot] = d.id;
      }
    };
    grantRules();
    const save = () => {
      st.v = SCHEMA;
      if (!storage) return;
      try { storage.setItem(KEY, JSON.stringify(st)); persistent = true; } catch { persistent = false; }
    };
    const changed = () => {
      grantRules();
      save();
      for (const fn of listeners) { try { fn(); } catch {} }
      ui.render();
    };

    // ---- catalogue
    const itemState = (it) => {
      if (st.equipped[it.slot] === it.id) return "equipped";
      if (st.owned[it.id]) return "owned";
      if (!reqMet(st, it.requires)) return "locked";
      return st.coins >= it.price ? "buyable" : "poor";
    };
    const items = () =>
      cfg.catalog.map((it) => ({
        id: it.id,
        slot: it.slot,
        name: it.name[lang] || it.name.en,
        price: it.price,
        requires: it.requires,
        state: itemState(it),
      }));
    const buy = (id) => {
      const it = cfg.catalog.find((i) => i.id === id);
      if (!it) return { ok: false, reason: "unknown" };
      if (st.owned[id]) return { ok: false, reason: "owned" };
      if (!reqMet(st, it.requires)) return { ok: false, reason: "locked" };
      if (st.coins < it.price) return { ok: false, reason: "poor" };
      st.coins -= it.price;
      st.owned[id] = true;
      changed();
      return { ok: true };
    };
    let dirty = false;
    const equip = (id) => {
      const it = cfg.catalog.find((i) => i.id === id);
      if (!it || !st.owned[id]) return { ok: false, reason: "not_owned" };
      if (st.equipped[it.slot] !== id) {
        st.equipped[it.slot] = id;
        dirty = true;
      }
      changed();
      return { ok: true };
    };

    // ---- daily
    const dailyOn = () => !!(cfg.daily && cfg.daily.enabled);
    let loadedDay = null; // the UTC day the world was built for (what the seed came from)
    const dailyActive = () => dailyOn() && st.mode === "daily";
    const modifierFor = (day) => {
      const mods = cfg.modifiers || [];
      const i = pickModifier(gameId, day, mods.length);
      return i === null ? null : mods[i];
    };
    const dailyInfo = () => {
      const day = dailyActive() && loadedDay !== null ? loadedDay : dayNumber(now());
      const mod = modifierFor(day);
      const today = st.daily.today.day === day ? st.daily.today : { score: 0, combo: 0, runs: 0 };
      return {
        enabled: dailyOn(),
        active: dailyActive(),
        day,
        date: dateString(day),
        seed: dailySeed(gameId, day),
        modifier: mod ? { id: mod.id, name: mod.name[lang] || mod.name.en } : null,
        streak: st.daily.last_day !== null && dayNumber(now()) - st.daily.last_day <= 1 ? st.daily.streak : 0,
        best_today: today.score,
        best_score: st.daily.best_score,
        stale: dailyActive() && loadedDay !== null && dayNumber(now()) !== loadedDay,
      };
    };
    const setMode = (mode) => {
      const next = mode === "daily" && dailyOn() ? "daily" : "free";
      if (next === st.mode) return;
      st.mode = next;
      dirty = true;
      changed();
      applyChange();
    };

    /** World in, world out: equipped items, then the daily modifier, then the daily seed. */
    const prepare = (world) => {
      const out = clone(world);
      if (!isObj(out.game)) return out;
      let game = out.game;
      for (const slot of SLOTS) {
        const it = cfg.catalog.find((i) => i.id === st.equipped[slot]);
        if (it) game = applyPatch(game, it.patch, allow);
      }
      loadedDay = dayNumber(now());
      if (dailyActive()) {
        const mod = modifierFor(loadedDay);
        if (mod) game = applyPatch(game, mod.patch, allow);
        const path = (cfg.daily.seed_path || "course.seed").split(".");
        let node = game;
        for (let i = 0; i < path.length - 1; i += 1) {
          if (!isObj(node[path[i]])) node[path[i]] = {};
          node = node[path[i]];
        }
        node[path[path.length - 1]] = dailySeed(gameId, loadedDay);
      }
      out.game = game;
      return out;
    };

    // ---- runs
    let run = null;
    let last = null; // what the result card shows
    const runDaily = () => dailyActive() && loadedDay !== null && dayNumber(now()) === loadedDay;
    const startRun = (sim) => {
      const revives = toInt(sim && sim.revives, 100);
      if (run && revives > 0) return; // a Continue: same run
      run = { paid: 0, counted: false, dailyCounted: false, doubled: false, daily: dailyActive() };
    };
    const settle = (sim) => {
      const r = sim && sim.result && sim.result.run;
      if (!isObj(r)) return;
      if (!run) run = { paid: 0, counted: false, dailyCounted: false, doubled: false, daily: dailyActive() };
      const score = toInt(r.score);
      const combo = toInt(r.combo);
      const coins = earn(cfg.earn, r);
      const delta = Math.max(0, coins - run.paid);
      run.paid = Math.max(run.paid, coins);
      st.coins = Math.min(BIG, st.coins + delta);
      st.lifetime = Math.min(BIG, st.lifetime + delta);
      if (!run.counted) { st.stats.runs += 1; run.counted = true; }
      st.stats.best_score = Math.max(st.stats.best_score, score);
      st.stats.best_combo = Math.max(st.stats.best_combo, combo);
      let bonus = 0;
      let newBest = false;
      const isDaily = run.daily && runDaily();
      if (isDaily) {
        const day = loadedDay;
        if (st.daily.today.day !== day) st.daily.today = { day, score: 0, combo: 0, runs: 0 };
        if (!run.dailyCounted) { st.daily.today.runs += 1; }
        if (score > st.daily.today.score) { st.daily.today.score = score; newBest = true; }
        st.daily.today.combo = Math.max(st.daily.today.combo, combo);
        st.daily.best_score = Math.max(st.daily.best_score, score);
        st.daily.best_combo = Math.max(st.daily.best_combo, combo);
        if (!run.dailyCounted) {
          run.dailyCounted = true;
          const n = streakNext(st.daily.last_day, st.daily.streak, day);
          if (n.counted) {
            st.daily.streak = n.streak;
            st.daily.last_day = day;
            st.stats.daily_played += 1;
            st.stats.best_streak = Math.max(st.stats.best_streak, n.streak);
            bonus = streakReward(cfg.daily.streak_reward, n.streak);
            st.coins = Math.min(BIG, st.coins + bonus);
            st.lifetime = Math.min(BIG, st.lifetime + bonus);
          }
        }
      }
      const prevBonus = last && last.run === run ? last.bonus : 0;
      last = {
        run,
        coins: run.paid,
        bonus: bonus || prevBonus,
        doubled: run.doubled,
        daily: isDaily,
        date: isDaily ? dateString(loadedDay) : null,
        score,
        combo,
        newBest,
      };
      changed();
    };

    const canDouble = () => {
      const slot = cfg.rewarded && cfg.rewarded.slot;
      if (!slot || !last || last.doubled || last.coins <= 0) return false;
      if (o.mockAds) return true;
      return !!(o.host && o.host.canOfferRewarded && o.host.canOfferRewarded(slot));
    };
    let adBusy = false;
    const doubleCoins = async () => {
      if (adBusy || !canDouble()) return false;
      adBusy = true;
      let ok = false;
      try {
        if (o.mockAds) {
          await new Promise((r) => setTimeout(r, 900));
          ok = true;
        } else {
          ok = !!(await o.host.rewarded(cfg.rewarded.slot)).rewarded;
        }
      } catch { ok = false; }
      adBusy = false;
      if (ok && last && !last.doubled) {
        last.doubled = true;
        last.run.doubled = true;
        st.coins = Math.min(BIG, st.coins + last.coins);
        st.lifetime = Math.min(BIG, st.lifetime + last.coins);
        changed();
      } else {
        ui.render();
      }
      return ok;
    };

    const shareText = () => {
      const l = last;
      const url = String(o.url || "");
      const head = l && l.daily
        ? T("share_daily", { title, date: l.date, score: l.score, combo: l.combo })
        : T("share_free", { title, score: l ? l.score : 0, combo: l ? l.combo : 0 });
      return url ? `${head}\n${url}` : head;
    };
    const share = async () => {
      const text = shareText();
      const clip = o.clipboard !== undefined ? o.clipboard : (typeof navigator !== "undefined" ? navigator.clipboard : null);
      try {
        if (clip && clip.writeText) { await clip.writeText(text); return { ok: true, text }; }
      } catch {}
      try {
        const d = o.doc || (typeof document !== "undefined" ? document : null);
        if (d) {
          const ta = d.createElement("textarea");
          ta.value = text;
          ta.setAttribute("readonly", "");
          ta.style.cssText = "position:fixed;left:-9999px;top:0;opacity:0";
          d.body.appendChild(ta);
          ta.select();
          const ok = d.execCommand && d.execCommand("copy");
          d.body.removeChild(ta);
          if (ok) return { ok: true, text };
        }
      } catch {}
      return { ok: false, text };
    };

    function applyChange() {
      if (!dirty) return;
      dirty = false;
      ui.busy(T("apply"));
      setTimeout(() => {
        if (typeof o.reload === "function") o.reload();
        else if (typeof location !== "undefined") location.reload();
      }, 60);
    }

    // ---- UI (only with a document)
    const doc = o.doc !== undefined ? o.doc : (typeof document !== "undefined" ? document : null);
    const ui = makeUi();

    function makeUi() {
      if (!doc || !doc.createElement) return { render() {}, busy() {}, setPhase() {}, openShop() {}, closeShop() {} };
      const h = (tag, attrs, ...kids) => {
        const el = doc.createElement(tag);
        for (const k of Object.keys(attrs || {})) {
          if (k === "class") el.className = attrs[k];
          else if (k.startsWith("on")) el.addEventListener(k.slice(2), attrs[k]);
          else el.setAttribute(k, attrs[k]);
        }
        for (const kid of kids) if (kid !== null && kid !== undefined) el.append(kid);
        return el;
      };
      const stop = (e) => { e.stopPropagation(); };
      let phase = "title";
      let bar = null;
      let shop = null;
      let shopSlot = null;
      let veil = null;

      const css = h("style", { id: "kagra-meta-css" });
      css.textContent = [
        "#kagra-meta-bar{margin:10px auto 0;max-width:420px;display:flex;flex-direction:column;gap:6px;align-items:stretch;font:600 14px system-ui,sans-serif;color:#fff}",
        "#kagra-meta-bar .km-row{display:flex;gap:8px;justify-content:center;align-items:center;flex-wrap:wrap}",
        "#kagra-meta-bar .km-note{opacity:.85;font-weight:500;text-align:center}",
        "#kagra-meta-bar .km-award{font-size:16px;text-align:center}",
        ".km-btn{min-height:44px;min-width:44px;padding:8px 14px;border-radius:12px;border:2px solid rgba(255,255,255,.35);background:rgba(255,255,255,.12);color:#fff;font:700 14px system-ui,sans-serif;cursor:pointer;touch-action:manipulation}",
        ".km-btn:disabled{opacity:.45;cursor:default}",
        ".km-btn.km-on{background:rgba(255,92,170,.35);border-color:#ff5caa}",
        ".km-coin{display:inline-block;padding:6px 12px;border-radius:999px;background:rgba(0,0,0,.35);border:1px solid rgba(255,255,255,.25)}",
        "#kagra-shop{position:fixed;inset:0;z-index:2147483000;display:flex;align-items:center;justify-content:center;background:rgba(8,6,20,.82);font:600 15px system-ui,sans-serif;color:#fff}",
        "#kagra-shop[hidden]{display:none}",
        "#kagra-shop .km-panel{width:min(94vw,460px);max-height:92vh;display:flex;flex-direction:column;background:#1d1838;border:2px solid rgba(255,255,255,.3);border-radius:18px;padding:14px;gap:10px;box-sizing:border-box}",
        "#kagra-shop .km-head{display:flex;justify-content:space-between;align-items:center;gap:8px}",
        "#kagra-shop .km-tabs{display:flex;gap:6px;flex-wrap:wrap}",
        "#kagra-shop .km-list{overflow-y:auto;display:flex;flex-direction:column;gap:8px;-webkit-overflow-scrolling:touch}",
        "#kagra-shop .km-item{display:flex;align-items:center;gap:10px;padding:8px;border-radius:12px;background:rgba(255,255,255,.07)}",
        "#kagra-shop .km-name{flex:1;min-width:0}",
        "#kagra-shop .km-sub{display:block;opacity:.75;font-weight:500;font-size:12px}",
        "#kagra-shop .km-sw{display:flex;gap:2px}",
        "#kagra-shop .km-sw i{display:block;width:16px;height:32px;border-radius:5px;border:1px solid rgba(255,255,255,.3)}",
        "#kagra-meta-veil{position:fixed;inset:0;z-index:2147483001;display:flex;align-items:center;justify-content:center;background:rgba(8,6,20,.9);color:#fff;font:700 18px system-ui,sans-serif}",
      ].join("\n");
      (doc.head || doc.body).appendChild(css);

      const colours = (it) => {
        const src = cfg.catalog.find((c) => c.id === it.id) || {};
        if (Array.isArray(src.preview) && src.preview.length) return src.preview;
        const found = [];
        const walk = (v) => {
          if (found.length >= 4) return;
          if (Array.isArray(v) && v.length === 3 && v.every((n) => Number.isInteger(n) && n >= 0 && n <= 255)) found.push(v);
          else if (isObj(v)) for (const k of Object.keys(v)) walk(v[k]);
        };
        walk(src.patch);
        return found;
      };
      const reqText = (r) => T(`unlock_${r.stat}`, { n: r.at_least });
      const coinBadge = () => h("span", { class: "km-coin" }, `${T("coins")} ${st.coins}`);

      const drawShop = () => {
        if (!shop) return;
        const panel = shop.firstChild;
        panel.textContent = "";
        const slots = SLOTS.filter((s) => cfg.catalog.some((c) => c.slot === s));
        if (!shopSlot || !slots.includes(shopSlot)) shopSlot = slots[0] || null;
        panel.append(h("div", { class: "km-head" }, h("strong", {}, T("shop")), coinBadge(),
          h("button", { class: "km-btn", type: "button", "data-km": "close", onclick: () => closeShop() }, T("close"))));
        panel.append(h("div", { class: "km-tabs" }, ...slots.map((s) =>
          h("button", { class: `km-btn${s === shopSlot ? " km-on" : ""}`, type: "button", "data-km": `tab-${s}`,
            onclick: () => { shopSlot = s; drawShop(); } }, T(`slot_${s}`)))));
        const list = h("div", { class: "km-list" });
        for (const it of items().filter((i) => i.slot === shopSlot)) {
          const sw = h("span", { class: "km-sw" }, ...colours(it).map((c) => {
            const i = h("i", {});
            i.style.background = `rgb(${c[0]},${c[1]},${c[2]})`;
            return i;
          }));
          let sub = null;
          let btn;
          if (it.state === "equipped") {
            btn = h("button", { class: "km-btn km-on", type: "button", disabled: "" }, T("equipped"));
          } else if (it.state === "owned") {
            btn = h("button", { class: "km-btn", type: "button", "data-km": `equip-${it.id}`, onclick: () => { equip(it.id); } }, T("equip"));
          } else if (it.state === "locked") {
            btn = h("button", { class: "km-btn", type: "button", disabled: "" }, T("locked"));
            sub = reqText(it.requires);
          } else if (it.state === "poor") {
            btn = h("button", { class: "km-btn", type: "button", disabled: "" }, T("buy", { n: it.price }));
            sub = T("need", { n: it.price - st.coins });
          } else {
            btn = h("button", { class: "km-btn", type: "button", "data-km": `buy-${it.id}`, onclick: () => { buy(it.id); } }, T("buy", { n: it.price }));
          }
          list.append(h("div", { class: "km-item" }, sw,
            h("span", { class: "km-name" }, it.name, sub ? h("span", { class: "km-sub" }, sub) : null), btn));
        }
        panel.append(list);
      };

      const openShop = () => {
        if (!shop) {
          shop = h("div", { id: "kagra-shop", role: "dialog", "aria-modal": "true", "aria-label": T("shop") }, h("div", { class: "km-panel" }));
          for (const ev of ["pointerdown", "pointerup", "touchstart", "touchend", "keydown", "keyup", "click"]) {
            shop.addEventListener(ev, stop);
          }
          doc.body.appendChild(shop);
        }
        shop.hidden = false;
        drawShop();
      };
      const closeShop = () => {
        if (shop) shop.hidden = true;
        applyChange();
      };

      const render = () => {
        if (shop && !shop.hidden) drawShop();
        if (!bar) return;
        bar.textContent = "";
        bar.hidden = phase === "playing";
        if (bar.hidden) return;
        const info = dailyInfo();
        const row = h("div", { class: "km-row" }, coinBadge(),
          h("button", { class: "km-btn", type: "button", "data-km": "shop", onclick: () => openShop() }, T("shop")));
        if (info.enabled) {
          row.append(h("button", { class: `km-btn${info.active ? " km-on" : ""}`, type: "button", "data-km": "mode",
            onclick: () => setMode(info.active ? "free" : "daily") }, info.active ? T("daily_switch_free") : T("daily_switch_daily")));
        }
        bar.append(row);
        if (info.active) {
          const parts = [T("daily_line", { date: info.date })];
          if (info.streak > 0) parts.push(T("streak", { n: info.streak }));
          if (info.best_today > 0) parts.push(T("today_best", { n: info.best_today }));
          else if (info.best_score > 0) parts.push(T("best", { n: info.best_score }));
          bar.append(h("div", { class: "km-note" }, parts.join(" · ")));
          if (info.modifier) bar.append(h("div", { class: "km-note" }, T("mod", { name: info.modifier.name })));
          if (info.stale) {
            bar.append(h("div", { class: "km-row" }, h("span", { class: "km-note" }, T("stale")),
              h("button", { class: "km-btn", type: "button", "data-km": "refresh", onclick: () => { dirty = true; applyChange(); } }, T("refresh"))));
          }
        }
        if (phase === "complete" && last) {
          const bits = [T("earned", { n: last.coins })];
          if (last.bonus > 0) bits.push(T("bonus", { n: last.bonus }));
          if (last.doubled) bits.push(T("doubled"));
          bar.append(h("div", { class: "km-award", "data-km": "award" }, bits.join(" · ")));
          const acts = h("div", { class: "km-row" });
          if (canDouble()) {
            acts.append(h("button", { class: "km-btn", type: "button", "data-km": "double", onclick: () => { doubleCoins(); } }, T("double")));
          }
          const shareBtn = h("button", { class: "km-btn", type: "button", "data-km": "share" }, T("share"));
          shareBtn.addEventListener("click", async () => {
            const res = await share();
            shareBtn.textContent = res.ok ? T("copied") : T("share");
            if (!res.ok) {
              const ta = h("textarea", { readonly: "", rows: "3", "aria-label": T("copy_failed") });
              ta.value = res.text;
              ta.style.cssText = "width:100%;box-sizing:border-box";
              bar.append(h("div", { class: "km-note" }, T("copy_failed")), ta);
              try { ta.focus(); ta.select(); } catch {}
            }
          });
          acts.append(shareBtn);
          bar.append(acts);
        }
      };

      const mount = () => {
        if (bar) return true;
        const card = doc.querySelector("#overlay .card") || doc.querySelector("#overlay");
        if (!card) return false;
        bar = h("div", { id: "kagra-meta-bar" });
        for (const ev of ["pointerdown", "pointerup", "touchstart", "touchend", "keydown", "keyup", "click"]) bar.addEventListener(ev, stop);
        card.append(bar);
        return true;
      };

      return {
        render,
        openShop,
        closeShop,
        busy(text) {
          if (veil) return;
          veil = h("div", { id: "kagra-meta-veil" }, text);
          doc.body.appendChild(veil);
        },
        setPhase(p) {
          phase = p;
          if (mount()) render();
        },
      };
    }

    let phaseNow = "title";
    return {
      enabled: true,
      key: KEY,
      persistent: () => persistent,
      prepare,
      /** Page hook: call once after the world is loaded and the overlay exists. */
      attach(a) { if (a && typeof a.reload === "function") o.reload = a.reload; ui.setPhase(phaseNow); },
      /** Page hook: call from showPhase(next) with the sim JSON. */
      phase(p, sim) {
        phaseNow = p;
        if (p === "playing") startRun(sim);
        else if (p === "complete") settle(sim);
        ui.setPhase(p);
      },
      state: () => clone(st),
      items,
      buy,
      equip,
      setMode,
      dailyInfo,
      doubleCoins,
      canDouble,
      share,
      shareText,
      openShop: () => ui.openShop(),
      closeShop: () => ui.closeShop(),
      lastAward: () => (last ? { coins: last.coins, bonus: last.bonus, doubled: last.doubled, daily: last.daily, score: last.score, combo: last.combo } : null),
      onChange: (fn) => listeners.push(fn),
    };
  };

  return {
    SCHEMA,
    create,
    fnv1a32,
    dayNumber,
    dateString,
    dailySeed,
    pickModifier,
    streakNext,
    streakReward,
    earn,
    mergePatch,
    patchPaths,
    patchAllowed,
    applyPatch,
    sanitize,
    migrate,
    freshState,
  };
});
