# Changelog

## Unreleased

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
