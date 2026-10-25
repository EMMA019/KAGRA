# Meta layer: wallet, unlocks, daily run, modifiers

Retention parts every game can reuse. No server, no account, no leaderboard, no network
request, nothing paid. A game opts in with one `meta` block in its manifest.

## Where it lives

| Part | File | Job |
| --- | --- | --- |
| Manifest block | `games/<name>.json` → `meta` | what to earn, sell and rotate |
| Validator + reference rules | `kagra/meta_spec.py` | `normalize_meta`, seed, date, streak, earn, patch |
| Rust mirror (pure rules) | `kagra-shared/src/game_core/meta.rs` | same rules for a native / Android shell |
| Page module | `scripts/preview_site/meta.js` | save, wallet, shop UI, daily, share card |
| Shared vectors | `kagra-shared/tests/fixtures/meta_vectors.json` | replayed by Rust, Python and JS |
| Vector generator | `tools/gen_meta_vectors.py` (`--check` in the tests) | Python reference writes the file |

`meta` is **not** part of `GameSpec`: it changes which spec is loaded, not the rules of the
simulation. So there is no wasm export, no baked-fixture change and no sim change. (Same
choice as `platform.ads`; see `docs/publishing/web_portals.md`.)

## How a game opts in

1. Add a `meta` block to `games/<game>.json` (schema below).
2. List in `meta.patchable` the spec paths a skin or modifier may touch. Anything else is
   ignored whole. HOP-BEAT: `look`, `course.ramp_bars`, `course.rest_every_bars`. It does **not**
   list `music`, `audio`, `judge` or `lives`, so no unlock can change timing, difficulty
   fairness or sound. (BPM is deliberately not patchable: the music file is fixed at 126 BPM.)
3. The page needs three things from the kit, all already true for HOP-BEAT:
   the world's `game` object is the spec (`world.game.*`); a finished run arrives as
   `sim.result.run` with numeric fields (`score`, `combo`, `perfect`, …); and `sim.revives`
   counts Continues so a revive is not paid twice.
4. Add the page hooks (below) once. After that a new game needs no page code.

## `meta` schema (version 1)

```jsonc
"meta": {
  "currency": {"id": "coins", "name": {"en": "Coins", "ja": "コイン"}},
  "earn": {                       // coins = floor(base + Σ per[k] * run[k]), capped
    "base": 3,                    // 0..1000
    "per": {"score": 0.01, "combo": 0.5, "perfect": 0.25},   // ≤ 8 keys, weights 0..1000
    "cap": 300                    // per run, 1..100000
  },
  "patchable": ["look", "course.ramp_bars"],   // dotted paths under world.game
  "catalog": [                    // ≤ 64 items
    {"id": "hero.sun", "slot": "hero", "name": {"en": "Sun", "ja": "サン"}, "price": 0,
     "patch": {"look": {"hero": [255, 214, 120]}}},
    {"id": "hero.berry", "slot": "hero", "name": "Berry", "price": 200,
     "requires": {"stat": "runs", "at_least": 5},          // unlock rule
     "patch": {"look": {"hero": [255, 120, 170]}},
     "preview": [[255, 120, 170]]}                         // optional swatch colours (≤ 4); default: colours found in the patch
  ],
  "daily": {"enabled": true, "seed_path": "course.seed",
            "streak_reward": {"base": 10, "per_day": 5, "max": 50}},
  "modifiers": [                  // ≤ 16; one is picked per UTC day
    {"id": "sprint", "name": {"en": "Sprint", "ja": "スプリント"}, "desc": {"en": "Gets hard sooner", "ja": "早く難しくなる"},
     "patch": {"course": {"ramp_bars": 24}}}
  ],
  "rewarded": {"slot": "double_coins"}   // must be a platform.ads.rewarded slot
}
```

* `slot`: `hero`, `look` or `music`. Each slot that has items needs one **default**: price 0, no
  `requires`. The default is owned and equipped from the start, and should equal the shipped spec
  (the Rust test checks this for HOP-BEAT).
* `requires.stat`: `runs`, `best_score`, `best_combo`, `best_streak`, `daily_played`. An item with a
  requirement is shown locked ("Play 5 runs") until the stat is reached, then can be bought for its
  price. Price 0 plus a requirement = granted when the stat is reached.
* Patches are JSON merge patches (objects merge, arrays and numbers replace). `null` (delete) is
  refused. A patch must be non-empty and every leaf must sit at or below a `patchable` path.
* `name` may be a plain string (used for both languages) or `{en, ja}`.
* Validation is `kagra.meta_spec.normalize_meta`; a bad block fails `load_manifest` with the key named.
  `meta.rewarded.slot` is cross-checked against `platform.ads.rewarded`.

The normalized block (defaults filled in, plus `version: 1`) is what ends up in `bundle.json`.

## Rules (identical in Python, Rust and JS)

* **Date**: UTC. `day = floor(unix_ms / 86400000)`, date string `YYYY-MM-DD` (UTC). Everyone on Earth
  has the same daily at the same moment; in Japan the daily changes at 09:00 JST.
* **Daily seed**: `FNV-1a-32("{game_id}:{YYYY-MM-DD}")`, written to `daily.seed_path` (a u32).
  HOP-BEAT's `course.seed` picks the phrase layout, so the layout is the same for every player.
* **Daily modifier**: `FNV-1a-32("{game_id}:{date}:mod") % modifiers.len()`.
* **Precedence** when the world is built (`meta.prepare(world)`): equipped items (hero, look, music) →
  the daily modifier (so a themed daily look overrides the equipped look for that day) → the daily seed.
* **Streak**: counted on the first *finished* daily run of a UTC day. The next day: +1. A missed day:
  back to 1. A clock that goes backwards changes nothing. Free play does not count. Reward on that
  first run: `min(max, base + per_day × (streak − 1))` coins. A second daily run the same day pays no
  streak bonus (it still earns run coins and can raise the day's best).
* **Run coins**: `floor(base + Σ per[k]·run[k])`, capped; a run with score 0 earns nothing; negative or
  non-finite stats count as 0. Keys are summed in sorted order so floats agree across languages.
  A Continue (revive) is the **same run**: only the increase over what was already paid is added, so a
  revive cannot be farmed.
* **Stale daily**: if the page stays open past UTC midnight, the loaded course is still yesterday's. That
  run earns coins but no daily credit, and the bar offers "Refresh".
* **Double coins**: optional button after a run, once per run, only if the host can offer the rewarded slot
  (`KagraPortal.canOfferRewarded`). Pays the run's coins again only after the ad finished. Retry/Start
  always work without it. No ad is ever forced by this layer. GameMonetize has no rewarded ads, so there
  the button never appears.

## The save

`localStorage` key `kagra.meta.<game_id>`, JSON, one object:

```json
{"v":1,"coins":0,"lifetime":0,
 "stats":{"runs":0,"best_score":0,"best_combo":0,"best_streak":0,"daily_played":0},
 "owned":{"hero.sun":true},"equipped":{"hero":"hero.sun","look":"look.neon"},
 "mode":"free",
 "daily":{"best_score":0,"best_combo":0,"last_day":null,"streak":0,
          "today":{"day":null,"score":0,"combo":0,"runs":0}}}
```

* **Tamper tolerant, never throws.** Every load goes through `sanitize`: wrong types become defaults,
  numbers are clamped to 0..1e9, unknown item ids are dropped, an equipped item you do not own reverts to
  the default, `__proto__` style keys are ignored. Unparseable text is kept once under
  `kagra.meta.<game_id>.bad` and the player starts fresh. A save is single-player and offline, so a player
  who edits it only cheats themselves; there is no checksum.
* **Versioned.** `v` is the schema (1). `migrate` steps old shapes up one version at a time (v0, no `v`:
  `owned` as a list of ids, is the first step). A **newer** `v` is read best-effort (known fields kept) and
  written back as `v:1`; do not rely on downgrade.
* **No storage** (private mode, blocked, full): the wallet works in memory for the session
  (`persistent()` is false) and nothing crashes.
* No export / import (by design).

## The screens (EN / JA strings live in `meta.js`)

The module injects its own DOM and CSS; it needs only `#overlay .card` (falls back to `#overlay`).

* **Bar** (on the title card and the result card, hidden while playing): coin badge · **Shop** · daily
  toggle ("Play today's daily" / "Back to free play"). In daily mode a line: date · streak · today's best
  (or all-time daily best) and "Today's twist: Sprint". On the result card: "+N coins · Streak bonus +M",
  **Double coins (watch an ad)** (only if offered), **Share result**.
* **Shop** (modal, `role=dialog`): tabs per slot (Hero / Look / Music), a row per item with colour swatch, name,
  and one button: Equipped · Equip · Buy N · "Need N more" (disabled) · Locked with the rule text ("Play 5
  runs"). Buying never reloads. Equipping marks the world dirty; closing the shop applies it with **one**
  reload of the page (default `location.reload()`), because skins are part of the spec the game is loaded
  with. Switching daily/free reloads at once.
* All controls are ≥ 44 px and stop pointer/key events so they cannot start a run by accident.
* **Share card**, copied with `navigator.clipboard` (falls back to a textarea copy, then shows the text):
  `HOP-BEAT Daily 2026-10-25 score 1234 combo 40` + newline + the game URL. JA: `HOP-BEAT デイリー 2026-10-25 スコア 1234 コンボ 40`.
  Free play: `HOP-BEAT score 1234 combo 40`. The URL is the page's own (origin + path, plus `?game=` when set);
  a `?daily=1` link opens straight into daily mode, `?daily=0` into free play.

## Page hooks (`scripts/preview_site/play.html`)

Four small, labelled blocks (`META hook n/4`). They are the only edits to existing files besides the manifest:

1. `<script src="./meta.js">` after `portal.js`.
2. After `portalHost.init()`: `metaLayer = KagraMeta.create({config: bundle.meta, gameId, title, url, host: portalHost, mockAds, mode})`.
   With no `bundle.meta` it is inert (`enabled:false`) and the page behaves as before.
3. End of the payload builder: `if (metaLayer && metaLayer.enabled) return JSON.stringify(metaLayer.prepare(world));`.
4. End of `showPhase(next)`: `if (metaLayer) metaLayer.phase(phase, sim);`.

If the page later wants to apply an equip without a page reload, pass `reload` to `create` (a function that
rebuilds the payload and calls `session.loadGame`); nothing else changes.

## What a new game needs

A `meta` block, the same four hooks (already in the shared page), `sim.result.run` with the stat keys the
formula names, and a default item per slot equal to its shipped look. To make an enemy mix or a BPM step a daily
modifier, add the spec path to `patchable` and a modifier that patches it (check the sim stays valid; the
Rust test in `kagra-shared/tests/meta_manifest.rs` is the template: apply every patch, `HopSpec::from_value`,
`course_problems()` empty, a year of days).

## Tests

* Rust: `game_core/meta.rs` unit tests (vectors); `kagra-shared/tests/meta_manifest.rs` (every unlock and
  modifier is a valid HOP-BEAT spec, skins never change the course, the daily layout is fixed and varies by day,
  an off-list patch changes nothing).
* Python: `tests/test_meta_spec.py` (validation, rules, bundle/packager), `tests/test_meta_js.py` + `tests/data/meta_harness.mjs`
  (JS under node: vectors, wallet, shop, daily/streak/rollover, share, double coins, 13 kinds of bad save).
* Not covered by automation: the real page on a phone (see the PR description's checklist).

## Not done / later

* Music unlocks need a second track in the kit (the `music` slot is accepted by the schema and the shop but HOP-BEAT
  ships one track). BPM stays fixed per track.
* Enemy-mix modifier: needs the enemy spec from the HOP-BEAT polish branch; add its paths to `patchable`.
* A stat for stomps works as soon as `result.run` carries it (add `"stomps": 2` to `earn.per`).
