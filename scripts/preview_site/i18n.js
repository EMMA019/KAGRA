/* KAGRA page strings, English and Japanese.
 *
 * A classic script (no module) so play.html and the node tests can both load
 * it. English is the default; Japanese is picked by ?lang=ja or by a Japanese
 * browser (navigator.language starts with "ja"). ?lang=en forces English.
 * Strings that come from the game spec (mission labels, the sim's death text)
 * stay as the spec wrote them in Japanese; English death lines are keyed by
 * the obstacle name so the page can show them without touching the spec.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.KagraI18n = api;
  if (typeof document !== "undefined" && document && document.documentElement) {
    api.install(root);
  }
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  "use strict";

  const LANGS = ["en", "ja"];

  const TABLE = {
    en: {
      "home": "Games",
      "pad.move": "Move",
      "pad.act": "Action",
      "pad.jump": "Jump",
      "blurb": "Tap to start. Left stick moves, bottom right jumps, top right is the action.",
      "blurb.swipe": "Tap to start. Keep a finger down and drag left or right to switch lanes, flick up to jump, flick down to drop or roll.",
      "keys": "WASD or arrows to move, Space to jump, J for the action. Same layout in portrait and landscape.",
      "start": "Start",
      "retry": "Retry",
      "catalog": "All games",
      "boot": "Loading…",
      "boot.progress": "Loading {done} / {total}",
      "sound.on": "Sound: ON",
      "sound.off": "Sound: OFF",
      "phase.playing": "Playing",
      "phase.result": "Result",
      "phase.title": "Title",
      "err.start": "Could not start.\n{err}\n\nIf the wasm files are missing, run ./scripts/build_wasm.sh and open this page again.",
      "err.render": "Could not start drawing.\n{err}\n\nWebGL2 or WebGPU is required.",
      "err.stopped": "Drawing stopped.\n{err}",
      "err.fallback": "WebGL2 fallback failed:\n{err}",
      "missions": "MISSIONS",
      "best.title": "BEST SCORE",
      "best": "BEST {n}",
      "goal.coins": "Collect {n} coins",
      "goal.gates": "Pass {n} gates",
      "goal.perch": "Perch up high {n} time(s)",
      "col.time": "TIME",
      "col.gates": "GATES",
      "col.coins": "COINS",
      "col.score": "SCORE",
      "col.combo": "COMBO",
      "col.perfect": "PERFECT",
      "hit": "HIT {reason}",
      "death.fallback": "You crashed",
      "death.CRATE": "You hit a crate",
      "death.BARREL": "You hit a barrel",
      "death.BARRIER": "You hit a barricade",
      "death.GENERATOR": "You hit a generator",
      "death.OVERPASS": "You bumped your head on the awning (flick down to roll)",
      "death.TRAIN": "You hit a train",
      "death.WALL": "You hit a wall (flick down to slip through the gap)",
      "death.METEOR": "You hit a meteor",
      "death.CRYSTALS": "You hit crystals",
      "death.ROCK": "You hit a rock",
      "death.fell": "You fell through a gap",
      "death.block": "You ran into a block",
      "hop.blurb": "Tap on the beat to hop. Hold for a long hop. Land past every gap in time with the music. (Space also works.)",
      "hop.continue": "Watch an ad to continue",
      "hop.sync": "Audio sync",
      "hop.sync.help": "If you tend to hop late, press +. If early, press −.",
      "paused": "PAUSED",
      "paused.tap": "Tap or press Space to continue",
      "ad.wait": "Advertisement",
      "ad.sub": "The game continues in a moment",
    },
    ja: {
      "home": "一覧",
      "pad.move": "移動",
      "pad.act": "アクション",
      "pad.jump": "ジャンプ",
      "blurb": "タップではじめる。左が移動、右下ジャンプ、右上アクション。",
      "blurb.swipe": "タップではじめる。指を置いたまま左右にドラッグで移動、上フリックでジャンプ、下フリックで急降下。",
      "keys": "WASD / 矢印、Space ジャンプ、J アクション。縦でも横でも同じ配置です。",
      "start": "はじめる",
      "retry": "もう一度",
      "catalog": "ジャンル一覧",
      "boot": "読み込み中…",
      "boot.progress": "読み込み {done} / {total}",
      "sound.on": "サウンド: ON",
      "sound.off": "サウンド: OFF",
      "phase.playing": "プレイ中",
      "phase.result": "リザルト",
      "phase.title": "タイトル",
      "err.start": "起動できませんでした。\n{err}\n\nwasm が無いときは ./scripts/build_wasm.sh のあと、このページを出してください。",
      "err.render": "描画を開始できません。\n{err}\n\nWebGL2 か WebGPU が必要です。",
      "err.stopped": "描画が止まりました。\n{err}",
      "err.fallback": "WebGL2 fallback failed:\n{err}",
      "missions": "ミッション",
      "best.title": "ベストスコア",
      "best": "ベスト {n}",
      "goal.coins": "コインを{n}枚あつめる",
      "goal.gates": "ゲートを{n}回くぐる",
      "goal.perch": "高いところに{n}回のる",
      "col.time": "生存",
      "col.gates": "ゲート",
      "col.coins": "コイン",
      "col.score": "スコア",
      "col.combo": "コンボ",
      "col.perfect": "パーフェクト",
      "hit": "HIT {reason}",
      "death.fell": "穴に落ちた",
      "death.block": "ブロックにぶつかった",
      "hop.blurb": "ビートに合わせてタップでジャンプ、長押しで大ジャンプ。リズムよく穴を越えよう。（Space キーでも遊べます）",
      "hop.continue": "広告を見てつづける",
      "hop.sync": "音ズレ補正",
      "hop.sync.help": "遅れて押しがちなら＋、早いなら−を押します。",
      "paused": "ポーズ",
      "paused.tap": "タップか Space で再開",
      "ad.wait": "広告",
      "ad.sub": "このあとゲームに戻ります",
    },
  };

  const pick = (search, navLangs) => {
    let asked = "";
    try { asked = String(new URLSearchParams(search || "").get("lang") || "").trim().toLowerCase(); } catch {}
    if (asked) {
      const short = asked.slice(0, 2);
      if (LANGS.includes(short)) return short;
    }
    const list = Array.isArray(navLangs) ? navLangs : [navLangs];
    for (const l of list) {
      if (typeof l === "string" && /^ja\b/i.test(l.trim())) return "ja";
      if (typeof l === "string" && l.trim()) break;
    }
    return "en";
  };

  const create = (lang) => {
    const code = LANGS.includes(lang) ? lang : "en";
    const has = (key) => Object.prototype.hasOwnProperty.call(TABLE[code], key) ||
      Object.prototype.hasOwnProperty.call(TABLE.en, key);
    const t = (key, vars) => {
      let s = Object.prototype.hasOwnProperty.call(TABLE[code], key)
        ? TABLE[code][key]
        : TABLE.en[key];
      if (typeof s !== "string") return String(key);
      if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
      return s;
    };
    // Only keys this language itself has (no English fallback): the page uses
    // it for lines Japanese gets from the spec instead (the sim's death text).
    const tryT = (key, vars) =>
      Object.prototype.hasOwnProperty.call(TABLE[code], key) ? t(key, vars) : null;
    /** [data-i18n="key"] elements get the text. */
    const apply = (doc) => {
      if (!doc || !doc.querySelectorAll) return;
      doc.documentElement.lang = code;
      for (const el of doc.querySelectorAll("[data-i18n]")) el.textContent = t(el.getAttribute("data-i18n"));
    };
    return { lang: code, t, tryT, has, apply };
  };

  /** Pick from the page's location and navigator, and publish `t` on the window. */
  const install = (win) => {
    const nav = win.navigator || {};
    const langs = nav.languages && nav.languages.length ? nav.languages : [nav.language];
    const loc = win.location || {};
    const api = create(pick(loc.search, langs));
    win.kagraLang = api.lang;
    win.kagraT = api.t;
    api.apply(win.document);
    return api;
  };

  return { LANGS, TABLE, pick, create, install };
});
