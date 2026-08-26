# Let an agent build a KAGRA game / エージェントにゲームを作らせる

English first. 日本語は後半。

KAGRA's loop is: search the API → write a scene → verify headlessly.
Rules live in [`AGENTS.md`](../../AGENTS.md). Logged runs live in
[`docs/agent-runs/`](../agent-runs/README.md).

Heart Catch / Switch Room / Dodge Room prove the loop. They are day-one
box games. Do not start a fourth box room and call it D-6. D-6 waits on
the 30s demo test and must be playable for 30s+ with a score or a clear
goal. Engine bar is [`docs/ROADMAP.ja.md`](../ROADMAP.ja.md): 100% = an
agent ships a normal indie game with no human screen; 80% is that minus
net/destruction/cloth/vehicles/GI bake/DOTS/HDRP/human editor/VRM-on-Wasm;
now ~15%. Mountains: signboard (#97) → world as data → one runtime →
game-enough → ship. Old 63% is archived. Brain is `kagra.brain("kairi")`
(default https://kairi.onrender.com, `KAIRI_API_TOKEN`). OSM / extra CSM /
Rapier stay outside 80%. Rigid boxes are AABB
(`add_box(..., is_static=False)`). Ask the world with `world.query` /
`world.dump` before looking at a PNG.

## One-line prompt

```
Using KAGRA, make a short game where a VRM walks three lanes
and catches hearts flying in from the distance. Public APIs only.
Verify with python -m kagra.verify.
```

Reference result of that prompt: `examples/vrm_heart_catch.py`
(log: `docs/agent-runs/20260823-heart-catch/`).

A second, world-shaped prompt:

```
Using KAGRA, make a short 3D game where a VRM walks a room of boxes
and steps on a floor switch. Camera follows. Public APIs only.
Verify with python -m kagra.verify.
```

Result: `examples/vrm_switch_room.py`
(log: `docs/agent-runs/20260823-switch-room/`).

A third, written by a different agent:

```
Using KAGRA, make a short 3D game where a VRM stands in a small arena
and dodges boxes falling from the sky. No catching, no switches —
survive as long as possible while difficulty ramps up. Public APIs
only. Verify with python -m kagra.verify.
```

Result: `examples/vrm_dodge_room.py`
(log: `docs/agent-runs/20260823-dodge-room/`).

## APIs agents actually need

| Job | Call |
|---|---|
| VRM | `kagra.ensure_vrm()` then `kagra.avatar(path)` |
| Move | `avatar.set_position(x, y, z)` / `avatar.set_yaw(rad)` after `avatar.update(dt)` |
| 3D → HUD | `cam.world_to_screen(x, y, z)` — not the 2D `kagra.world_to_screen` |
| World | `World3D` (floor + boxes) then `Camera3D.follow` |
| Short 3D | `Prop` + `Walk` + `sky()` / `room()` / `water()` |
| Island | `World3D.set_height_fn(overworld_height, tile=10, stream_radius=28)` + `load_city` + `apply_outdoor_look` + `Walk(..., jump=)`. Box city JSON, not OSM yet |
| First person | `Walk(..., first_person=True)` — lock + eye height. Prop Garden: `F` / Start |
| Gamepad | `axis("left")` / `pad("a")` / `inject_pad`. `Walk` uses both sticks |
| Hover | `hovered_prop(cam)` — not the 2D `mouse`. Floor `plane` is skipped |
| Move / delete | `prop.x = …` or `vx` + `Prop.update_all(dt)`. `destroy(prop)` / `prop.enabled` |
| Texture / parent | `Prop(..., texture=kagra.texture_from_fn(...))` or `kagra.load`. 4-level `set_parent` / `parent=`. Child `x,y,z,yaw` are local |
| Click / carry | `clicked_prop(cam)` then `walk.carry(prop)` |
| Agent eyes | `kagra.annotate(sx, sy)` click → JSONL. `kagra.debug_trace(foot_y=, height_fn=)` slope float (threshold 0.05). `World3D.update` feeds an active tracer. Not an editor |
| Chase cam | `Camera3D.follow(..., world=)` pulls in before walls |
| Animate / HUD / SE | `animate(prop, "y", end)` / `Label` / `Button` / `sound("coin")` |
| glTF part | `Prop("crate.glb")` — not `stage()`. Bundled `cube.glb`. Collision is AABB |
| Lights | `set_point_light` / `set_spot_light` (`slot=0..3`). Slot 0 is the key (spot shadow) |
| Shape hit | `Prop("sphere")` / `cylinder` collide and hover as those shapes, not boxes |
| Mesh retain | `upload_mesh_3d` once, `draw_mesh_id` each frame — or `world.bake` / `world.draw` |
| Art / SE | `kagra.texture_from_fn` / `kagra.tone` / `kagra.sound` / `kagra.draw_billboard` |
| 3D SE | `set_listener(x,y,z, fx,fy,fz)` then `play_se(path, x=, y=, z=)` / `play_loop` |
| Score | `kagra.save_json` / `kagra.load_json` |
| One-shot pose | `ActionController(avatar)` then `action.play("clap")` — `ActionController.names()` |
| Locomotion blend | `avatar.set_locomotion(speed)` idle/walk/run. Local Mixamo: `bind_locomotion()` (not the `walk` alias). `play_upper("idle")` for spine/arms while legs walk |
| N VRM | same-path `kagra.avatar(path)` shares GPU. `vrm_gpu_stats()`. Example: `vrm_multi_avatar.py` |

## Close the loop

```bash
python -m kagra.verify examples/verify_scenarios/heart_catch_smoke.json
python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json
python -m kagra.verify examples/verify_scenarios/switch_room_smoke.json
python -m kagra.verify examples/verify_scenarios/dodge_room_smoke.json
python -m kagra.verify examples/verify_scenarios/prop_garden_smoke.json
python -m kagra.verify examples/verify_scenarios/pretty_room_smoke.json
python -m kagra.verify examples/verify_scenarios/overworld_smoke.json
python -m kagra.verify examples/verify_scenarios/multi_avatar_smoke.json
```

Save the prompt, the stumbles, and the verify output under
`docs/agent-runs/YYYYMMDD-<slug>/`.

---

# 日本語

一行で渡す:

```
KAGRA で、VRM が3レーンを左右に歩いて、奥から飛んでくるハートを
キャッチする短いゲームを作って。公開 API だけ。verify で閉じて。
```

参照実装とログは上と同じ。3D の投影は `Camera3D.world_to_screen`、
セーブは `save_json`（`load_data` はアセット用）、VRM は `ensure_vrm()`。
箱のある部屋は `World3D` + `Camera3D.follow`。静的メッシュは
`upload_mesh_3d` / `draw_mesh_id`。避けるゲームは
`examples/vrm_dodge_room.py`（ログ: `docs/agent-runs/20260823-dodge-room/`）。
短い 3D は `Prop` + `Walk` + `sky()` / `room()` / `water()`（Garden / Pretty Room /
Overworld。これは play-surface デモで、エージェント製ログではない）。
島は `World3D.set_height_fn(overworld_height, tile=10, stream_radius=28)` と `load_city` と `Walk(..., jump=)`。屋外の影は `set_shadow_cascades(2)`。箱の街 JSON と高さ場。積み木は `add_box(..., is_static=False)`（乗れる）。OSM は 80% の外。
一人称は `Walk(..., first_person=True)`。ホバーは `hovered_prop(cam)`。
パッドは `axis("left")` / `pad("a")`。テストは `inject_pad`。
動かすのは `p.x` か `vx` + `Prop.update_all(dt)`。消すのは `destroy(p)`。
テクスチャは `texture=kagra.texture_from_fn(...)`（または `load`）。
親子は 4 段（`set_parent` / コンストラクタの `parent=`。玄孫まで）。
子の `x,y,z,yaw` は親からのローカル。
局所ライトは `set_point_light(..., slot=0..3)` / `set_spot_light(..., slot=)`。0 がキー。
glTF 部品は `Prop("crate.glb")`（`stage()` は会場。同梱は `cube.glb`）。
球 / 円柱の当たりとホバーは箱ではない。
箱部屋の 4 本目を D-6 と呼ばない。D-6 は 30 秒以上 + スコアかゴール。頭脳は `kagra.brain("kairi")`（既定 kairi.onrender.com）。100% は画面なしでインディーを出荷。80% はそのマイナス。今約 15%。山は看板 → 世界をデータに → ランタイム一つ → ゲームとして足りる → 出荷。旧 63% はアーカイブ。最終目標は第一想起。`world.query` でスクショなしに世界を聞く。
