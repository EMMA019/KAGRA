Emma pulled PR #81 (already on master) and played Crest Isle on Windows. Sticky walk after LONG HOLD is FIXED. Three remaining issues from her new clip (images attached, ~38s of python examples/vrm_open_world.py).

Repo: https://github.com/EMMA019/KAGRA. Start from current master (has #81). Do not clone onto a user machine. Investigate; hypotheses below are non-binding.

## What she sees
Image 1: meadow is olive/yellow/BROWN pixelated aerial texture (not paper-white anymore). Kenney pines are GREEN (colormap works). Horizon is thick WHITE/grey fog. No HUD in the capture. Title: VRM Crest Isle.
Image 2: chunky brown/tan tiles, a black void seam on the right, white fog.
Image 3: foreground still pale grey, mid-ground brown, white fog.
Her words: 茶色くなった? 空は相変わらず白い。重すぎるのかな? 長押しバグは直ったけど、長押し→離す→同じ方向キーを押す（反応しない）→もう一度押しなおすと進む。

## Issue 1 (must fix) — re-press ignored after long hold
PR #81 added LONG_HOLD_FRAMES=8 and LONG_REHOLD_FRAMES=16 in kagra-core/src/input.rs. At 60fps that is ~267ms of ignoring non-repeat KEYDOWN after a long hold. A human release-then-tap-same-key lands inside that window, so the first re-press is eaten; the second works after rehold_left expires. That matches her report exactly.

Keep leftover Win32 KEYDOWN-after-KEYUP from sticking (the #80/#81 bug). Do NOT go back to sticky walk.

Fix: leftover KEYDOWNs arrive immediately (same/next few frames). A real re-press is hundreds of ms later. Shrink LONG_REHOLD_FRAMES to ~3–4, OR (better) clear the long block once the key has been fully up with no down for 2 quiet frames — a new KEYDOWN after that silence is a real press. Taps must stay snappy (short #80 window). Test GPU-free:
- long hold, up, 30 frames of immediate down(repeat=false) → NOT held
- long hold, up, 3 quiet frames, then down(repeat=false) → HELD (this is Emma's re-press)
- short tap still re-holds after the 1–2 frame #80 window

## Issue 2 — meadow looks brown dirt, not green grass
Texture is now binding (good). examples/assets/relic_run/polyhaven/aerial_grass_rock_diff_1k.jpg is an aerial GRASS+ROCK photo — brown soil with some green. Combined with #81 Lambert IBL * albedo * 0.35 and lower HDRI, it reads as dirt. User wants 草原 (green meadow), not a desert.
Do not blow it back to white (do not restore additive env * 0.95 or sun -Y).
Options (pick the smallest that looks green): tint terrain mesh_mat.base toward green; multiply albedo in Crest Isle only; or a greener procedural overlay / different CC0 grass. Relic Run shares the jpg — don't ruin that demo. GPU-free: lambert_rgb of tinted grass stays green-dominant (G > R, G > B) and max channel < 0.9 so it doesn't blow white.

## Issue 3 — sky still white
#81 Stage.draw / sky() snapshots fog, draws backdrop with fog off, restores. Emma's clip still has a featureless white/grey sky. Possible: set_fog(enabled=False) not actually clearing shader fog_params.z; puresky sphere still fogged; puresky PNG is pale overcast and looks white; fog_end 102 vs radius 140; draw order; or sky_stage is None. Fix so the sky reads as a sky (clouds/blue of kloofendal puresky), not cls/fog grey-white. Don't disable world fog entirely (distance haze on terrain is OK).

## Issue 4 — heavy / hitch (if cheap)
She asked 重すぎるのかな. Crest streams STREAM_RADIUS tiles + 120+ vista Props. If easy: 1 new tile per frame after the first ring; don't draw_mesh_3d in Prop.__init__ when bake_all will upload. Don't add Rapier. Don't redo slope AABB.

## Constraints
No Unity editor, no Rapier, no new public API unless unavoidable. Don't revert #81 IBL albedo scale or sun +Y. Don't treat SMOKE inject_key as the play bug.

## Verify
python tools/gen_api_index.py --check
pytest tests -m "not golden"
cargo test -p kagra-core --no-default-features --locked input
Open a PR to master. Log under docs/agent-runs/ if that's the convention.
Investigate and discard a wrong hypothesis.

---

Follow-up: Emma confirmed two more play bugs from the same clip. Promote these to must-fix alongside re-press / green meadow / sky.

Image 1 (~0:24): camera is VERY FAR — Alicia is a tiny speck, high downward angle, slope fills the foreground.
Image 2 (~0:30): camera slammed INTO the back of her head. Face features show THROUGH the pink hair (hair backfaces / camera inside the skull). Mesh lines on hair.
Image 3 (~0:36): extreme close-up. Face is a FLAT bright WHITE mask (no skin shading, only purple eyes + brows). Hair and kimono still shade normally. Forehead clipped by the window.

So Camera3D.follow distance is unbounded: far → inside head → face close-up. That also makes the face look broken. Clamp follow distance to the Crest Isle CAM_DISTANCE/CAM_HEIGHT range; don't let wall-clip or mouse-wheel/look delta explode distance or push the eye into the VRM head. Face white-out at close range: check mtoon/lighting after #81 sun +Y and IBL; don't blow skin to white. Hair must not show the face from behind in third-person.

Keep: no Rapier, no Unity editor, don't revert #81 albedo*0.35 IBL or the sticky-walk fix.

---

Full clip (~38s) now described. Extra facts beyond the stills:

- 00:00–00:03 character is completely stationary (matches re-press ignored after long hold).
- 00:03–00:24 walks continuously, no obvious hitch during that stretch.
- 00:10 a large BLACK VOID where ground should be (streaming miss / tile unload / bind-group eviction). 00:13 walks onto smooth untextured solid GREY (not the brown aerial tex).
- 00:24–00:37 stops; camera slams in to the face, orbits, then zooms extremely far; at 00:36 fog whites out the whole scene.
- Trees green, props colored, sky flat white/grey the whole time. No HUD. Window title VRM Crest Isle.

Please also: (a) don't leave holes/black tiles when streaming; (b) check Camera3D.follow distance exploding on look/zoom (wall clip or mouse delta) so far-zoom doesn't turn the world into fog-white. Don't let that delay the re-press + green meadow + real sky fixes.

