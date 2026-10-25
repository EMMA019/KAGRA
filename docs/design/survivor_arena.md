# Survivor arena kit: design (Phase 1)

Status: design for Emma, written 2026-10-25 (JST). Build step 1 follows in the same PR.
Branch `nana/survivor-arena`. Nothing here costs money; every asset is original or CC0.

## 0. 日本語まとめ (Emma 向け)

- **何を作るか**: ヴァンサバ / Brotato 系の「サバイバーアリーナ」。1本指の「指に追従」操作、自動攻撃、敵の大群、
  経験値ジェム、レベルアップで3択、最後はボス（または時間切れクリア）、結果でコイン（#203 のメタ層）、デイリー。
  1ラン 3〜5分（既定 4分30秒、デイリーは3分）。
- **タイトル/テーマ案（3つ）**
  1. **BUNNY BLITZ（本命）** — HOP-BEAT のうさぎが月夜の屋上庭園をスライム・カブトムシ・コウモリの大群から守る。
     うさぎ・スライム・カブトムシ・コウモリのモデルは既にあるので、最初から見た目が揃う。武器は「にんじんダーツ」「ほしの輪」「もち爆弾」など。
  2. **MIDNIGHT PATROL** — Neon Cat が雨のネオン路地でロボットとドローンを撃退。ネオンの見た目は一番かっこいいが、敵のモデルは新規。
  3. **SLIME SWEEP** — ほうきの魔女見習い（またはお掃除ロボ）が、パステルのスライム祭りを掃除。いちばんゆるい。新規モデルが多い。
- **作り方の要点**: 共通エンジン（`game_core`）に「群れ」の部品（プール、空間ハッシュ、追跡、ディレクター、経験値/3択）を足し、
  ゲーム固有部分は JSON（敵6種、武器8種、パッシブ6種、ウェーブ曲線、ボス）。新作は JSON と見た目を差し替えるだけ。
- **Pixel 10 向け**: 同時に敵 200〜300 体（画質ティアで 120/220/320 に上限）、描画は最大約 640 インスタンス・十数メッシュ、
  フレーム内のメモリ確保ゼロ（固定プール）、空間ハッシュで当たり判定は O(N)。
- **公平さ**: ボットを走らせるバランス検査（放置・逃げ回り・拾い集め・ランダム選択）を CI に入れる。仕様読み込み時には静的な予算チェック。
- **更新 (2026-10-25 夕)**: Emma が CC0 素材の自由な利用を許可したので、ステップ1は HOP-BEAT のうさぎ/スライムではなく **Kenney (CC0) のモデル**で作った。
  タイトルは仮に **「Moonlit Swarm（月夜のむれ）」**: 月夜の墓地で、ミニキャラ（5スキン）がおばけとゾンビの大群を魔法の弾で撃退する。ジェムは Kenney のジュエル。
  音は OpenGameArt の CC0 レトロ SFX、Kenney のボタン音、OpenGameArt の CC0 ループ曲。採用した素材は台帳（`assets/LEDGER.json`）に1パック1エントリ。
  **Quaternius は使っていない**（ライセンス未確認、Emma 未承認）。フォント（OFL）はステップ1では未使用。詳細は下の「0b」。
- **4ステップ**: ①最小プレイ可能（今回）→ ②敵6種・武器・パッシブ・ディレクター・ボット検査 → ③ボス・ミッション・デイリー・メタの本実装・音 → ④Pixel 10 実測と最適化・ポータル梱包。各ステップ後にスマホで遊べるプレビュー。

## 0b. Phase 2, step 1 as built (what is in this PR)

Title (working): **Moonlit Swarm**. A mini-character guards a moonlit graveyard from ghosts and zombies for 2 minutes (step 1 run length; the
4:30 run comes with the director in step 2). Genre id `survivor_arena`, manifest `games/survivor_arena_minimal.json`, sim `kagra-shared/src/survivor_arena.rs`.

- **Controls**: one finger drags, the hero walks toward it (dead zone near the hero, full speed from about 1.5 m). A second finger is ignored. Keyboard: arrows/WASD.
- **Auto-attack**: weapon `bolt` fires at the nearest foe on its cooldown; four weapon levels (more bolts, damage, faster, pierce).
- **Foes**: one `EnemyType` (`ghoul`, `swarm` stats in JSON), drawn as a ghost (3 in 4) or a zombie (1 in 4). The director spawns on a curve
  (`start_rate` 1.5/s growing to `end_rate` 8.5/s over `ramp_secs`, squared curve, so the first 30 s are gentle and the last minute is a crowd) and
  foe hp rises with time. Pools are sized from the quality tier (120/220/320), the grid makes contact checks O(N), no allocation in the frame.
- **XP and level-up**: a kill drops a gem; the gem is pulled in inside the pickup radius. A full bar freezes the sim and shows 3 cards (bolt level or one of six
  passives: vitality, swift, magnet, might, haste, growth), one tap to pick, 0.5 s grace. The finger that picked a card does not steer afterwards.
- **Finish**: survive until the clock reaches `run.secs` (dawn, win bonus) or die (result). Score = kills, levels, time, win bonus.
- **Meta**: result `run` = {score, time, kills, level, win}; `meta.earn` pays coins; five hero skins in the shop (classic, mint, ember, ghost, gold; unlocks by runs or best score),
  daily seed (`director.seed`) and three modifiers, rewarded "double coins". All through the #203 meta layer, no new page code for it.
- **Balance check (unit tests, deterministic)**: idle dies; a kiting bot that prefers weapon cards wins some seeds and loses others; level-up cards show up within ~10 s.
  These are the first bots; the full baseline harness is step 2.
- **Page**: `play.html` has a `survivor_arena` branch: fixed 60 Hz stepping, pause on hidden tab / Esc, column labels. No graphics-settings code is touched.

### Assets adopted in step 1 (all CC0; one ledger entry per pack; files copied only if used)

| Use | Pack (ledger id) | Files | Change made |
|---|---|---|---|
| Hero, 5 skins | Kenney Mini Characters (`kenney-mini-characters`) | `character-female-a/male-c/male-a/female-d/male-e.glb` | Rest-pose copy: animations/skin/joints dropped, colormap embedded (`tools/gen_assets/pack_glb.py`), 82-103 KB each |
| Foes | Kenney Graveyard Kit (`kenney-graveyard-kit`) | `character-ghost.glb`, `character-zombie.glb` | Same rest-pose packing; bob/squash/float is done in code |
| Arena dressing | Kenney Graveyard Kit (same entry) | 3 gravestones, 2 pines, 2 pumpkins, lantern, cross, rocks | Same packing |
| XP gem | Kenney Platformer Kit (`kenney-platformer-kit`, existing) | `jewel.glb` | Same packing |
| SFX | OpenGameArt "50 CC0 retro / synth SFX" (`oga-50-cc0-retro-synth-sfx`) | shot, explosion, die x2, coin, power-up x2 | ogg to mono 22.05 kHz wav (Safari-safe) |
| Pick chime | Kenney Interface Sounds (`kenney-interface-sounds`) | `confirmation_001` | ogg to wav |
| Music | OpenGameArt "Flowerbed Fields (loop)" (`oga-flowerbed-fields-loop`) | one loop | ogg to 96 kbit/s mp3 |

### Picked for later steps, not adopted yet

- More foes: Kenney Cube Pets (already in the ledger), KayKit Skeletons (clips stripped to about 8) for an elite. Not in step 1: one foe type is the step.
- Weapons: Kenney Blaster Kit. Step 1's shot is an emissive sphere (a particle sprite pass needs the VFX path, which is step 2).
- Arena floor: Mini Arena / Prototype / Nature Kit. The floor is still a procedural plane. **Nature Kit and Blocky Characters use
  `KHR_materials_unlit`, which the renderer ignores (they would render lit)**; check before relying on them. `KHR_texture_transform` is supported.
- VFX: Kenney Particle / Splat packs. UI: Game Icons, Mobile Controls (the HUD is drawn in the canvas today).
- Fonts: Zen Maru Gothic and Noto Sans JP (OFL 1.1, not CC0). Step 1 uses the existing in-canvas font. When they are bundled they go in as
  a separate ledger class with the OFL text next to the file (like `font-publicsans`).
- **Not used, on purpose: anything from Quaternius** (license status not confirmed, Emma has not approved). No purchases were made.

## 1. Working title and theme

| | Idea | Why | Cost |
|---|---|---|---|
| A (recommended) | **BUNNY BLITZ**: the HOP-BEAT bunny guards a moonlit rooftop garden from a swarm of slimes, beetles and bats | Same original-art family as HOP-BEAT (neon-night / dusk looks, the bunny, slime, beetle and bat GLBs already in `assets/models/hop_beat/`), so step 1 already looks like one world. The look presets and skins from #202/#203 carry over. | Lowest: 4 of the 7 creatures exist |
| B | **MIDNIGHT PATROL**: Neon Cat on a rainy neon alley against drones and rats | The best-looking night scene we have (Neon Cat's atmosphere). | Enemy models all new |
| C | **SLIME SWEEP**: a broom-riding apprentice sweeps up a pastel slime festival | Softest, most "cute" pitch, good for ads and thumbnails. | Hero and most enemies new |

The kit is genre-neutral (`GameKind` / genre `survivor_arena`); the theme is only JSON plus models. Step 1 ships as A with a working
title; renaming it is a one-line manifest change (`title`).

## 2. Core loop (4:30)

1. Hero stands in a walled arena (a disc, radius 14 m). One finger drags; the hero walks toward the finger. Everything else is automatic.
2. Weapons fire on their own cooldowns at the nearest enemy (or in their own pattern). Enemies stream in from outside the view and chase.
3. A kill drops an XP gem. Walking near it pulls it in (pickup radius, upgradable). A full XP bar is a **level up**: the sim
   pauses and shows **3 cards, pick 1** (new weapon, weapon level, passive). Pause time does not count toward the run clock.
4. The director raises the pressure on a curve: more types, more at once, a formation burst every 30 s.
5. At 3:30 a **boss** walks in. Kill it, or survive until 4:30 (the dawn finale clears the field): the run is a **win**.
   HP 0 is a loss (one optional Continue from a rewarded ad, same pattern as HOP-BEAT).
6. Result card: time, kills, level, boss, score, **coins** from the meta layer, then Retry / Shop / Daily.

Target feel: first level-up by about 0:12, 12 to 16 level-ups in a winning run, the first real danger around 1:30, a screen full of enemies by 3:00.

## 3. Controls (one thumb)

- **Follow (default)**: while a finger is down, the hero moves toward the point on the ground under the finger. Speed ramps with distance
  (`input.dead_zone` m: no move when the finger is on the hero; `input.full_speed_dist` m: full speed). Lift to stop. A small ring marks
  the target. The pointer is turned into a ground point by the camera ray (kit side, no page code).
- **Stick** (setting): the first touch is an anchor, the drag direction is the move. Same `touch()` events, a flag in the spec.
- Desktop: WASD / arrows. Pause: a corner button and Esc (the HOP-BEAT page code already pauses a hidden tab).
- No buttons on the field. Level-up cards are big taps (full-width, 56 dp+).
- Occlusion: the thumb covers the hero on a phone, so the ring and a hero outline (silhouette pass when behind a swarm) are drawn.

## 4. Enemies (six types plus a boss, from `game_core::enemies`)

`game_core::enemies` today has `EnemyType`/`EnemySpec` (id, look, colour, hurt band, step-based walk, stomp), `weighted_pick`, `Slot`
and the sweep. A swarm needs more fields and a different clock (seconds, free movement, hit points). Plan:

- Keep the module, extend it: `EnemyType` gains optional swarm fields (`hp`, `speed`, `radius`, `touch_dmg`, `xp`, `unlock_secs`, `weight`,
  `behavior` and its parameters). They default to values that leave HOP-BEAT's JSON and behavior byte-identical (`deny_unknown_fields`
  stays; HOP-BEAT never sets them).
- A new genre-neutral `game_core::swarm` holds what has no HOP-BEAT meaning: the fixed-capacity pool (structure of arrays), the spatial
  hash, steering (chase, separate, charge, orbit, keep-distance), the per-type behavior update, and a `Director`.
- `enemies::weighted_pick` picks the type; `unlocked(level)` is reused with `level = floor(t / unlock_stage_secs)`.

| id | look (model) | behavior | hp | speed m/s | unlock | role |
|---|---|---|---|---|---|---|
| slime | hop_slime (exists) | chase | 6 | 1.6 | 0:00 | filler, the carpet |
| beetle | hop_beetle (exists) | chase fast, armoured (flat `armor` 2 off every hit) | 14 | 2.4 | 0:45 | soaks weak weapons |
| bat | hop_bat (exists) | sine-weave chase, ignores slows | 5 | 3.4 | 1:15 | arrives in a V formation, punishes standing still |
| hopper | new: toad-like (primitives+AO) | **charge**: stops, flashes 0.5 s, dashes in a line | 18 | 1.2 / 7.0 dash | 1:45 | teaches reading telegraphs |
| lantern | new: floating lantern ghost | **keep distance** and spit a slow orb every 2.5 s | 10 | 1.4 | 2:15 | forces movement, not just kiting |
| brute | new: big mochi golem | slow chase, big radius, **splits** into 3 slimes on death | 80 | 0.9 | 2:45 | tanky payoff for area weapons |
| boss: king_slime (or moon_beetle) | scaled slime + crown | phases: summon ring, charge, orb spiral | 1500 | 1.1 | 3:30 | the finale |

All numbers live in JSON and are tuned by the bots (section 12). Touch damage is per contact with a short per-enemy cooldown (0.6 s) so
a swarm cannot melt the hero in one frame; the hero also has `invuln_secs` (0.4 s) after a hit, shown as a blink (`game_core::lives::Lives`
blink phase is reused).

Adding a creature is JSON plus (optionally) a model; an unknown `look` falls back to a primitive (the same rule as HOP-BEAT).

## 5. Weapons (8) and passives (6): JSON

```jsonc
"weapons": [
  {"id": "carrot",   "name": {"en": "Carrot Dart", "ja": "にんじんダーツ"}, "kind": "projectile", "target": "nearest",
   "cooldown": 0.65, "damage": 6, "speed": 13, "pierce": 1, "count": 1, "range": 9, "max_level": 5,
   "levels": [{"count": 2}, {"damage": 3}, {"pierce": 1}, {"cooldown": -0.15}]},
  {"id": "orbit",    "kind": "orbit",     "count": 2, "radius": 1.7, "spin": 3.2, "damage": 4, "tick": 0.35, ...},
  {"id": "mochi",    "kind": "lob",       "target": "densest", "aoe": 2.2, "damage": 14, "cooldown": 2.4, ...},
  {"id": "thump",    "kind": "pulse",     "radius": 2.6, "damage": 9, "cooldown": 2.0, "knockback": 1.2, ...},
  {"id": "zap",      "kind": "chain",     "targets": 3, "damage": 7, "cooldown": 1.3, "jump_range": 3.5, ...},
  {"id": "petals",   "kind": "cone",      "arc": 70, "range": 4.5, "count": 5, "damage": 5, "cooldown": 1.0, "aim": "move_dir", ...},
  {"id": "moonbeam", "kind": "sweep",     "length": 7, "width": 0.6, "spin": 1.1, "damage": 3, "tick": 0.2, ...},
  {"id": "garden",   "kind": "trail",     "lifetime": 3.0, "radius": 0.9, "damage": 2, "tick": 0.4, "drop_every": 0.35, ...}
],
"passives": [
  {"id": "vitality", "stat": "max_hp",       "per_level": 0.20, "mode": "mul", "max_level": 5},
  {"id": "swift",    "stat": "move_speed",   "per_level": 0.08, "mode": "mul", "max_level": 5},
  {"id": "magnet",   "stat": "pickup_radius","per_level": 0.30, "mode": "mul", "max_level": 5},
  {"id": "might",    "stat": "damage",       "per_level": 0.10, "mode": "mul", "max_level": 5},
  {"id": "haste",    "stat": "cooldown",     "per_level": -0.08,"mode": "mul", "max_level": 5},
  {"id": "growth",   "stat": "xp_gain",      "per_level": 0.10, "mode": "mul", "max_level": 5}
]
```

Weapon `kind` is a closed list implemented once in the engine (projectile, orbit, lob, pulse, chain, cone, sweep, trail); a new weapon is
a new row. A level row overrides or adds (`+`) fields. Unknown keys fail the load. Slots: 4 weapons, 4 passives (JSON). Evolutions
(weapon L5 + its passive) are a stretch item in step 3, not required.

## 6. Upgrade pick rules

- On level up the sim freezes and offers **3 distinct cards** drawn from: new weapon (if a slot is free), +1 level of an owned weapon
  (below max), a new passive (if a slot is free), +1 level of an owned passive. Fallback cards (heal 30%, +coins) fill the list when the pool is empty.
- Weights: new weapon 3, weapon level 4, new passive 2, passive level 3; **pity**: if the last two offers had no weapon, one slot is forced
  to a weapon card; the first level-up always offers the starting weapon family so a run never opens on three passives.
- The draw uses its own seeded RNG stream (`seed ^ level`), so a restart cannot reroll a bad offer and the daily is fair; a **reroll**
  (1 per run, rewarded-ad option later) is the only way to redraw.
- Cards show name, level pips and a one-line effect in JA/EN from the JSON. A tap on a card picks it; the sim resumes after 0.5 s of invulnerability.

## 7. Director: waves and the curve

`Director` (genre-neutral, in `game_core::swarm`) spends a **budget** each second:

`budget(t) = b0 + b1 * t + b2 * t^2` (points per second), each type costs `cost = hp * speed_factor` points. It picks among unlocked types by
weight (`weighted_pick`), spawns on a ring just outside the camera (never inside a safe radius of the hero), and stops at the live cap
(`caps.live` by quality tier). Extras: a **formation burst** every 30 s (ring, line, or V for bats) with a one-second warning marker on the
ground; a **breather** (budget x 0.4 for 8 s) after each boss or mini event; a **catch-up** rule that adds budget when kills per second fall far
behind the plan (so a weak build is not buried and a strong one is not bored).

Time plan: 0:00 slime carpet; 0:45 beetles; 1:15 bats and the first burst; 1:45 hoppers; 2:15 lanterns; 2:45 brutes; 3:30 boss; 4:30 dawn.
The boss fight: it must be killable by a typical level-12 build in under 40 s (bot-checked), and the dawn timer kills the remaining field
(win), so a finale is always reachable.

## 8. How it reuses `game_core`

| Part | Used how |
|---|---|
| `Kit` + `tick_frame` | The sim step; fixed 1/60 s, up to 3 substeps per frame. |
| `Core` (stats, juice, lives, missions, result) | `stats.score` (score = kills x tier + time bonus), `juice` (hitstop on boss, shake on hit, sparks on kill), `result` (game-over cue, banner, BEST chip). |
| `lives` | Hero HP is the kit's own bar. `Lives` supplies the hit blink and the one **Continue** (`revives`, `can_revive`, the rewarded `revive` slot) exactly as HOP-BEAT does. |
| `missions` | New generic `MissionKind`s: `kills`, `level`, `survive_secs`, `boss`, `gems`. `Counters` is implemented by the kit; the strip and the result list come for free. Step 3. |
| `result` and the page | `sim.result` = `{key, title, run: {score, kills, level, secs, boss, win}, can_revive, new_best, missions}` so `meta.js` pays coins from the same fields and the page's game-over card and local board work unchanged. |
| `enemies` | Section 4: extended, HOP-BEAT unchanged. |
| `items` (#201, still a draft) | It holds the runner's timed pick-up effects (magnet, boost, jump, double). The arena's drops (magnet pulse, heal, bomb, gold) need only a weighted draw and a clock, about 40 lines. **Not a dependency**: step 3 reuses `items::Effects` only if #201 has merged by then. |
| `feel` / `juice` | Hitstop, shake, spring pops (level-up card pop, gem collect). |

## 9. Meta layer (#203): coins, unlocks, daily

A `meta` block in `games/survivor_arena_minimal.json`; `meta.js` and the hooks already in `play.html` do the rest (no new page code, no sim change).

- **Earn**: `coins = floor(base + sum(per[k] * run[k]))` via `earn.per` over `sim.result.run` fields: `kills`, `level`, `secs`, `boss` (1 on a boss kill), `win` (1). Capped per run.
- **Heroes** (slot `hero`): skins now (`hero.skin`), later heroes with different base stats and a starting weapon (`hero.start_weapon`, `hero.max_hp`).
- **Starting upgrades**: a new catalogue slot `start` (one equipped: +1 starting level of a weapon, +10% max HP, a free reroll). This is the one meta extension:
  `SLOTS` in `kagra/meta_spec.py`, the Rust mirror and `meta.js` (shop tab) each gain one word, covered by the shared vectors. A merge-patch on
  `hero.*` and `levels.*` paths in `patchable` (never `director`, `enemies`, `weapons`, `audio`).
- **Look** slot: the `look.preset` unlocks from HOP-BEAT (dusk, neon_night, plain).
- **Daily**: `seed_path: "director.seed"`: one fixed spawn schedule, boss, and upgrade offers per UTC date; its own best; streak reward; modifiers
  (`horde`: +30% budget; `glass`: hero HP x0.6 with +50% coins; `swift`: enemy speed +10%) patch `director.*` and `hero.*` (all whitelisted).
- Rewarded slots: `revive` (one Continue) and `double_coins`, as in HOP-BEAT. Interstitial placement follows `platform.ads.min_interval_secs`.
- Fairness rule: a meta patch may make a run **easier or more rewarding**, never change the enemy roster, the boss or the clock; the bots run each
  unlock and each modifier (section 12).

## 10. Performance budget (Pixel 10: Tensor G5, PowerVR GPU, 60 Hz)

Targets (to be measured on the phone with the `?dev=1` overlay and Record 60 s from #204): p95 frame <= 20 ms, no hitch over 50 ms, 10 min
without thermal drop. The browser path is WebGL2 on the PowerVR (and WebGPU where present).

| Budget | Value | How |
|---|---|---|
| Live enemies | 120 (`low`), 220 (`mobile`, default), 320 (`high`) | `caps.live`; the page's `quality` tier picks it (read-only; the graphics-settings code is not touched) |
| Instances per frame | <= 640: enemies 320, gems 160, projectiles 96, fx 64 | One mesh per kind, so about 14 batches |
| Per-frame allocation | **zero** in the sim and in the doc build | Fixed `Vec` capacity at load; the doc keeps a persistent prop pool whose strings are set once; only `position`, `yaw`, `scale` and `color` change |
| Collision | O(N) | Uniform grid, cell 1.0 m, counting sort into reused arrays (`cell_start`, `order`); queries: 3x3 cells for separation, ring search for nearest, cell range for area damage |
| Separation | one pass, <= 8 neighbours | Push-apart accumulated per enemy, capped |
| Sim cost | <= 3 ms at 320 enemies (native test asserts a loose ceiling) | Structure of arrays, no `f32::sin` per enemy per frame except bats (a table) |
| Shadows | hero only (one soft blob); enemies cast none | `casts_shadow` off for the pool |
| Text | HUD texts <= 24; damage numbers **off** by default | `TextQuad` is the expensive path on mobile; hit feedback is colour flash and size pop |
| Gems | merge when over 160: the oldest two become one big gem | Keeps the cap without losing XP |
| Doc clone | `SharedSession::render` clones the `WorldDoc` each frame | Fine below about 300 props; step 4 changes it to a borrow (a 5-line change, measured first). |

## 11. Art plan

- **Step 1 (built; supersedes the original plan of reusing the HOP-BEAT models)**: Kenney CC0 models, see section 0b. Ground and walls are primitives, shots are emissive spheres.
  Five ledger entries (one per pack) were added.
- **Steps 2-3**: the three new creatures (hopper, lantern, brute) and the boss crown, the arena props (flower beds, lamp posts), made by a new
  `tools/gen_assets/survivor_models.py` with the same recipe as `hop_models.py`: flat-coloured primitives, **baked AO** in `COLOR_0`,
  deterministic bytes, `--check` in tests, a ledger entry per pack (`project-...`, CC0, "written by this script").
- Look: the HOP-BEAT looks (`neon_night`, `dusk`, `plain`) and finishing (bloom, grade, vignette) as they are; pastel gems and hit flashes read well on the dark ground.
- Readability rules: enemies are always brighter than the ground; projectiles are emissive; the hero has a rim light and a thin outline when covered.
- All art original or CC0. Nothing bought, nothing fetched from outside.

## 12. Fairness and balance checker (bots)

Same idea as HOP-BEAT's `course_problems` plus its measurement harness:

- **On load (static)**: spec validation (`deny_unknown_fields`, ranges), roster budget (cost curve under the live cap, total enemy hp spawned per second
  against a reference DPS), spawn ring outside the view, safe radius, boss reachable (hp / reference DPS < time left), card pool never empty.
- **Headless bots (CI, deterministic seeds)**, `survivor_arena_measure.rs` writing `docs/measure/survivor_arena_baseline.json`:
  - `idle`: stands still. Must die before 2:00 (the game is not a screensaver).
  - `kite`: runs away from the local enemy centroid. Median survival must reach at least 2:30 with the starting weapon only, and a
    kiter must not be fully encircled in the first 90 s (checked by an escape-path test on the spawn schedule).
  - `collector`: kites and detours to gems. Must reach level 10 by 3:30 and win in at least half of the seeds with the "greedy" picker.
  - Pickers: `first`, `random`, `greedy` (highest DPS card) and `defensive`. All of them must be able to win some seeds, and `greedy` must not
    win 100% (the curve is not flat).
  - Metrics: survival time, kills, level at 1:00/2:00/3:00, damage taken per second, peak live count, sim ms per tick.
  - Each meta unlock and each daily modifier is run against the baseline: an unlock never lowers win rate, a modifier stays inside a band.
- A change to the spec that moves a baseline needs a regenerated JSON in the PR (the same rule as HOP-BEAT).

## 13. Audio plan

- **SFX**: synthesized by `tools/gen_assets/survivor_audio.py` on the `cat_audio.py` oscillators (CC0, seeded, `--check`). Cues: `shoot`, `hit`, `kill`,
  `gem`, `levelup`, `hurt`, `boss`, `win`, `gameover`; rate-limited per cue per frame (a swarm must not machine-gun). Step 1 (built) uses the CC0 OpenGameArt/Kenney sounds and a CC0 loop instead (section 0b); the procedural plan stays for later steps.
- **Music**: `/workspace/music` (the procedural kit: `kagra_music/tracks/*.py`, `render_all.py`, `verify.py`, `seam_test.py`, `make_provenance.py`).
  A new track "game N" copied from `hop_beat.py`: 105 or 120 BPM (integer samples per beat), 3 sections that map to the director's stages (intro, build, boss),
  plus stingers (win, gameover). Loop is gapless mp3, ledger entry from `make_provenance.py`. No samples, no ML.
- The page's audio code (cue names from the sim, gesture unlock, mute button) is reused as is.

## 14. Shared engine vs game-specific

| Shared (new or extended in `game_core`) | Game-specific |
|---|---|
| `swarm`: pool, spatial hash, steering, behaviors, `Director` | `survivor_arena.rs`: spec, hero, weapons runtime, doc build, HUD, level-up cards |
| `enemies` extended fields (HOP-BEAT unchanged) | `games/survivor_arena_minimal.json`: roster, weapons, passives, curve, boss, look |
| `progression`: XP curve and the seeded 3-card draw | Models, SFX, music, copy |
| `missions` generic kinds, `result`, `lives`, `meta` | Balance baselines |
| Page: `survivor_arena` genre branch in `play.html` (best score key, audio spec, swipe-mode card) | |

A second game of the same kind is a new manifest plus models: no Rust.

## 15. Effort, risks, build order

Effort (one agent working sessions, each ends in a playable preview): step 1 about 1, step 2 about 2 to 3, step 3 about 3, step 4 about 2.

Risks and mitigations:

| Risk | Mitigation |
|---|---|
| Frame time with hundreds of enemies on PowerVR | Caps, pools, one batch per mesh, measured on the phone after steps 1 and 2 (not after step 4 only) |
| Per-frame `WorldDoc` clone and `String` props | Persistent prop pool now; borrow instead of clone in step 4 if the overlay shows it |
| Thumb hides the hero | Target ring, outline when covered, optional stick mode |
| Balance (too easy / too hard / one dominant weapon) | Bots + baselines in CI; weights in JSON, not code |
| Level-up pause feels like a stall | Cards pop in 150 ms, 0.5 s grace after the pick, one-tap choose |
| Daily determinism across devices | Fixed-step f32 sim, own seeded RNG streams, no `HashMap` iteration order, same wasm everywhere |
| Page conflicts with the quality-tiers work | The kit reads `quality` like HOP-BEAT does; `play.html` gets only a genre branch; no graphics-setting code is touched |
| Ads policy / revenue | Only rewarded Continue and double coins; the existing `platform.ads` block and interval rule |
| Scope creep | Each step is playable and mergeable alone; evolutions, banish and more heroes are step 3 stretch |

**Build order** (a playable preview after each):

1. **Minimal playable** (this PR): arena, follow-the-finger hero, auto-attack with one weapon (carrot), one enemy (slime) in growing numbers, XP gems,
   level-up with 3 upgrade cards, death/result, coins through the meta layer, JSON spec, Rust sim in `kagra-shared` (`KitHost`), page support, tests and a headless bot.
2. **Content and director**: the six enemy types, 8 weapons, 6 passives, the director curve with bursts, merged gems, pickups, hit feel, **bots and baselines in CI**.
3. **Finale and meta**: boss and dawn finish, missions, daily and modifiers, the `start` slot and heroes, new models, SFX and music, Continue ad.
4. **Phone pass and release**: Pixel 10 measurement and optimization (borrowed doc, instance batches, caps), portal packaging, balance tuning from baselines, store/portal copy.

## 16. Quality roadmap (polish pass, PR #209)

North star: Switch-class look and feel (stylised, not realistic). Everything expensive is JSON-switchable and tied to the picture tier
(`low` / `mobile` / `high`); the mobile default stays Pixel-10-friendly (capped pools, persistent buffers, no per-frame allocation).
The `fx` block of the spec (`particles`, `shake`, `damage_numbers`, `knockback`, `zoom_pulse`, `trails`, `high_post`, `cap[3]`) switches each effect;
effects use their own RNG stream, so toggling them never changes a run (tested).

| Area | Done | Gap to Switch-class | Tier |
|---|---|---|---|
| Aim / weapon | Blaster Kit model points along the shot; hero faces the target; shots spawn at the muzzle (tested) | Aim-line anticipation, recoil | all |
| Arena | Mini Arena walls and columns, seamless paver floor, stray squares removed | Props and set dressing, animated torches, a second biome | all |
| Lighting / grade | Warm hero lantern, cool moon key, calmer grade, tinted foes; high tier adds wider soft shadows, stronger bloom/SSAO (+TAA, cascades, PCSS from the tier) | Baked-looking GI, rim light, colour-grade LUT per phase | high |
| VFX | Pooled sparks, death puffs, gem glints, muzzle flash, hurt burst, shot streaks, level-up ring | Flipbook sprites (Kenney Particle/Splat), screen-space distortion, ground decals | caps 48 / 160 / 480 |
| Camera | Shake and zoom pulses on hit, kill, hurt, level-up | Dolly on level-up, hit-stop, framing for boss | all |
| Characters | Kenney Mini Characters (baked pose), hit flash, knockback | Skeletal animation (run, hit, death), squash and stretch, anticipation, more enemy kinds | all / high |
| HUD / UI | HP bar with sheen, XP bar, level pill, kills, clock pill, dawn bar, low-HP pulse; level-up cards with glyph badges and level pips | Real icon art (HUD image quads do not draw in the browser yet), rounded OFL font (the font is embedded in `font.rs`, shared engine), animated card entrance | all |
| Audio | Kenney impact hit, wet-squish pop, gem/level/pick cues; BGM loop checked for clicks | Layered mix with ducking, pitch variation per cue, adaptive music intensity, ear-checked loop | all |

Order by visible impact per effort: animated characters (squash/anticipation), flipbook VFX, icon art in the browser HUD, font swap, mix and ducking, extra enemy kinds.

## 17. Manual target selection (PR #209, step 5)

`input.aim` in the JSON spec: `auto` (nearest foe, as before), `manual` or `hybrid` (default). `input.tap_px` is the pick radius (44 px at 400 px short side).
One thumb keeps steering (drag-follow). The other thumb taps:

- **a foe** locks it: a red ring on the floor and a bobbing red arrow over it; every weapon fires at it while it is in range (out of range falls back to nearest); tap it again to release;
- **bare ground** aims that way (a gold line from the hero); a tap on the hero clears it;
- a quick tap of the lone steering finger (under 0.25 s and 12 px) counts as a tap too, for one-thumb play.

Fallbacks: a dead lock drops to the nearest foe. `hybrid` is lock, then nearest, then the ground aim when nothing is in range. `manual` is lock, then the ground aim (even with foes near), then nearest,
so nobody is stuck without a target. `auto` ignores taps. The hero faces and the muzzle sits on the shot line for every choice. The pause card has an AIM button that cycles auto / tap / hybrid (kept until the page reloads; it is not stored on the device).
The lock is one uid and an index hint, the ring and arrow reuse two persistent lines and one HUD text, so no pools grow and nothing allocates per frame.

## 18. Drops, weapon slots and depth (PR "survivor-drops", step 2a)

日本語: 敵がたおれるとたまにぶき・回復・ボムなどを落とす。ぶきは3スロット、にたぶきは強化、いっぱいのときは入れかえカード。エリートはかならず宝箱を落とす。ぶきとパッシブの組み合わせ(シナジー)でさらに強くなる。ぜんぶ JSON で変えられる。

Goal: a run that has decisions in it. Level-ups stay the steady spine; drops are the surprise on top; weapon slots and synergies make "what do I keep" matter.
Everything below is JSON (`SurvivorSpec`), validated, deterministic for a seed; the defaults ship in `SurvivorSpec::default()`.

### 18.1 Drops

```jsonc
"drops": {
  "enabled": true, "cap": 16, "life": 16.0, "blink": 4.0, "pull_m": 2.4, "pity_kills": 24,
  "items": [   // one row per pick-up: kind, weight, rarity 0 common / 1 rare / 2 epic
    {"id": "heal",   "kind": "heal",   "rarity": 0, "weight": 30, "amount": 0.30},       // share of max HP
    {"id": "magnet", "kind": "magnet", "rarity": 0, "weight": 24},                       // every gem flies to the hero
    {"id": "xp",     "kind": "xp",     "rarity": 0, "weight": 18, "amount": 4.0},        // an XP burst
    {"id": "bomb",   "kind": "bomb",   "rarity": 1, "weight": 14, "amount": 14.0},       // damage to everything near the screen
    {"id": "shield", "kind": "shield", "rarity": 1, "weight": 14, "secs": 6.0},          // no damage for a while
    {"id": "frenzy", "kind": "frenzy", "rarity": 1, "weight": 14, "secs": 8.0},          // faster casts and legs
    {"id": "weapon", "kind": "weapon", "rarity": 2, "weight": 16}                        // grants or upgrades a weapon (see 18.2)
  ],
  "tables": [  // what a kind of foe rolls on death
    {"id": "common", "chance": 0.07, "rolls": 1, "rarity_min": 0},
    {"id": "elite",  "chance": 1.00, "rolls": 2, "rarity_min": 1, "chest": true}         // a chest, guaranteed
  ]
}
```

- Each foe type names a table (`foes[].table`) and a `drop_mul` (a tank drops more often than a runner). Elites use `elite`: a chest that opens on touch with `rolls` rewards of at least `rarity_min`, so elite and boss drops are better, never worse.
- `pity_kills`: after that many kills without a drop the next kill drops one (a long dry run cannot happen).
- A pick-up bobs and glows, flies to the hero inside `pull_m` (the Magnet passive widens it), blinks for the last `blink` seconds and then goes. Pool `cap` is fixed (no allocation in a step); a full pool drops nothing new.
- Draws use the sim RNG (deterministic per seed, so the daily challenge gives everybody the same drops); effects keep their own RNG.

### 18.2 Weapons and slots

`WeaponKind` grows from one to six: `projectile` (the moon bolt), plus four-to-five new ones with their own feel. All numbers stay in `base` and `levels[]`; `area` is the new radius/range field.

| id | kind | feel | key numbers |
|---|---|---|---|
| bolt | projectile | steady auto-aim stream | as before |
| petals | spread | shotgun: a fan of short pellets, strong up close, weak against armour | `count` pellets over `spread_deg`, short `range` |
| blades | orbit | crescents circle the hero and shred whatever touches them; no aim needed | `count`, `area` = orbit radius, `speed` = rad/s, `cooldown` = per-foe hit gap |
| wisps | missile | launched sideways then home in; explode in an area, ignore armour | `count`, `area` = blast radius |
| zap | chain | instant lightning that jumps from foe to foe | `count` = targets, `area` = jump range, damage falls 20% per jump |
| pulse | pulse | a ring around the hero that shoves the crowd back | `area` = radius |

- **Slots**: `levels.max_weapons` (default 3, 1..=4). A level-up card can offer a new weapon while a slot is free, or +1 level of an owned one.
- **Pick-up rules**: a weapon pick-up of an owned weapon is +1 level (a chest does the same for the best owned weapon); an unowned one fills a free slot; with the slots full it opens a **swap** card set: replace one of your weapons (the new one starts at level 1 and keeps no levels, the old one is gone), or keep what you have (the pick-up turns into XP).
- **Evolve / merge** (step 2b): a weapon at max level with its synergy passive gets the evolved form (`levels[last]` stats x1.5, new FX). Not in 2a; synergies below are the first half of it.

### 18.3 Temporary power-ups

Kinds `magnet`, `bomb`, `heal`, `shield`, `frenzy` (and `xp`). `shield` and `frenzy` are timed (a small clock per effect, like `items::Effects` in #201: restarting refreshes, it does not stack); the HUD shows a chip with its time left. `bomb` and `magnet` are instant. #201 (items-core, draft) holds the runner's effect vocabulary (`ItemEffect`: meow, boost, superjump, double); the arena's effects are different, so 2a keeps its own 20 lines and does not depend on #201. When #201 merges, the clock and `weighted` draw can move over.

### 18.4 Enemies

The five roles live on `game_core::enemies` types (`swarm` stats: hp, speed, radius, touch damage, xp, unlock second, weight) plus a survivor-only `foes[]` row per type id for what a swarm stat cannot say:

| id | role | behaviour (`foes[].ai`) | counter |
|---|---|---|---|
| ghoul | filler (the carpet) | `chase` | anything |
| runner | fast, brittle, arrives in packs | `chase` with a side weave | area weapons, orbit |
| tank | slow, big, armoured (`armor` flat per hit, `knock_resist`) | `chase` | missiles, pulse, high-damage hits; petals struggle |
| shooter | keeps its distance and lobs a slow orb every `shoot_every` s (telegraphed flash) | `shoot` | close the gap, chain, wisps |
| warden (elite) | scheduled (`director.elite_first`, `elite_every`), big, many hp | `chase` | everything; drops a chest |

Orbs the shooters throw live in a fixed pool and hurt like a touch; the hero's shield and hurt cooldown apply.

### 18.5 Synergies (runs get a shape)

```jsonc
"synergies": [ {"weapon": "bolt", "passive": "haste", "min_level": 3, "name": {"en":"Rapid moon","ja":"はやづき"},
                "damage": 0.0, "cooldown": -0.15, "count": 0, "pierce": 1, "area": 0.0} ]
```

A weapon at `min_level` or above plus any level of the named passive adds the row's bonus (`damage`/`cooldown`/`area` are fractions, `count`/`pierce` are whole numbers). The slot chip shows a ✦ while one is active. Rows are data, so a weapon x passive grid can grow without code. Evolve (18.2) builds on it.

### 18.6 Meta layer (wired lightly in 2a)

- Weapon unlocks: heroes in the existing `hero` catalog slot start with a different `hero.start_weapon` (path whitelisted in `meta.patchable`); coins unlock them. No new slot, no new page code.
- Weapon codex: a "seen" list per weapon id in the profile and a codex tab in the shop page. **Step 2b** (needs a page change; kept out so #207's `play.html` edits do not conflict).
- Meta patches may make a run easier or richer, never change a table's rarity floor or the roster.

### 18.7 Performance (Pixel 10 rules still hold)

Fixed pools sized once: pick-ups (`drops.cap`), enemy orbs (24), missiles share the shot pool (`caps.shots`), blades (6), lightning segments (24), foe props per model. Per-foe extras (orbit hit gap, shooter clock, dash state) sit in one parallel array that swaps with `Foes::remove`. The per-frame work added: pick-ups (<=16), blades x grid query, missile homing (<=12 nearest queries), no new allocation in a step. Bots (section 12) check the run is still winnable with each weapon alone and that a drop-heavy build cannot trivialise the clock.

### 18.8 Gap list (what is not done after 2a)

Evolutions, codex page, boss (the king) and its table, pick-up icon art (glyphs are used because browser HUD image quads are not drawn), a "weapons unlocked" gate for the drop draw, per-weapon unique models (KayKit weapons), more enemy kinds (skeleton mage, bat swarm), and ear-checking the new SFX mix.
