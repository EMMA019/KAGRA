# Changelog

## Unreleased

- **Station Escape, round 3: the Warden, stage select and records.** Stage 5, Reactor Core: a boss that hovers and fires aimed bolts, glows and drops to one side, then swoops across low; land on it or dash into it while it is low (three hits, it speeds up as it is hurt). The exit glows red and stays shut until it is down; hits stick across a respawn; the camera leans toward it so both are on screen. Clearing stages and best times are kept on the device, and the run's first story card picks among the stages reached (left / right, or tap the card's sides) and shows the record.

- **`page.save`: a kit's progress on the device.** `KitHost::save_json` returns what a kit keeps (Station Escape: stages cleared, best times); with `"save": true` in the manifest's `page` the page stores it under `kagra.save.v1.<slug>` (about twice a second while it changes, and when the tab hides) and hands it back as `world.save` on the next load. `Session.kitSaveJson` in wasm.

- **Station Escape, playtest 1 (Emma).** The stage 1 dash gap had no run-up after the spikes: the spikes move back (6 m of run-up) and the gap is 4.6 m (was 5.3); a test jumps half a metre early and dashes late and still makes it. Spikes can pull in and come out on a cycle (`on` / `off` / `phase`, like lasers), rattling and glowing just before they come out; stage 1 and 2 spikes do. Drones join stages 1 and 2, and a test keeps every drone's patrol off spawns and checkpoints.

- **Station Escape, round 2: the station fights back.** Patrol drones (touch one and you fall back; land on it or dash straight through to wreck it, a stomp bounces you up) and wall turrets whose bolts fly until a wall or their range stops them. Two new stages: Cargo Spine (drones and turrets, a high road) and Escape Pod (everything at once, up to the shuttle). Wrecked drones count on the clear card (40 points each). `?stage=3` on the preview link starts at that stage. The test bot clears all four stages.

- **Station Escape, round 1 (`station_escape`).** A side-on 3D platformer: run, jump and dash out of a space station that is breaking up. Left thumb runs (a floating stick), JUMP (hold for higher) and DASH (one in the air) on the right; keys A/D, Space, J/K/Shift and a gamepad work too. Two stages of JSON ledges (solid, one-way, crumbling, moving), blinking lasers, spikes, power cells, checkpoints, a story card and a clear card each; no game over. A test bot plays both stages to the end. Bundled assets only (Kenney Space Kit, Mini Characters). Design: `docs/design/station_escape.md`; build log `docs/agent-runs/20261027-station-escape/`.

- **`game_core::platform`: a platformer body** with coyote time, a jump buffer, a short hop on an early release, one air dash, one-way platforms and riding moving ones, against box solids (no physics engine). **`KitHost::buttons`** hands kits the second and third buttons (J / K / Shift, a gamepad's B / X / left trigger).

- **No more hitch when a model comes or goes.** The renderer decodes and uploads every glTF model again whenever the set of models a frame names changes (`WorldDoc::gpu_mesh_key`), and that set was the models of the *enabled* props in *first-seen order*: a walker reaching Lantern Watch's lantern, a new kind rising or the list reordering re-uploaded every model (Emma felt it on the phone). The set is now every model any prop names, hidden ones included, in name order, and `game_core::declare_models` names a game's models up front as hidden props, so the set is fixed from the first frame. Starter, Lantern Watch and Crate Tower declare theirs, with tests that a played run never changes the set.

- **Lantern Watch is harder** (Emma: too easy). Walkers are faster (ghost 1.5 -> 1.9 m/s, skeleton 2.4 -> 2.9, zombie 0.8 -> 1.05, vampire 1.9 -> 2.3), the later kinds come sooner (12 / 25 / 40 s), spawns tighten from 1.15 s to 0.26 s (was 1.6 -> 0.42) with up to 32 on the field. An ignored `difficulty_probe` plays a perfect-aim bot: it now needs about 4 taps a second to see the dawn (2.5 was enough before).

- **Crate Tower, a one-button stacker (`crate_tower`).** A crate swings above the tower; tap (or Space) to drop it. Dead centre is a PERFECT (it snaps square, and PERFECTs in a row pay more), off the top it tumbles away and costs one of three lives, and every off-centre crate shifts the tower's balance until it topples. The swing speeds up as it grows; every tenth crate is a treasure chest. Bundled assets only (Kenney Platformer Kit crate and barrel, Mini Dungeon chest, Space Kit buildings far behind). The result card's column names are now translated for `height` and `stars` too. Build log: `docs/agent-runs/20261026-crate-tower/`.

- **Lantern Watch, a one-thumb tap defense (`lantern_watch`).** The dead rise on the rim of a graveyard and walk toward the lantern in the middle; tap one to strike it (ghosts and skeletons fall to one tap, zombies take three, vampires weave), every hit knocks it back, a tap on empty ground breaks the combo multiplier, and five that reach the lantern put it out. Last 90 s for the dawn bonus. Built from the Starter template with bundled assets only (Kenney Graveyard Kit, the existing sounds); walkers, waves and scores are JSON (`game.foes`). Build log: `docs/agent-runs/20261026-lantern-watch/`.

- **`game_core::view`: camera math for touch.** `View::frame` backs the camera off until a set of ground points fits a phone either way up, `View::fit` widens the angle instead, `project` puts a world point on the canvas and `ground_at` finds the ground under a finger (Moonlit Swarm's private copies now call it). The Starter template uses it, so its arena no longer loses its sides on a portrait phone, and plays music and sound cues (`game.audio`). `Motion::parse` lets JSON pick a crowd motion.

- **survivor_arena.rs split up (no behaviour change).** 6,843 lines became the kit core (2,330) plus `survivor_arena/{spec, input, doc, hud, tests}.rs` next to the existing `hud_extra`, `loot`, `view` and `weapons`: the spec and its checks, touch / pointer input, the 3D world, the HUD and the tests each in their own file. Moved private methods became `pub(super)`; the code is otherwise unchanged.

- **Docs say what KAGRA is for now.** `AGENTS.md` is rewritten around the goal (phone browser games made with AI, the engine growing from the games), the kit + JSON + manifest workflow, the template, the shared parts, the page contract, the ship checks, assets and the PR rules; the old desktop / VRM guide moves to `old/docs-archive/AGENTS-2026-10-legacy.md`. `docs/ROADMAP.ja.md` and `docs/REVIEW.ja.md` are rewritten for the same goal (the old ones are archived next to it); the Cursor skill points at `AGENTS.md`; `docs/AGENT.md` says it maps the desktop side.

- **Starting a new game is one command.** `kagra-shared/src/starter.rs` is the smallest whole kit (stick, stars, bouncing hazards, a timer; title -> play -> result, best, pause, two languages, all JSON) on the shared parts, and the template `scripts/new_kit.py <slug> "<Title>"` copies: it writes the renamed kit, registers it in `lib.rs` and `KITS`, and writes the world fixture, the manifest (with `page`) and the `--check` example, so the game shows up in the preview and the CI browser smoke with no other edit. Tested end to end (a generated kit builds, passes its tests and starts in the browser). `docs/making_a_game.md`: the walk-through, the parts to reuse and the ship checklist. New i18n key `blurb.stick` for stick games.

- **One page for every game: `page` in the manifest instead of genre branches in play.html.** The preview page switched features on by genre (`hopMode` x18, `survivorMode` x8, `slug === "neon_cat"`), so each new game meant new branches. `games/<name>.json` now carries a `page` block (`kagra/page_spec.py`, validated at load): `best`, `audio`, `lang`, `pause`, `fixed_step`, `one_button`, `music_sync`, `revive`, `direct`, `tagline`, `theme`, `blurb`, `default_quality`, `quality_words`, `url_params`. It reaches `bundle.json` and the page reads only that; HOP-BEAT, Moonlit Swarm, Neon Cat and Meteor Cat declare what they used before (unchanged behaviour), and a new game needs no page edit. `tests/test_page_spec.py` keeps genre and slug branches out of the page.

- **Shared parts from Moonlit Swarm.** `game_core::stick` (a floating thumb stick: `land`, `follow`, `knob`, `analog` with a dead zone, `draw` with a `StickLook`) and `game_core::motion` (crowd body motion: `Motion`, `pose`, `phase_tempo`) move out of the survivor kit so any kit can use them; Moonlit Swarm now calls them. `hud_kit` gains smooth rounded pieces on the GPU's rounded quads: `round_panel`, `gauge` (pill HP/XP/charge bar) and `pill_button` (returns its hit rect). Moonlit Swarm's title blurb describes the stick instead of the old drag-to-follow.

- **HUD 2D layer: rounded shapes and a glyph atlas.** `Quad` gains `radius` (rounded corners; `Quad::round`, `Quad::circle`) and `stroke` (an outline band; `Quad::ring`), drawn by an SDF in the 2D shader with a one-pixel soft edge. Text now draws one textured quad per glyph from an R8 glyph atlas (`render/glyph_atlas.rs`: shelf-packed, rasterized once per character, raster step and face; starts over when full) instead of one quad per covered pixel, at sizes stepped so animated text reuses a few rasters. Layout is shared with the CPU `text_quads` path (`font::layout`), so positions do not move. Moonlit Swarm's stick discs and rings are rounded quads: a frame's HUD went from ~1,700 quads to ~24 plus glyphs.

- **Lighter wasm: no pixel-font fallback in the browser build.** PixelMplus (1.1 MB) stays the last fallback on desktop, Android and Python, but the wasm build no longer embeds it: every game string is in the Zen Maru Gothic subset (a test checks). Measured on the same build: 8.59 MB -> 7.45 MB raw, 2.98 -> 2.66 MB gzip, 2.17 -> 1.90 MB brotli. (wasm-opt -Oz was measured too: 7% smaller raw but larger compressed, so it is not used; the old genre modules are ~250 KB of code in all, not worth a feature split yet.)

- **CI: every game must start in a browser.** `scripts/browser_smoke.mjs` opens each playable game of the staged preview in headless Chromium (WebGL2 on SwiftShader) and fails on a page error, an error card, a load that never ends or a blank picture; Preview runs it last, after publishing. 22/22 start; a truncated wasm fails it.

- **Moonlit Swarm: a darker night.** New `arena.dark` (0..1, default 1): fill light, IBL, exposure and the sky ease toward dim moonlight, the fog comes in closer, the key light tightens around the hero, the warm lantern gets stronger, and the floor and walls lose some light, so the pale foes glow out of the dark. `arena.dark: 0` keeps the old blue-lit arena.

- **Moonlit Swarm: the foes move.** The models are static poses, so each foe now gets body motion from its transform, each model its own way: the ghost floats and sways, the zombie shuffles onto one leg, the skeleton hops and squashes on landing, the vampire glides leaning in, the keeper stomps. Every foe has its own phase and tempo (from its id), so a crowd does not move in lockstep, and a hit throws it back for the flash. `fx.foe_motion` (0 = still, 1 = as tuned, up to 2); picture only (`survivor_arena/motion.rs`). Lean direction checked in a side render.

- **HUD fonts: Zen Maru Gothic for Japanese, Cinzel for numbers.** Japanese text drew in the PixelMplus pixel face (Emma: the font looked cheap). The `Ui` face now falls back from Public Sans to Zen Maru Gothic Bold (SIL OFL, subset to every character the games use plus all kana: 355 KB), then PixelMplus. New `TextFace::Display` (`TextQuad::with_face`) draws Latin and digits in Cinzel Bold (OFL, wght 700, Latin subset: 53 KB); Moonlit Swarm uses it for the clock and damage numbers. `tools/subset_fonts.py` rebuilds the subsets; a test fails when a game string has a kanji the subset lacks. Ledger entries `font-zenmarugothic` and `font-cinzel`, OFL texts next to the fonts.

- **Moonlit Swarm: a soft floating thumb stick.** The base appears where the thumb lands in the lower-left zone (kept on screen) and follows the thumb past the rim, so turning back is instant; a faint ring shows where to put a thumb. Drawn smooth instead of from a scaled-up glyph (the old one was a blocky mosaic): one-pixel rows with faded row ends, a dark violet base and rim, a jelly neck (the hull of base and knob, thinner as it stretches, drawn once per row so it does not stack alpha) and a soft knob with a glint (`survivor_arena/stick.rs`).

- **Preview page: PowerVR phones render at 1.5x at most.** Emma's Pixel 10 (Chrome, `img-tec d-series`) blacked out the whole page, title and all, at render scale 2 (822x1530: the High and Max tiers, and Auto once it climbed to High) while the game ran and played sound; 1.5 is fine. The page reads the GPU name (WebGPU adapter info, else the WebGL2 renderer) and caps PowerVR at 1.5. As a second net, ten frames after any resize a blank picture steps the scale down a rung and the device keeps that ceiling (`localStorage kagra.scale.safe`). `?dev=1&scale=` (lifts the PowerVR cap) and `&cascades=` A/B one setting over a tier.

- **Preview page: no more silent black screen.** An exception in the frame loop, or anywhere between hiding the loading card and showing the title, used to stop the page with no title and no message. The loop now catches it and shows the error (with the Rust panic text when there is one), errors before the loop show the start error, the gamepad read turns itself off if `getGamepads()` throws (a portal iframe without `allow="gamepad"`), graphics tiers fail soft to the default, and the loading card names the GPU step while the backend starts.

- **Moonlit Swarm: a fixed on-screen thumb stick is the default.** `input.move` is `stick` (default) or `drag` (the old drag-follow): a translucent base bottom-left, a big thumb zone (lower-left), `input.stick_dead` dead zone and analog speed; any other finger taps to lock a foe, so manual aim keeps its own thumb. play.html also reads a standard gamepad (left stick or d-pad move, A / right trigger = action, Start = pause); keyboard WASD / arrows + Space already worked.

- **Moonlit Swarm: lighter crowds.** Profiled the 220-foe worst frame: the sim is ~50 us a step and the renderer setup ~0.4 ms native, so the cost is the GPU: the foes alone were about two thirds of a frame (a thousand triangles each, drawn twice with the shadow). New `WorldDoc.mesh_lod` (`keep`, `models`) thins glTF models at load by vertex clustering (cells also split on facing and texel neighbourhood, so hard edges and the texture lookup are untouched; positions stay on the surface), set by the game from `fx.foe_detail` (low/mid/high 0.5/0.65/1.0). Crowd triangles at the same kill count: low 126k -> 68k, mid 204k -> 139k, high unchanged. Gems that land beside a resting gem melt into it (a bigger, then golden, gem worth the sum; wider reach once the pool is 60% full) and repeat hits on one foe count up one damage number instead of stacking. Picture only; `crowd_probe` and `KAGRA_CROWD=n render_probe` measure it.

- **Moonlit Swarm: the warden is a fight, and the chest is easy to take.** `director.elite_fight_secs` (14) and `elite_hp_cap` (4): a warden's HP scales with the hero's firepower (weapons, levels, damage mods) so it takes about that long to bring down at half hits landing, never below its own HP and never above the cap times it; the warden's base HP 50 -> 110. A chest now drifts to the hero from 7 m with no magnet, opens from 2.4x the usual reach, and an off-screen chest shows a pulsing edge arrow with its distance. Picture and pick-up reach only; the dice order is unchanged.

- **Moonlit Swarm perf pass 2 (phone slowness) and a clearer hero.** The real cause of the slow phone was a per-frame re-decode of every GLB just to cull
  (`compile_scene` -> `compile_meshes`): now cached (`WorldDoc::cull_bounds`), native 122 ms -> ~1 ms per frame, headless CPU-bound 4.2 -> 34.6 fps. The baked env strip is cached and the per-prop world-matrix map is skipped when nothing is parented.
  #207's graphics tiers are merged in and the game opts in: Graphics row, `?quality=auto|safe|mid|high|max`, first-run probe and guard (`atmo` thins the
  dressing, `particles` thins sparks, picture only). The blaster is held only while a gun weapon is owned (blades, zap and pulse no longer show a gun beside one HUD chip) and petals are tumbling discs without a barrel-like streak. Design: `docs/design/survivor_arena.md` section 20.

- **Moonlit Swarm lighter-download pass.** The survivor preview downloads 15.10 MB -> 9.49 MB (103 -> 67 requests) with no visible change. The wasm build (`scripts/build_wasm.sh`) now uses opt-level `s`, one codegen unit, `panic=abort` and strips the name/producers sections: 9.74 -> 6.58 MB raw, ~2.93 -> ~2.27 MB gzip (set `KAGRA_WASM_OPT=3` or `default` for the old speed-first build, `KAGRA_WASM_NAMES=1` to keep names for profiling). The preview bundler no longer adds every clip under `assets/audio` when a game names its own audio packs (survivor: -31 files, ~2.5 MB; hop_beat too). `tools/gen_assets/pack_glb.py --lean` drops tangents and the second UV set from the Kenney GLBs the engine never reads (survivor bundle GLBs 1.50 -> 1.18 MB), and five unused GLBs (pines, rocks, pumpkin, wall corner) are removed. An ignored `perf_probe` test measures per-tick cost (~7-10 us) and live prop counts. Design: `docs/design/survivor_arena.md` section 19.

- **Moonlit Swarm step 2a: drops, weapon slots, new weapons, more foes.** Foes now drop pick-ups from JSON drop tables (per enemy type, with rarity, a pity timer and a pool cap): rice ball (heal), magnet, moon dust (XP), bomb, shield, frenzy, and weapon pick-ups that grant or deepen a weapon. Up to 3 weapon slots (`levels.max_weapons`); a full loadout offers a swap card. Five new weapons beside the bolt: petals (spread), blades (orbit), wisps (homing missiles), zap (chain lightning), pulse (area shove), each with its own FX and sound cue. Three new foes (fast runner, shooter that lobs telegraphed orbs, armoured tank) and a timed warden elite with a boss bar that always leaves a treasure chest with a weapon. Seven weapon x passive synergies at level 3. A loadout HUD shows slot chips with levels, and the active power-ups. All of it is JSON (`drops`, `foes`, `synergies`, `weapons`, `director.elite_*`), pooled and capped, no per-frame allocation. New CC0 assets with ledger entries: Kenney Graveyard Kit skeleton / vampire / keeper, Kenney Mini Dungeon chest and sword. Design: `docs/design/survivor_arena.md` section 18. Not done: weapon codex, evolve/merge, #201 items-core reuse.

- **Moonlit Swarm manual aim.** `input.aim` (`auto` | `manual` | `hybrid`, default hybrid) and `input.tap_px`: tap a foe with the other thumb to lock it (ring and arrow marker,
  weapons prefer it, tap again to release, a dead lock falls back to the nearest foe), tap the ground to aim that way. An AIM button on the pause card cycles the scheme. Headless tests cover lock, unlock, fallback and the muzzle line.
- **Moonlit Swarm polish pass.** The blaster now points where it fires (the hero faces the target, shots spawn at the muzzle). The arena has Kenney
  Mini Arena walls and columns and a seamless paver floor; the stray grey squares (the distant pine ring) are gone. A warm hero lantern, a calmer grade
  and tinted foes fix the washed-out look. New `fx` spec block, all switchable in JSON: pooled sparks, death puffs, gem glints, muzzle flash, shot
  streaks, damage numbers, hit knockback, shake and zoom pulses (own RNG, so a run is identical with them off; pools capped per tier 48/160/480).
  Polished HUD and level-up cards with icon badges and level pips. Audio: Kenney impact hit and a wet-squish pop. New CC0 ledger entries: Kenney Mini Arena,
  Blaster Kit, Impact Sounds, and OpenGameArt wet squish. Design: `docs/design/survivor_arena.md` section 16.

- HOP-BEAT: a jumping enemy, the hopper (a green frog), replaces the bat in the late phrases that put a bat over a beetle or over a slime (the bat now only flies alone, in its level-2 debut). It is the genre-neutral `game_core::enemies` schema in JSON: `jump_height`, `jump_air` and `telegraph` on an enemy type turn a walker into a jumper that sits, squats (the tell) and leaps one step at a time, landing on the beat; it moves 3 m a beat against the beetle's 2 (JSON). It debuts alone at level 3 (phrases: hopper alone; beetle then hopper then a gap; slime-hopper-slime; a hopper between two hold gaps; frog-beetle-frog-slime), warns with the existing `warn` cue and the "!" badge, and has a procedural CC0 model `hop_frog.glb` (ledger text updated). The fairness checker covers it: the leap may not overlap a gap hop, two enemies of different speeds may not overlap in the last beat before the hero reaches the nearer one, seams with a hopper are played at +-90 ms and the 2000-seed laid-course test includes them. `docs/enemy_kit.md` documents the fields and rules; the fixture and measure baseline are regenerated.

- **Moonlit Swarm, the survivor-arena kit (step 1).** A survivor-like game on the shared parts: `game_core::swarm` (pooled foes, spatial
  grid, spawn budget, seeded RNG), `EnemyType.swarm` stats, and a new `KitHost` (`kagra-shared/src/survivor_arena.rs`, genre `survivor_arena`,
  manifest `games/survivor_arena_minimal.json`). One finger drags and the hero walks toward it; the bolt weapon auto-fires; ghosts and zombies
  swarm in growing numbers; kills drop XP gems; a level-up freezes the sim and offers 3 cards (bolt level or six passives); the run ends at dawn or
  on death and feeds the #203 meta layer (coins, five hero skins, daily seed and modifiers). `play.html` has a small `survivor_arena` branch
  (fixed 60 Hz step, pause on Esc/hidden tab, result labels); no graphics-setting code is touched. Art and sound are CC0 only, copied only when used,
  one ledger entry per pack: Kenney Mini Characters, Graveyard Kit and Platformer Kit (jewel), the OpenGameArt "50 CC0 retro synth SFX" and "Flowerbed
  Fields" loop, and one Kenney interface chime. Models are rest-pose copies made by `tools/gen_assets/pack_glb.py`. Design: `docs/design/survivor_arena.md`.

- Graphics tiers, shared by every game: `safe` / `mid` / `high` / `max` defined in one JSON file (`kagra-shared/src/quality_tiers.json`: render scale,
  picture quality, SSAO, TAA, shadow cascades and softness, bloom, sky props, particles, fx switches), a **Graphics** row on the settings card
  (auto + the four tiers, EN/JA), auto-selected by a first-run probe and a runtime guard that steps down on a slow stretch and back up at calm moments
  (frame-time trend, bans against flapping), stored in `localStorage` (`kagra.quality`), `?quality=auto|safe|mid|high|max` (and the old `low`/`mobile`) for a
  visit. A kit opts in (`supports_quality_tiers`, `quality_hints`); HOP-BEAT does, Neon Cat is unchanged. A game may override the tiers with a
  `quality_tiers` block. Python mirror and validation in `kagra/quality.py`. HOP-BEAT gets an `autoplay` bot switch. See `docs/quality_tiers.md`.

- Measurement tool: the recorder is now a fixed wall time per step, counts the seconds the page stopped drawing instead of dropping them silently
  (`wallMs`, `coverage`, `stalls`, `hiddenMs`, a `note`), keeps a run going with the game's autoplay bot (restarts after a game over), and gets a
  **Find my limit** button: stepwise adds load (scale, bloom, sky props, particles, shadows, SSAO, soft shadows, TAA), 9 s a step, and reports the
  last step that held p95 <= 16.7 ms (+0.5) and p99 <= 20 ms, a suggested tier, and GPU time where the WebGL2 timer exists (`&backend=gl`; else it says why not).
  Copy JSON is `v` 2 and carries `limit`. See `docs/phone_measure.md`.

- On-phone measurement tool, shared by every game (`?dev=1`): an overlay with fps, frame time p50 / p95 / p99, jank,
  render scale, backend, adapter, canvas and DPR, display refresh, JS heap and a thermal guess; **Record 60 s** runs the
  `fx_post` / `fx_atmo` A/B (all-off, post, atmo, both, 15 s each) and shows a table with **Copy JSON**. HOP-BEAT gets a runtime
  `setSwitch` (picture only). `render_skip` (off by default, `?render_skip=1` or world.json) skips the draw on a 120 Hz frame
  where the 60 Hz sim did not step. The tool is `scripts/preview_site/perf.js`: dev only, not in portal zips, no network,
  nothing stored. See `docs/phone_measure.md`.

- HOP-BEAT fixes. (1) A fall that costs a life no longer pops the bunny straight back: the drop plays out, the world holds for a READY count of 3 beats (JSON `recover` block, Python mirror), the music restarts on a bar line and the bunny blinks while protected for 2.4 s; a Continue gets the same count. (2) A bat could hang over a gap, where a tap held a little too long (a thumb, over the 110 ms tap window) turned into a hold hop that flew into the bat with a hole below. The `tap_window_secs` default is 0.16 s, the bat+gap phrase is rearranged (bat over the beetle, never over the gap), flyers may not overlap the hop across a gap (Rust and Python rule), flyer phrases are checked at +-100 ms, and every ordered pair of phrases with a bat is played back to back (`seam_problems`); a 2000-seed x 5-modifier test checks every seam the generator lays and a thumb-tap bot clears the course.

- Retention layer, shared by every game (`meta`): a coin wallet earned from runs, an unlock catalogue (hero skins and
  background looks, music later) with prices and rules, a daily run (one fixed seed per UTC date, its own best, a streak with a
  small coin reward, a text share card) and run modifiers, with a shop screen, EN/JA. It is a manifest block (`meta` in
  `games/<name>.json`, validated by `kagra/meta_spec.py`, copied into `bundle.json`) read by the new `scripts/preview_site/meta.js`;
  it edits the spec a game is loaded with and never touches the simulation, so baked fixtures and baselines are unchanged. A kit
  opts in by listing the spec paths a patch may touch (`meta.patchable`). Rules mirrored in Rust (`game_core/meta.rs`), Python and
  JS from one vector file (`meta_vectors.json`). HOP-BEAT opts in with its real skins and looks (9 unlocks, `stomps` earn coins, daily best kept apart from the free-play best, 4 daily modifiers, an optional "double coins" rewarded
  slot). No server, no leaderboard, no network. See `docs/meta_core.md`.

- Shared foundation, step 1b: the session holds one boxed kit (`kit: Option<Box<dyn KitHost>>`, new `kit_host.rs`) instead of one
  slot per genre. Touch, pointer, key, tick, state/dump JSON, HUD, title/confirm and debug now go through the trait, so a new
  kit adds one `impl KitHost` and one line in `kit_host::KITS` rather than a field and about ten `if let Some(cat)` branches.
  `SceneKind::Neon` and `NeonCat` became `SceneKind::Kit`; tests and tools reach a kit with `session.cat()` / `session.neon()`.
  Behaviour is unchanged (measure baselines, golden runs and the mesh-key test are untouched).

- Shared foundation, step 1: lives, goals and the game-over/banner state moved out of `neon_cat.rs` into the generic
  `game_core` (`lives.rs`, `missions.rs`, `result.rs`), carried on `Core` so any kit can use them from the `lives` and
  `missions` GameSpec sections. Behaviour is unchanged: the golden runs, the measure baselines and the GPU mesh-key test
  are untouched. `LivesSpec` and the mission types are re-exported from their old paths (JSON is identical). Python
  mirror `kagra/run_rules.py`; one vector file (`run_rules_vectors.json`) is replayed by Rust and Python.

- Neon Cat: the goal "perch up high once" is no longer offered on the plain street, where it could not be cleared (the stair lane
  and the decks are off by default). `missions.list` is now a pool and the new `missions.draw` (default 3, 0 = all) says how many a run
  shows. A perch goal is only drawn when `perch_rules.enabled` or `deck_rules.enabled` is true. The default pool gains a street goal,
  `passes` ("get past 30 obstacles", counted whether or not one hit the cat; Japanese "しょうがい物を30個こえる"). Plain street: coins,
  gates, passes; with the stair lane or decks on: coins, gates, perch. Rust and Python agree (`MissionsSpec::active` /
  `MissionsSpec.active`); a spec that lists a perch goal with the course off simply skips it.

- Neon Cat: the stair lane (perch steps, raised steps and slopes) and the new upper course
  are **off by default** (`perch_rules.enabled`, `deck_rules.enabled`, both false) because
  they were hard to see on a phone; the default course is the plain street again, as before
  the perch routes. The code, validators (Rust + Python), JSON tunables and measure checks
  stay, and each switch brings its half back; tests run both the on and the off paths. The
  side-hit rule still applies to any ramp that exists (a train's wedge). Everything else
  merged since stays (night atmosphere, fx tiers, GL fix, portal prep). Ideas to make them
  easier to see before re-enabling: a higher or tilted camera over a deck, translucent deck
  slabs, an emissive outline glow on edges, and side lighting that rakes across the steps.

- Neon Cat: an upper course (off by default, see above). `deck_rules` puts short raised decks (2 m up, 7 m long, open
  underneath) over a lower lane in some free slots; only a Superjump gets up (an orb
  lies before each deck), a plain jump passes under. The stair lane now carries on up
  to a deck as a second way up, with no jump. The stair lane is rarer (every 4th free
  slot of a rest or reward section, was every 2nd), so there are fewer slopes. Running
  into the side of a stair step, a stair slope or a train's ramp is now a hit; the front
  ride-up, a jump across and the stair lane's first-step switch-in (#192) are not. Spec
  checks (Rust `validate_decks` with a Python mirror `_check_decks`) refuse decks a plain
  jump can reach, that a high jump cannot, or whose drop lands on the next obstacle; the
  measure harness plays the decks live (`decks`, `deck_*` codes). A deck is only built
  in a slot whose drop (at the speed cap) lands at least 0.15 s short of the next
  obstacle's real front; otherwise the slot stays plain (a stair lane then has no deck).
  The bot now keeps off step walls.

- Web-portal prep (nothing submitted anywhere): `scripts/preview_site/portal.js` is a
  host shim with a no-op default and CrazyGames / GameMonetize adapters
  (`?portal=` or the `kagra-portal` build flag; the default build makes no
  external request). It sends gameplay start/stop, asks for midgame ads at the
  breaks `platform.ads` allows, and mutes, pauses and blocks input during ads.
  The page also pauses on blur / tab hide, blocks scroll gestures, has an
  English / Japanese string table (`i18n.js`, `?lang=`), and keeps `?debug=1`
  and `?backend=` behind `?dev=1`. `platform.ads` lives in the game manifest
  (`kagra/platform_spec.py`), not in the Rust spec. `scripts/package_portal.py`
  builds a single-game zip and checks a 5 MB / 8 MB budget (`--wasm-opt`
  optional). Cover art comes from `tools/gen_assets/cover_art.py`
  (`docs/media/neon_cat/`); store text and the AI-use statement are in
  `docs/publishing/`. Rewarded ads need an engine effect and are not offered yet.

- Neon Cat: fix the stutter that came with the night look. The textured wall panels
  were listed for the GPU in a different set and order every time the cat crossed a
  12 m segment, so the renderer dropped and re-uploaded every mesh (textures
  included) about once a second. The ring now lists the same panels in the same
  order on every frame (`the_gpu_mesh_key_never_changes_while_the_cat_runs`). The
  `mobile` tier also draws a 30 m wall ring, no emissive map, no ripples and 36
  rain streaks (was 56). New `?fx=low|mid|high` and a render-scale step-down
  (2 -> 1.5 -> 1.25 -> 1) when frames run over 26 ms for 2.5 s (`?adapt=0` = off).
  `examples/fx_cost.rs` times the render per frame.

- Neon Cat: a night street, sound, and goals (`atmosphere`, `audio`, `missions` in
  the game spec). Rain with ripples, neon signs and a procedural concrete wall
  (`tools/gen_assets/cat_wall.py`) on the neon base look; three picture tiers
  (`low`, `mobile`, `high`) and a plain-wall fallback. The phone page plays BGM and
  SFX with Web Audio from JSON-named `.wav`/`.ogg`/`.mp3` files (unlocked on the
  first touch, mute button, volumes in JSON); placeholder sounds come from
  `tools/gen_assets/cat_audio.py` (CC0). Three missions per run (40 coins, 5
  gates, 1 perch) and a high score on the title and result. All three default on
  for Neon Cat and off in `GameSpec::legacy()`; Meteor Cat keeps its own look
  (`atmosphere` and `missions` off). Mission `bonus` is 0, so scores, the
  ranking and the golden runs do not move. See `docs/runner_kit.md`.

- Lumen (`games/neon_run_minimal.json`): a neon endless flyer. The sim is
  `NeonRun` in the shared crate (wasm, Android, and the Python window share
  it) and is not a WorldPlay genre. Glow uses dump `emissive` /
  `emissive_map` (`preset:core` on the ship). Reflections use `hemi_sky`,
  `hemi_ground`, `env_map: preset:studio`, and `shadow_softness`.
  The window picture calls `apply_look(world, "night")`, draws `kagra.Trail`
  (the shared emissive quad ribbon), and sets `cull` plus `prop_lod`.
  `Emitter` can set `emissive`. The phone page is
  `kagra-shared/www/play.html` (GitHub Pages default). Corridor Haul moved
  to `corridor.html`. `scripts/package_web.py` stages the site. Art is
  procedural. Sound clips stay the CC0 tones in `assets/audio`.

- Trails, prop LOD, frustum bounds, art-direction bundles, and glTF ORM.
  `cull: true` registers mesh bounds and drops props outside the camera.
  `prop_lod` turns a distant non-vegetation prop into one billboard.
  Omit either and the old picture stays (every prop, full meshes).
  glTF `metallicRoughnessTexture` is linear ORM with factors baked in;
  a dump map still replaces it, and toon primitives skip it.
  `kagra.Trail` is a ribbon of emissive quads.
  `apply_look(world, "pastel"|"night")` sets sky, lights, exposure,
  hemisphere, and the LUT. Box Hop draws the hop trail under the night bundle.

- Image IBL and an authorable hemisphere on the shared renderer.
  Dump `hemi_sky` / `hemi_ground` feed the L1 SH (omit them and the studio
  pair stays). `env_map` is `preset:studio` or a PNG equirect, baked to a
  64×64 lat-long with 4 roughness rows (`texture_2d`, no cubemap). Specular
  on metal, water, and solid gloss samples that strip. Omit `env_map` and
  specular stays the SH, so an old dump does not change. Box Hop uses a
  cooler sky, a warmer ground, and the studio env.

- Authored bloom sources, roughness/metal maps, and a wider sun shadow.
  Dump `emissive` / `emissive_color` / `emissive_map` add an unlit HDR term
  the bloom pass can extract. `roughness_map` and `metalness_map` are a PNG
  or `preset:worn` / `preset:split` / `preset:brushed` (linear ORM, G and B).
  `shadow_softness` omitted stays the 3×3 PCF; about 3 uses a 5×5 tent.
  Box Hop glows the well, splits the crates between shiny metal and rough dielectric, and softens the sun.

- Phone-web picture on the shared renderer: dump `bloom`, `bloom_threshold`,
  `vignette`, `lut` (`off` / `warm` / `punch` / `cool`), `lut_strength`, `fxaa`.
  Omit them and the old picture stays (bloom off, vignette 1, no LUT, FXAA on).
  The LUT is a 16³ strip in a `texture_2d` so WebGL2 can sample it.
  `kagra.Juice` is shake, hitstop, squash/stretch, and easing.
  `kagra.Emitter` is a capped CPU spark list drawn as sphere props.
  Box Hop uses the mobile preset (`KAGRA_QUALITY=0` turns it off).
  Gap table: `docs/quality_gap.md`.

- CI: `python-unit` はネイティブ拡張が本当に無いときだけ `WorldDoc` /
  `WorldPlay` の実体チェックをスキップする（切った名前と annotate / brain は
  検査したまま）。ホイールを入れた `build` ジョブが両方の `from_json` を断言する。
  Android debug APK は `android-actions/setup-android` の `sdkmanager tools` を
  やめ、Package Android と同じ `scripts/setup_android_sdk.sh` を使う。
  `demo_live_smoke` は欠落スクリプトのまま本線に置かず、`old/examples/` へ移した。

- Box Hop（配布の既定）にタイトル → プレイ → ポーズ → 結果 → リトライ。
  音量とハイスコアは Rust の JSON セーブ（Windows は `%APPDATA%\kagra`、
  Android は内部ストレージ）。`kagra.audio` は BGM のフェード、効果音、
  グループ音量、ミュート。クリップは `assets/audio/` の CC0 合成音。
  デバイスが無いときは無音。箱跳びのジャンプ、FPS の発射、カードのプレイに
  効果音を付けた。

- Flight pose on a free `WorldDoc`: `WorldProp.pitch` / `roll` (nose up / bank)
  and `local: true` parent TRS (up to 4 links). `parent` without `local` stays
  a link so existing world-space positions do not move. `model: "ring"` is an
  XY torus (hole along +Z). `render_rgba` / `rgba_to_ppm` let `kagra.gameloop.run`
  present a frame without PNG compression when `Scene._frame_rgba` is set.
  Multi-primitive static `.glb`/`.gltf` merge material `baseColorFactor` palettes
  and node transforms; KayKit characters filter unskinned attachment meshes and
  pick `Walking_*`/`Running_*` clips.
  PBR specular + Fresnel environment reflections + CPU mipmaps on `Material::Solid`,
  plus sub-mesh selector (`chess_king`, `chess_queen`, ...) for PolyHaven `chess_set_1k.gltf`.
  Samples: `examples/flight_minimal.py`, `examples/backyard_brawl_minimal.py`,
  `examples/chess_minimal.py`.

- Dump picture fields: ``sky`` (clear+fog), ``ground`` (auto grass plane;
  indoor rooms set false), ``WorldProp.uv_scale`` (CPU-tiled albedo).
  ``expect_offscreen.pixels`` checks region mean / not_solid (not golden).
  Linux CI installs lavapipe and runs ``bar_sim_smoke`` after the wheel
  import. ``render_world`` prefers the installed ``kagra_shared`` extension.

- **0.2.0 mainline cut:** `import kagra` is shared wgpu 30 (`WorldDoc` /
  `WorldPlay` / `gameloop`). Root `pyproject.toml` builds `kagra-shared` as
  `kagra.kagra_shared`. Archived RendererV2 / `kagra_core` is not imported.
  `WorldDoc.genre` is the only genre switch (prop names are scenery).
  `WorldProp.texture` + HUD `images` show PNG art. MSRV 1.88 + `rust-toolchain.toml`.
  Bar management sim: `examples/bar_sim_minimal.py` (free-world dump, procedural
  assets under `assets/bar/`).

- Distance LOD + GPU instancing on shared wgpu 30: repeated vegetation (Crest trees/grass) uses dump lod_radius / lod_cells for two levels (full trunk+cone vs existing billboard quad) and batches same-mesh props into one draw (instance locations 2..7, no base_instance, WebGL2-safe). Kenney vegetation is not thinned. Scene3D::render_stats exposes batch/instance counts. No Bevy / SSAO / GI / SSR / Rapier / second renderer. world_play.rs untouched. Relic Run UV defaults stay. examples/vrm_open_world.py not rewritten.

- Official Emma VRoid play dump: ``python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json`` walks ``assets/Emma.vrm`` (repo-relative; Mixamo rest+roll, MToon/albedo/spring/morph/look-at already on VRM walkers). ``python -m kagra.play_world`` with no args stays Crest Isle capsule collectathon. ``Emma.vrm`` stays gitignored; missing file falls back to tiny ``tpose_humanoid.vrm`` so CI stays green. No Rapier / SSAO / second renderer / new ECS. ``examples/vrm_open_world.py`` not rewritten.

- 2D action projectile + room switch on ``play_world``: J / click from range spawns a dump-visible ``shot`` sprite that moves and can hit/kill; crossing a ``trigger`` swaps hall <-> den (dump scene / name / flag). Walk / melee hit / kill stay. Sprite/quad stays on WorldDoc. No SSAO, no VRM, no new ECS, no RendererV2, no Rapier.

- First M3 genre (collectathon): ``python -m kagra.play_world`` is one loop — title → play → result, walk the island, pick up, count, finish. Shared ``WorldPlay`` reuses collectathon ``IsleGame`` / pickups (no new ECS). ``compile_scene`` production picture on shared wgpu 30: island heightfield (not a placeholder plane) + Grass albedo, metal coins (existing GGX ``Material::Metal``), lights slot 0..3 1:1 (no leak), readable capsule (body+head). Verify: ``examples/verify_scenarios/collectathon_smoke.json`` (coins / on_ground / query / tile albedo_ok + offscreen not empty). GPU-free tests stay green. ``new_for_window`` stays ``cfg(not(wasm32))``. No VRM skin, no Rapier, no RendererV2 delete, no action/RPG.

- One runtime (M2 remaining, before the kit): WASD + look (mouse / arrows) flow into ``WorldDoc`` (walker position/yaw, chase camera). ``python -m kagra.play_world`` / example ``window`` ticks shared ``WorldPlay`` each frame (collectathon ``WalkInput`` → sit on named height fn / samples). ``compile_scene`` emits a heightfield batch (not a flat plane) and glTF props via ``gltf_load`` (``cube.glb`` / ``crate.glb`` alias). Capsule player; VRM skin is not ported. Official Crest play is this wgpu 30 window walking a Crest dump. ``examples/vrm_open_world.py`` may keep RendererV2 for the leftover VRM path. New games must not start on RendererV2. Fake-headless ``(-12800,-12800)`` stays only for leftover V2 smokes. No Rapier, no wgpu mix, RendererV2 is not deleted.

- One runtime (M2 window wedge): ``python -m kagra.play_world dump.json`` (or ``python examples/world_doc_window.py``) opens a **real** desktop window and presents a compiled ``WorldDoc`` through the existing kagra-shared wgpu 30 ``Renderer`` (same as collectathon / mobile / ``render_world``). Esc / close / optional ``--seconds`` camera orbit. Subprocess so wgpu 0.19 and 30 never mix. Does not use kagra-core ``RendererV2`` or ``window.rs`` / ``(-12800,-12800)``. Desktop VRM / Crest Isle stay on RendererV2. Missing helper, no display, or no adapter skips. No WASD, no Rapier, no extra PNG goldens.

- One runtime (M2 verify): ``python -m kagra.render_world dump.json out.png`` shells to the shared wgpu 30 offscreen helper (installed ``kagra-offscreen``, already-built ``target/*/examples/offscreen``, or ``cargo run -p kagra-shared --features render --example offscreen -- W H out.png world dump.json``). ``kagra.verify`` ``expect_offscreen`` smokes PNG file size / non-empty / IHDR dimensions (not golden pixels). Missing helper or no GPU adapter skips. Does not delete ``RendererV2``, does not retarget the desktop window, does not mix wgpu 0.19 and 30, does not use ``(-12800,-12800)``. Crest Isle UV/streaming untouched.

- One runtime (M2): persistent ``WorldDoc`` in ``kagra-shared`` matches ``docs/schemas/world.json`` version 1 (stable string ids, props, parent, heightfield, lights, cameras, walkers). ``from_json`` / ``to_json`` roundtrip dump JSON. ``compile_scene`` builds a one-frame ``Scene3D`` (camera + batches; capsules / box / sphere / plane). ``render_world_doc`` (feature ``render``) uploads ``compile_meshes``, draws those batches on the existing shared wgpu 30 offscreen ``Renderer``, and reads RGBA8. No kagra-core window, no ``RendererV2``, no wgpu 0.19 mix, no ``(-12800,-12800)``. GPU tests skip without an adapter. Python dump stays the exporter; no new public game API. Desktop window retarget is next. Crest Isle UV stream is not this slice.

- Crest Isle hillside black tile + gold spec: one streamed 16 m chunk was a pitch-black quad with a GGX highlight (geometry still there). Not the #94 dirt-rim stamp, not the #95 barcode / failed-upload hole, not the #96 JPEG biome window, not the #97 dead-albedo leftover. Terrain upload now pins Lambert (``metallic=0``, ``roughness=1``) so coin PBR cannot stick to a heightfield tile; ``World3D.draw`` draws live ``_tile_meshes``; mesh_mat slots init Lambert and pack 1:1 with the draw list (a missing mesh no longer shifts later tiles onto leftover gold). ``uv_rect`` / dead-albedo unload stay. Relic Run UV / PBR defaults unchanged. No Rapier / SSAO / brighter ``GRASS_TINT``.

- World as data (15% → toward 35%): ``World`` is ``World3D``. Stable string ids. ``world.query(type=, name=, aabb=)`` returns position / name / type / id without a screenshot; terrain tiles expose ``loaded`` / ``albedo_ok`` so Crest Isle はげ is detectable without a PNG. ``world.dump()`` / ``world.load()`` JSON (schema ``docs/schemas/world.json``) for Prop + parent id, heightfield name/samples, lights, camera, walkers. ``kagra.verify`` ``expect_world`` asserts player.on_ground / coins / query counts. Entity / tilemap / Tk are off ``import kagra`` (files stay; archive 2D imports from ``kagra.entity`` / ``kagra.tilemap``). Roadmap 80% redefined (100% = screenless indie ship; now ~15%; old 63% archived). Drawing / Rapier / SSAO / terrain stream (PR #97) / Relic Run UV defaults untouched.

- Crest Isle remaining ハゲ (GGX-only 16 m TILE): ``World3D._upload_tile`` no longer swallows TypeError / bind failure into a leftover mesh. Failed / 1×1 / missing albedo unloads the GPU id and retries (does not stick ``lod_ok``). LOD upgrade keeps the previous good mesh. Mesh3D bind groups are created at ``upload_mesh_3d`` and culled stream tiles stay pinned (PR #92 family). ``TERRAIN_UV_RECT`` / stream retry / prefetch unchanged. No Rapier / SSAO / Repeat / per-tile 0..1.

- Crest Isle remaining meadow ハゲ: ``aerial_grass_rock_diff_1k.jpg`` is mixed moss + brown rock even inside pad 0.28. Period 48 made each 16 m TILE a different 2D biome window (green tile glued to yellowish dirt). Crest UVs now ping-pong into a compact meadow-green ``TERRAIN_UV_RECT`` measured on the tinted JPEG. Period 48 / stream retry / LOD_CELLS=6 stay. Relic Run keeps the uncropped JPEG / default UV. No Rapier / SSAO / Repeat / per-tile 0..1.

- World3D terrain stream: failed ``upload_mesh_3d`` no longer sticks as loaded (bald tile skip-forever). Streaming worlds prefetch a 1-tile ring and delay unload one frame; LOD upgrades beat brand-new far tiles (still 1 GPU upload/frame while walking). Crest Isle meadow UVs: ``TERRAIN_UV_PERIOD=48`` (3× TILE, world-continuous, pad 0.28) so a 16 m chunk is a small 2D moss window — period 9.5 < TILE + ``lod_cells=3`` was the barcode / 1-axis JPEG stretch. ``LOD_CELLS=6``. Relic Run UV defaults unchanged. No Rapier / SSAO / Repeat sampler / per-tile 0..1 UV.

- Crest Isle bald meadow: `aerial_grass_rock_diff_1k.jpg` is a non-tiling aerial photo (mossy interior, dirt at UV 0/1). ClampToEdge + ping-pong of the full 0..1 range stamped that square as a grass island. Crest Isle now pads UVs past the dirt rim (`TERRAIN_UV_PAD=0.28`) with a world-continuous period ≠ TILE. Relic Run keeps default UV. No Rapier / SSAO / Walk.wish / Mesh3D pin revert.
- Character controller (no Rapier): `Walk.wish` / `Walk.move` / `Walk.try_jump` and `kagra.CharacterController`. Accel/decel (defaults 14 / 22 m/s²), ground stick, 8-point foot ring, bump-raise only for real ledges (one-sided max-Y on a slope was the remaining float — not missing Rapier). Capsules skip ground friction so the motor owns stop. Static prop lips ≤ `step_height` are climbable. Crest Isle constructs `Walk(..., controller=CharacterController(...))`. Sticky-walk quiet gap 3 is unchanged (input). Wheel size: no Rapier crate.

- Crest Isle picture pass (black trees / fake AO / terrain seams / CPU sparks / hair rim / gold orbs): Kenney forest GLBs keep `Textures/colormap.png` next to the `.glb` (no Blender re-export). `flatten_gltf` / Rust `extract_texture_data_from_glb` resolve that directory-relative URI; `KHR_texture_transform` is applied to atlas UVs (Kenney currently ships `{texCoord:0}`). Character blob is a soft Y-quad under the feet (`quad_y_mesh` + skip_fog), not SSAO. Stream tiles sample height *outside* the chunk for shared normals and Crest Isle blends/pads meadow UVs so the 16 m join is not a grass/dirt knife. Pickups spawn a CPU `{position, velocity, life, fade}` burst drawn with `draw_billboard_instances`. VRoid / Alicia **hair** MToon gets a stronger rim (`Hair` / `髪` names, warm color, lift 0.62); face/skin stay off that path and backfaces stay flipped so the skull does not white-mask. Crest coins are gold PBR spheres (`metallic=1`, `roughness=0.12`), not Kenney yellow discs. `apply_outdoor_look` does not raise global `set_rim`. Sticky-walk quiet gap 3, Mesh3D LRU 256, chase cam clamp, opaque title, spatial listener, `set_locomotion` / Mixamo `bind_locomotion` unchanged. No Rapier / SSAO / 4-cascade CSM / volumetric fog / visual editor / Web/XR / Mixamo binaries.

- Mixamo FBX locomotion on VRoid: rest-pose + bone-roll retarget so Idle/Walk/Run hang and swing instead of folding into a carry pose (T-pose Emma and A-pose Alicia). `avatar.bind_locomotion()` loads local Mixamo FBX into the existing `set_locomotion` mixer (never the `walk` alias / `synthetic_walk.bvh`). `dance()` stays a full-body drop path. No Mixamo binaries in git.

- Multi-avatar GPU share: same-path `kagra.avatar()` / `load_vrm()` clones Arc-share mesh / texture / MToon. Joint palettes, pose, expressions, and SpringBone stay per instance. Mesh3D LRU (256, never evict live diffuse) is still Props-only — a second Alicia does not multiply that cache. Measure with `kagra.vrm_gpu_stats()`. Spawn extras in `examples/vrm_multi_avatar.py` (`KAGRA_AVATARS=N`). Crest Isle play stays one player (title / input / camera). No Rapier / Mixamo / spatial audio / terrain retune.
- Crest Isle spatial audio: engine listener + world sources (`set_listener`, `play_se(..., x=, y=, z=)`, `play_loop`). Inverse-distance gain and equal-power stereo pan (no HRTF). Crest Isle loops a procedural sea drone at west water; crest/coin pickups play at the collectible. Title/start/win stay 2D. `sound()` unchanged. No Rapier / Mixamo / locomotion / multi-avatar / CSM.
- Crest Isle locomotion: `avatar.set_locomotion(speed)` blends built-in idle/walk/run (no Mixamo). Start/stop and analog stick no longer hard-cut the clip. `play_upper` / ActionController own spine/arms while legs keep walking (overlay does not mutate locomotion `current_rots`). `walk_wish` keeps analog magnitude. Mixamo/BVH walk still skipped (folded arms). No Rapier / terrain retune / spatial audio / multi-avatar.
- Sleeve / cloth stiffness on VRM SpringBone (Crest Isle Alicia / Emma-style MToon). Verlet now matches UniVRM / three-vrm (`stiffness * dt²` along the rest axis) so authored hair/skirt no longer glue (stiffness=1) or flop like paper (stiffness>1). VRM 0.x leaf bones (ribbons) get a 7cm virtual tail instead of being dropped. Models with no sleeve helper bones (Alicia Solid) get four arm-parented helpers; outer-tube weights transfer so the sailor sleeve lags as fabric. VRoid `*Sleeve*` / 袖 bones already in the file are chained, not duplicated. No Rapier. Crest Isle terrain / Mixamo walk / blend trees / spatial audio / multi-avatar are unchanged.
- Crest Isle look (on top of #82 cache/stream): chase cam clamps eye distance so wall-clip / hitch lerp / orbit zoom cannot fly to a tiny speck, slam into the VRM skull, or fog-white the world. Meadow `mesh_mat.base` tint is Crest-only so the shared aerial grass JPEG reads green without blowing white. Sky/backdrop pass `draw_mesh_3d(..., skip_fog=True)` so puresky is unlit and unfogged; fog-off is not inferred (that unlit every Mesh3D and broke goldens). MToon flips backface normals so inside-skull hair/face does not rim-blow to a white mask. Long-hold leftover KEYDOWN still uses #82's refresh window, but the quiet gap is 3 frames (~50ms) not 15 (~250ms) so a real same-key press after ~3 silent frames walks. Taps keep #80's 1–2 frame window. Does not revert #81 IBL `albedo * 0.35`.
- Sticky Walk after key-up on Windows: ignore post-`WM_KEYUP` KEYDOWN (`repeat=false`, which #71 missed), pair IME/JIS `Unidentified`/`Process` releases to WASD/arrows, disable IME on the game window, and stop leftover vx/vz when wish is 0 (steep slide kept). GPU-free tests cover the remaining path, not only #71.
- Slope grounding: tight foot AABB (0.08) + extra samples + snap-to-plane. `|foot_y − terrain|` while `on_ground` stays under 0.05 (`debug_trace`). Fat capsule AABB max-Y on a slope still floats — measure first. Still no Rapier.
- `kagra.stage` is the documented callable again (it was shadowed by `kagra/stage.py`). Same guard for `annotate` / `pad` / `brain`. Crest Isle / Relic Run sky spheres no longer TypeError.
- Crest Isle: init `_chunk_props` before `bake_terrain` so the first tile stream does not AttributeError.
- Agent eyes: `kagra.annotate` (click → JSONL) and `kagra.debug_trace` (foot vs terrain). Not a visual editor.
- `Camera3D.follow(..., world=)` pulls the chase camera in so it does not go through walls.
- Prop / terrain Lambert uses the same `cam.toon` stepped lighting as VRM when `set_toon_params` softness < 0.999.

## 0.1.4

Playable 3D, picture pixels, and `kagra.brain` on the wheel. Windows / Linux,
~5MB, no Rust. Rapier crate stays out. 30-second stranger demos are not this
release. Do not call 80% or three.js-class.

### 3D play

- `Prop` / `Walk` / `sky` / `room` / `World3D`. First-person, hover, carry,
  kinematic move, `destroy`, sphere/cylinder hit, 4-level parent, glTF parts
  (`Prop("crate.glb")`).
- Heightfield island, city JSON (not OSM), triangle mesh hit, tiled terrain.
- AABB rigid boxes: `add_box(..., is_static=False)` fall, stack, and `Walk`
  stands on them. Character vs crate does not sink the crate.
- Gamepad: `axis` / `pad` / `inject_pad`. USB/XInput via gilrs on the
  EventLoop (Windows: one loop). Real-hardware 30s not claimed.
- Agent games: Heart Catch, Switch Room, Dodge Room (logged). Orb Rush is
  the reference (no generation log).

### Picture

- Four local lights (`slot=0..3`; slot 0 is the key / shadow). Indoor spot
  umbra, 2-cascade outdoor shadows (per-layer light-VP), tangent-space
  normals, HDRI / irradiance, generic metal/roughness, opt-in ACES.
- Pairwise goldens on Windows CI: `indoor_spot`, `normal_bump`, `local_four`,
  `outdoor_crawl`, `tonemap_on`, `ibl_metal`.
- World mesh casters, VRM cull, 3D instancing, material sort.

### Brain

- `kagra.brain("kairi"|"ollama"|"openai")`. Default kairi is
  `https://kairi.onrender.com` (`KAIRI_API_TOKEN`). Models stay out of the
  wheel.

### Docs / install

- README / README.ja: `pip install kagra` is 0.1.4. Not yet is the remaining
  holes (macOS wheels, 30s demos, chat APIs, NDI/RTMP, autopilot, bundled
  TTS, pointer lock). Tilemaps / ECS stay on the shelf.

## 0.1.3

Streaming-V slice. OBS can take a virtual camera; VOICEVOX and chat stay
outside the wheel. WAV / VRMA still download on first run (keeps the ~5MB
install).

- Virtual camera extra: `pip install "kagra[stream]"` (`pyvirtualcam`).
  GPU `set_grab_frames` / `grab_frame()` returns RGB. 720p recommended —
  readback is every frame. `python -m kagra --loop --stream`.
- Stream HUD: song title, subtitle, recent chat via existing 2D text
  (`StreamHud`).
- Chat inbox: JSONL `{user,text}` (`ChatInbox`). YouTube / Twitch APIs are
  not in the engine — an external script writes the file.
- Official VOICEVOX recipe (`docs/recipes/voicevox.md`).
  `avatar.speak_voicevox("こんにちは")` uses mora lipsync. Engine not bundled.
- Mic extra: `pip install "kagra[mic]"` (`sounddevice`) → `MicLipsync`.
- Bloom: give extract / blur / composite their own uniforms. A single
  `write_buffer` was applied only after submit, so every pass used
  `intensity` as the extract threshold and smeared the whole frame
  (ghost trails next to the VRM).
- 3D physics: Y-up capsule, yaw OBB, layer/mask, triggers, `physics.sync_vrm`.
  Character-controller style (no Rapier). GPU-free tests in `tests/test_physics3d.py`.
- `Camera3D.ray_from_screen` and `avatar.pick(sx, sy)` → humanoid bone name
  (`head`, `leftHand`, …). Bone world spheres; gesture recognition stays out.
- Threshold bloom: extract only high-luminance pixels (eye highlights, outline,
  MToon rimLift), blur that, add back. Full-screen blur is not used — it muddies
  toon edges. `kagra.set_bloom(threshold=0.85, intensity=0.35)`.
- VRM SpringBone colliders (0.x spheres / 1.0 spheres and capsules). Hair and
  skirts now push off the body instead of passing through.
- `VRMC_node_constraint` 1.0: rotation, roll (twist bones), and aim. Applied
  after pose, before skinning.
- VRM 1.0 expression `overrideBlink` / `overrideMouth` / `overrideLookAt`
  (`none` / `block` / `blend`) and `isBinary`. A smiling face can now suppress
  blinks the way the author specified.
- First-person layer: `avatar.first_person = True` / `kagra.set_vrm_first_person`.
  `Auto` meshes drop head-weighted triangles; `ThirdPersonOnly` hides in FP.
- MToon: matcap, UV scroll/rotate (optional mask), and normal maps.
- glTF textures: BMP / TGA / GIF / TIFF in addition to PNG / JPEG. Unknown
  MIME types fall back to magic-byte sniffing.
- SpringBone Verlet + colliders run in `kagra-core` after pose (Python keeps
  parse tests and a fallback). Hair / skirt chains no longer do FK + FFI per
  joint on the Python side.
- Lipsync: `timeline_from_audio_query` / `play_audio_query` consume VOICEVOX
  mora timings instead of throwing the `audio_query` away. WAV Goertzel remains
  the fallback when there is no query.
- Live body ingest: `avatar.apply_pose({name: quat})` / `kagra.set_vrm_pose`.
  Capture stays outside the engine (VR / Kinect / Holistic).
- Drop-in venues: `kagra.stage("venue.glb")` / `kagra.load_gltf` now load
  JSON `.gltf` plus sibling `.bin` / image URIs (not just GLB). A PNG/JPEG
  path becomes an inverted sky sphere. `python -m kagra --stage` /
  `--backdrop` use `assets/stage.glb` when present, else the checkerboard.

## 0.1.2

- Renderer: each skinned draw now gets its own bone-matrix palette. Multi-skin
  VRMs (e.g. the Alicia Solid sample, 12 skins) rendered every mesh with the
  last-uploaded palette, so arms, hands, fingers, and legs stayed frozen in
  bind pose no matter what the animation sent. Single-skin VRoid exports were
  unaffected, which is why it went unnoticed.
- The bundled demo dance is now full-body: alternating arm swings (front and
  side), wrist flicks, head bob and tilt, hip sway, and small steps in place.
  Previously only the spine and both forearms moved.
- `VrmAvatar.relax_hands()` gently curls the fingers. `dance()` applies it
  automatically when a clip has no finger data, so hands no longer look like
  splayed bind-pose paddles during BVH dances.
- sdist: ship `LICENSE` in the tarball. PyPI rejected uploads whose metadata
  lists `License-File: LICENSE` but the file is missing (`v0.1.0` / `v0.1.1`
  wheels still published; only the `.tar.gz` failed).
- README: `pip install kagra` is the full product. Only webcam face tracking
  is an extra (`kagra[facetrack]`); Mac still has no wheel.

## 0.1.1

- Windows first-run: `python -m kagra` loaded the VRM before `run()`, so
  `Renderer not initialized`. `kagra.run(..., on_ready=)` now fires after the
  GPU exists; the demo / README / examples load `avatar()` there.
- `kagra.load_vrma()` / `avatar.load_motion(..., "*.vrma")` / `av.dance("wave.vrma")`
  — VRM Animation (`VRMC_vrm_animation` 1.0). Humanoid retarget (including
  fingers), LookAt yaw/pitch → eye blendshapes, and preset expressions.
  Files from [text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) play as-is.
  See `examples/vrm_vrma.py`.
- Publish / CI: drop macOS runners. Wheels are Linux + Windows only until a Mac
  can verify them (macos-13 also sat queued and blocked `v0.1.0` upload).
- Publish: Linux wheels now request CPython 3.10–3.12 inside manylinux_2_28
  (v0.1.0 failed with “Couldn't find any python interpreters from 'python3'”).

## 0.1.0

First public-facing cut.

- `pip install kagra` wheels via tag-triggered publish (`v*`)
- `python -m kagra` / `kagra` — sing & dance demo; downloads a sample VRM once
- `VrmAvatar.sing()` / `dance()` and a built-in song synthesizer
- `kagra.line()` no longer raises `NameError`
- Engine no longer writes `keymap.json` into the working directory
- Examples use `kagra.font()` instead of a Windows-only Meiryo path
