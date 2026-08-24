# Let an agent build a KAGRA game / エージェントにゲームを作らせる

English first. 日本語は後半。

KAGRA's loop is: search the API → write a scene → verify headlessly.
Rules live in [`AGENTS.md`](../../AGENTS.md). Logged runs live in
[`docs/agent-runs/`](../agent-runs/README.md).

Heart Catch / Switch Room / Dodge Room prove the loop. They are day-one
box games. Do not start a fourth box room and call it D-6. Next bar:
usable week in [`docs/ROADMAP.ja.md`](../ROADMAP.ja.md).

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
| Island | `World3D.set_height_fn(overworld_height, tile=10, stream_radius=28)` + `load_city` + `Walk(..., jump=)`. `set_shadow_cascades(2)` outdoors. Not OSM |
| First person | `Walk(..., first_person=True)` — eye height. Prop Garden: `F` / Start |
| Gamepad | `axis("left")` / `pad("a")` / `inject_pad`. `Walk` uses both sticks |
| Hover | `hovered_prop(cam)` — not the 2D `mouse`. Floor `plane` is skipped |
| Move / delete | `prop.x = …` or `vx` + `Prop.update_all(dt)`. `destroy(prop)` / `prop.enabled` |
| Texture / parent | `Prop(..., texture=kagra.texture_from_fn(...))` or `kagra.load`. 1-level `set_parent` / `parent=` (no grandchildren). Child `x,y,z,yaw` are local |
| glTF part | `Prop("crate.glb")` — not `stage()`. Bundled `cube.glb`. Collision is AABB |
| Shape hit | `Prop("sphere")` / `cylinder` collide and hover as those shapes, not boxes |
| Mesh retain | `upload_mesh_3d` once, `draw_mesh_id` each frame — or `world.bake` / `world.draw` |
| Art / SE | `kagra.texture_from_fn` / `kagra.tone` / `kagra.draw_billboard` |
| Score | `kagra.save_json` / `kagra.load_json` |
| One-shot pose | `ActionController(avatar)` then `action.play("clap")` — `ActionController.names()` |

## Close the loop

```bash
python -m kagra.verify examples/verify_scenarios/heart_catch_smoke.json
python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json
python -m kagra.verify examples/verify_scenarios/switch_room_smoke.json
python -m kagra.verify examples/verify_scenarios/dodge_room_smoke.json
python -m kagra.verify examples/verify_scenarios/prop_garden_smoke.json
python -m kagra.verify examples/verify_scenarios/pretty_room_smoke.json
python -m kagra.verify examples/verify_scenarios/overworld_smoke.json
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
島は `World3D.set_height_fn(overworld_height, tile=10, stream_radius=28)` と `load_city` と `Walk(..., jump=)`。屋外の影は `set_shadow_cascades(2)`。OSM / Rapier ではない。
一人称は `Walk(..., first_person=True)`。ホバーは `hovered_prop(cam)`。
パッドは `axis("left")` / `pad("a")`。テストは `inject_pad`。
動かすのは `p.x` か `vx` + `Prop.update_all(dt)`。消すのは `destroy(p)`。
テクスチャは `texture=kagra.texture_from_fn(...)`（または `load`）。
親子は 1 段（`set_parent` / コンストラクタの `parent=`。孫は不可）。
子の `x,y,z,yaw` は親からのローカル。
glTF 部品は `Prop("crate.glb")`（`stage()` は会場。同梱は `cube.glb`）。
球 / 円柱の当たりとホバーは箱ではない。
箱部屋の 4 本目を D-6 と呼ばない。次はロードマップの使える週。
