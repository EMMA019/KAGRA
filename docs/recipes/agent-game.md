# Let an agent build a KAGRA game / エージェントにゲームを作らせる

English first. 日本語は後半。

KAGRA's loop is: search the API → write a scene → verify headlessly.
Rules live in [`AGENTS.md`](../../AGENTS.md). Logged runs live in
[`docs/agent-runs/`](../agent-runs/README.md).

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
| Short 3D | `Prop` + `Walk` + `sky()` — not 2D `Entity`. See `examples/vrm_prop_garden.py` |
| First person | `Walk(..., first_person=True)` — eye height. Prop Garden: `F` |
| Hover | `hovered_prop(cam)` — not the 2D `mouse`. Floor `plane` is skipped |
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
短い 3D は `Prop` + `Walk` + `sky()`（`examples/vrm_prop_garden.py`。
これは play-surface デモで、エージェント製ログではない）。
一人称は `Walk(..., first_person=True)`。ホバーは `hovered_prop(cam)`。
