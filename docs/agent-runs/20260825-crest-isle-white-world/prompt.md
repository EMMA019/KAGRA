Fix two Crest Isle play issues Emma captured on Windows after pulling EMMA019/KAGRA master (PR #80 already merged). Repo: https://github.com/EMMA019/KAGRA. Demo: python examples/vrm_open_world.py. Local play is Windows at D:\program\kagra. Investigate yourself; treat the hypotheses below as non-binding.

## What Emma sees (clip attached)
Two frames from her ~19s play recording of the VRM Crest Isle window:
- Image 1 (early): ground is a flat white/grey plane (not green grass). Kenney pine trees are white silhouettes. Sky is featureless pale grey (no Poly Haven puresky). Some Nature Kit props still show teal/pink/green. Alicia VRM is visible. Title bar: VRM Crest Isle.
- Image 2 (later): still washed out. Coins (bronze) visible. Same white ground / white trees / grey sky.
She said: loading feels a bit bad; grassland is white; after a LONG key hold, stopping is quite delayed (長押しすると止まるのがだいぶ送れる).

This is NOT the CrestIsle SMOKE inject_key("W") missing down=False path. SMOKE only runs when KAGRA_SMOKE=1 and quits ~40 frames. She is playing for real.

## Constraints
- No Unity/Tk visual editor. Agent eyes: kagra.annotate() / kagra.debug_trace() already on master.
- Do not add Rapier.
- Do not redo slope foot AABB (PR #79) or the kagra.stage unshadow (PR #78) unless you prove they are the cause.
- No new public API unless unavoidable.
- Shared engine fix is OK (Crest Isle + Relic Run share grass jpg, puresky, Lambert).

## Issue A — white world / slow load (primary)
Assets ARE in git:
- Grass: examples/assets/relic_run/polyhaven/aerial_grass_rock_diff_1k.jpg (~667KB). Crest Isle _poly() looks there.
- Sky: same folder kloofendal_48d_partly_cloudy_puresky_1k.png. kagra.stage + set_hdri.
- Kenney forest colormap: examples/assets/open_world/kenney/forest/Textures/colormap.png (10KB). Nature Kit uses vertex baseColorFactor (those props still show color in the clip). Forest/town/castle GLBs need the external colormap — those trees are white in the clip.

Crest Isle on_enter: bake_terrain(tex) then apply_outdoor_look() then set_hdri(puresky, strength=0.95) + fog (48,102,(150,175,195)) + bloom(0.80,0.28) + set_light_dir + spot 1.15 + point 0.40.
apply_outdoor_look() already set_hdri("studio", 0.35), set_exposure(1.08), set_tonemap(True), bloom, 2 cascades.
SHADER_3D Lambert: rgb = albedo*lit + albedo*local_lit + hemi + env_irr*cam.env.x, then *exposure, then ACES. env.x=0.95 IBL on top of sun + lights can blow mid-green grass and muted colormap to white while saturated vertex colors survive. Default/toon from PR #76 (cam.toon.y < 0.999 uses mix(shade, lit) with lit=toon.w which can be >1).
Also check: GLTF loader failing to resolve Textures/colormap.png next to the GLB (classic Kenney white); kagra.load(jpg) returning a white default; terrain tiles not uploaded (_upload_tile skips if _terrain_tex<=0 — that would be MISSING ground, not a white plane, so a white plane means a mesh IS drawing).
Loading hitch: Prop.bake_all + many Kenney GLBs + stream_tiles on first frames.

Goal A: Crest Isle meadow reads as green grass, Kenney trees have colormap (not white), sky is the puresky sphere not a blank grey. Nature Kit vertex-colored props stay. Don't destroy Relic Run / Pretty Room look. GPU-free tests where possible; golden only if you must and then update with reason.

## Issue B — delayed stop after long hold
PR #80 is on master. input.rs rehold_block only lasts this frame + next. Test windows_keyup_then_nonrepeat_down_next_frame_does_not_rehold then explicitly allows a non-repeat KEYDOWN on the frame AFTER that to re-hold ("a real re-press after the block window must work"). On Win32, a long hold queues auto-repeat; after WM_KEYUP a leftover KEYDOWN can arrive with repeat=false MORE than 1-2 frames later, especially if a load hitch stalled begin_frame. That would look like: hold a long time, release, avatar keeps walking then eventually stops.
Also check kagra/play.py wish-idle vx/vz snap (skip if steep-sliding) and Walk animation using vx^2+vz^2 > 0.04.

Goal B: after releasing WASD/arrows following a long hold, wish goes idle immediately and walk stops on flat ground (tiny slope settle OK). Real re-press after a short pause must still start walking. Don't break pad add / IME scan pairing from #80.

## Verify
python tools/gen_api_index.py --check
pytest tests -m "not golden"
cargo test -p kagra-core --no-default-features --locked input
Add/adjust GPU-free tests for: (1) colormap path next to forest GLB is resolvable / Prop or gltf loader finds it; (2) outdoor look does not force env strength that blows albedo (or Crest Isle uses a sane IBL/exposure); (3) rehold_block covers a multi-frame post-up non-repeat KEYDOWN burst after a long hold, but a real press later still holds.
Open a PR to master. Log under docs/agent-runs/ if the repo convention wants it.
Invite yourself to investigate and discard a wrong hypothesis.
