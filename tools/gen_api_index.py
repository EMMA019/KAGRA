#!/usr/bin/env python3
"""kagra 公開 API 索引を自動生成する。

Usage:
    python tools/gen_api_index.py
    python tools/gen_api_index.py --check   # CI: 差分があれば非ゼロ終了

出力: docs/API_INDEX.md

シグネチャは AST のみから作る。Rust 拡張の有無で出力が変わると `--check` が
機能しなくなるため、実行時 import には依存しない。
"""
from __future__ import annotations

import argparse
import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "API_INDEX.md"
KAGRA_PKG = ROOT / "kagra"


def _sig_from_ast(fn: ast.FunctionDef | ast.AsyncFunctionDef) -> str:
    """ソース AST から簡易シグネチャ文字列を作る。"""
    args = fn.args
    parts: list[str] = []

    def fmt_arg(a: ast.arg, default=None):
        name = a.arg
        ann = ""
        if a.annotation is not None:
            try:
                ann = ": " + ast.unparse(a.annotation)
            except Exception:
                ann = ""
        if default is not None:
            # PEP 8: 注釈付きは ` = `、無注釈は `=`
            sep = " = " if ann else "="
            try:
                d = sep + ast.unparse(default)
            except Exception:
                d = sep + "..."
            return f"{name}{ann}{d}"
        return f"{name}{ann}"

    pos = list(args.posonlyargs) + list(args.args)
    defaults = list(args.defaults)
    # defaults は末尾に対応
    nd = len(defaults)
    for i, a in enumerate(pos):
        di = i - (len(pos) - nd)
        default = defaults[di] if di >= 0 else None
        parts.append(fmt_arg(a, default))

    if args.vararg:
        parts.append("*" + fmt_arg(args.vararg))
    elif args.kwonlyargs:
        parts.append("*")

    for a, d in zip(args.kwonlyargs, args.kw_defaults):
        parts.append(fmt_arg(a, d))

    if args.kwarg:
        parts.append("**" + fmt_arg(args.kwarg))

    ret = ""
    if fn.returns is not None:
        try:
            ret = " -> " + ast.unparse(fn.returns)
        except Exception:
            ret = ""
    return f"{fn.name}({', '.join(parts)}){ret}"


def _public_from_init() -> list[tuple[str, str, str]]:
    """kagra/__init__.py の公開 def / 再エクスポート名を列挙。

    Returns list of (name, signature, kind)
    """
    src = (KAGRA_PKG / "__init__.py").read_text(encoding="utf-8")
    tree = ast.parse(src)
    items: list[tuple[str, str, str]] = []

    # __all__ があれば優先
    all_names: set[str] | None = None
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for t in node.targets:
                if isinstance(t, ast.Name) and t.id == "__all__":
                    try:
                        all_names = set(ast.literal_eval(node.value))
                    except Exception:
                        all_names = None

    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("_"):
                continue
            if all_names is not None and node.name not in all_names:
                continue
            items.append((node.name, _sig_from_ast(node), "function"))
        elif isinstance(node, ast.ClassDef):
            if node.name.startswith("_"):
                continue
            if all_names is not None and node.name not in all_names:
                continue
            items.append((node.name, f"class {node.name}", "class"))
        elif isinstance(node, ast.Assign):
            # audio = _Audio() のような公開シングルトン
            for t in node.targets:
                if isinstance(t, ast.Name) and not t.id.startswith("_"):
                    if all_names is not None and t.id not in all_names:
                        continue
                    if t.id in {n for n, _, _ in items}:
                        continue
                    # 関数・クラスと衝突しなければ定数/オブジェクトとして載せる
                    if t.id[0].islower() or t.id.isupper():
                        items.append((t.id, t.id, "object"))

    # from kagra.xxx import Foo の公開名
    for node in tree.body:
        if isinstance(node, ast.ImportFrom) and node.module and node.module.startswith("kagra"):
            for alias in node.names:
                name = alias.asname or alias.name
                if name.startswith("_"):
                    continue
                if all_names is not None and name not in all_names:
                    continue
                if name in {n for n, _, _ in items}:
                    continue
                kind = "class" if name[0].isupper() else "export"
                items.append((name, f"{kind} {name}  (from {node.module})", kind))

    # 名前順
    items.sort(key=lambda x: (x[2] != "function", x[0].lower()))
    return items


# 北極星（VRM に体 / エージェントが 3D ゲームを作る）。棚の奥は 2D・タイル・ECS。
FRONT_NAMES = {
    "AABB",
    "ActionController",
    "AiCharacter",
    "brain",
    "Brain",
    "BrainError",
    "KairiBrain",
    "OpenAIBrain",
    "apply_live_look",
    "apply_room_look",
    "apply_pad",
    "axis",
    "avatar",
    "billboard_mesh",
    "box_mesh",
    "Camera3D",
    "camera_world_to_screen",
    "ChatInbox",
    "CharacterController",
    "cls",
    "cylinder_mesh",
    "describe_environment",
    "destroy",
    "disk_mesh",
    "down",
    "draw_billboard",
    "draw_billboard_instances",
    "draw_mesh_3d",
    "draw_mesh_id",
    "draw_mesh_instances",
    "draw_vignette",
    "draw_vrm",
    "vrm_gpu_stats",
    "EmotionController",
    "ensure_vrm",
    "fill",
    "font",
    "get_camera3d",
    "get_engine",
    "get_screen_size",
    "hovered_prop",
    "init",
    "inject_key",
    "inject_pad",
    "key",
    "LipSyncController",
    "load_json",
    "load_scenario",
    "load_vrma",
    "LookAtController",
    "MicLipsync",
    "pad",
    "pad_pressed",
    "Physics3D",
    "PointerEvent",
    "poll_pad",
    "pressed",
    "Prop",
    "quad_y_mesh",
    "quit",
    "released",
    "render_stats",
    "resolve_asset",
    "RigidBody3D",
    "run",
    "run_scenario",
    "run_scenario_path",
    "save_json",
    "Scene",
    "screenshot",
    "se",
    "set_ambient",
    "set_bloom",
    "set_camera3d",
    "set_exposure",
    "set_fog",
    "set_hdri",
    "set_light_dir",
        "set_mesh_cull",
        "set_mesh_pbr",
        "set_mesh_normal",
    "set_point_light",
    "set_rim",
    "set_shadow_enabled",
    "set_shadow_cascades",
    "set_spot_light",
    "set_toon_params",
    "sky",
    "sound",
    "spatial_mix",
    "play_loop",
    "play_se",
    "set_listener",
    "stop_loop",
    "room",
    "water",
    "island_height",
    "overworld_height",
    "open_world_height",
    "can_pick",
    "height_normal",
    "tile_keys",
    "stair_y",
    "city_boxes",
    "city_chunk",
    "load_city",
    "ramp_mesh",
    "heightfield_mesh",
    "heightfield_tile",
    "solid_tex",
    "sphere_mesh",
    "stage",
    "Stage",
    "StreamHud",
    "text",
    "texture_from_fn",
    "tick_count",
    "tone",
    "unload_mesh_3d",
    "upload_mesh_3d",
    "VirtualCam",
    "VirtualPad",
    "VrmAvatar",
    "Walk",
    "World3D",
    "set_tonemap",
    "apply_outdoor_look",
    "clicked_prop",
    "annotate",
    "debug_trace",
    "debug_trace_summary",
    "DebugTrace",
    "mouse_delta",
    "set_cursor_locked",
    "sound",
    "animate",
    "sequence",
    "Tween",
    "Sequence",
    "Label",
    "Button",
}


def render_markdown(items: list[tuple[str, str, str]]) -> str:
    lines = [
        "# KAGRA Public API Index",
        "",
        "このファイルは `tools/gen_api_index.py` により自動生成されます。手編集しないでください。",
        "",
        f"エントリ数: **{len(items)}**",
        "",
        "棚の**手前**は VRM / 3D ワールド / エージェントゲーム。",
        "棚の**奥**はレガシー 2D・タイルマップ・ECS・エディタ。推奨しない。",
        "",
        "## Front (recommended)",
        "",
        "| Name | Signature |",
        "|---|---|",
    ]
    front = [i for i in items if i[0] in FRONT_NAMES]
    shelf = [i for i in items if i[0] not in FRONT_NAMES]
    for name, sig, _ in front:
        esc = sig.replace("|", "\\|")
        lines.append(f"| `{name}` | `{esc}` |")

    lines += [
        "",
        "## Shelf (legacy 2D / tilemap / editor / ECS)",
        "",
        "| Name | Signature |",
        "|---|---|",
    ]
    for name, sig, _ in shelf:
        esc = sig.replace("|", "\\|")
        lines.append(f"| `{name}` | `{esc}` |")

    lines += [
        "",
        "## Agent notes",
        "",
        "- 存在しない API を呼ばないこと。ここに無い名前は未公開か内部用です。",
        "- 3D ゲームは Front から探す。Shelf の tilemap / ECS / 2D `Camera` は推奨しない。",
        "- `world_to_screen(wx, wy)` は **2D**。3D は `Camera3D.world_to_screen(wx, wy, wz)`。",
        "- セーブは `save_json` / `load_json`。`load_data` はアセットレジストリ。",
        "- VRM が checkout に無いときは `ensure_vrm()`。パスを直書きしない。",
        "- ワンショットポーズは `ActionController`（`ActionController.names()`）。歩きは `avatar.set_locomotion(speed)`（idle/walk/run を速度ブレンド）。ローカル Mixamo は `avatar.bind_locomotion()`（rest+roll 補償。`walk` エイリアス / synthetic_walk.bvh は使わない）。上半身は `play_upper`。`play(\"walk\")` はクリップ切替。`dance()` は全身置換。",
        "- 静的メッシュは `upload_mesh_3d` で一度載せ、`draw_mesh_id` で描く。",
        "- ワールド箱は視錐台カリングされる。箱の描画は `draw_mesh_instances`。直前フレームは `render_stats()`。",
        "- VRM プリミティブはパッド付きボーン AABB でカリング。`doubleSided` のときだけ両面。MToon は裏面法線を反転（頭の中からのリム白飛び / 髪越しの顔を防ぐ）。Hair / 髪 マテリアルだけ `rimLift` を上げる（顔は触らない）。",
        "- 同じパスの `kagra.avatar()` はメッシュ / テクスチャ / MToon を共有する。ジョイントパレットはインスタンスごと。計測は `vrm_gpu_stats()`。見本は `examples/vrm_multi_avatar.py`（Crest Isle は 1 人のまま）。",
        "- 床と箱: `World3D`（または `Physics3D` + `box_mesh`）。カメラは `Camera3D.follow`。",
        "- 短い 3D: `Prop` + `Walk` + `sky()` / `room()` / `water()`。地形は `World3D.set_height_fn` + `island_height` / `overworld_height` / `open_world_height`。タイル化は `tile=` / `stream_radius=`。遠いタイルは `lod_radius=` / `lod_cells=`。拾いは `can_pick`。`Walk(..., jump=)`。キャラコン: `Walk.wish` / `Walk.move` / `Walk.try_jump`（または `CharacterController`）。accel/decel 既定。坂は接平面に接地し、段差は `step_height`。Rapier は入れない。",
        "- 一人称: `Walk(..., first_person=True)`。目線は `eye_height`。ポインタロックは一人称のとき（OS が拒めばフォールバック）。`F` で切替えるデモは Prop Garden。",
        "- ホバー / クリック: `hovered_prop(cam)`。`clicked_prop(cam)` は押下。レイ直打ちは `kagra.play.hovered_prop(ox,oy,oz,dx,dy,dz)`。`plane` は除外。",
        "- エージェントの目: `kagra.annotate(sx, sy)` はプレビュークリックを JSONL に残す（screen / world / bone / Prop id）。`kagra.debug_trace(foot_y=…, height_fn=…)` は接地浮き。エディタではない。「ここもう少し」は数値にする。",
        "- カメラ壁クリップ: `Camera3D.follow(..., world=)` がプレイヤー→カメラの線分を静的箱に当て、当たったら距離を縮める。`min_distance` / `max_distance` で VRM 頭の中と Tiny speck を防ぐ。`Walk` は自動。",
        "- 動く Prop: `p.x` / `set_position` / `vx` + `Prop.update_all(dt)`。消すのは `destroy(p)` か `p.enabled = False`。持つのは `Walk.carry(prop)`。",
        "- `animate(obj, \"y\", end)` / `sequence` / `Tween`。`Prop.update_all` が回す。",
        "- HUD: `Label` / `Button`（画面空間。2D `kagra.ui` の同名は棚）。音は `sound(\"coin\")`。3D は `set_listener` + `play_se(..., x=, y=, z=)` / `play_loop`（距離減衰 + ステレオパン。HRTF ではない）。",
        "- 球 / 円柱の当たりとホバーは AABB ではない。`World3D.add_sphere` / `add_cylinder`。",
        "- Prop テクスチャ: `texture=kagra.texture_from_fn(...)` または `load`。0 なら `color`。",
        "- Prop 親子は 4 段（玄孫まで）。子の `x,y,z,yaw` はローカル。",
        "- glTF 部品: `Prop(\"crate.glb\")`。`stage()` / `load_gltf` は会場。同梱エイリアス `cube.glb`。当たりは AABB。`mesh_hit=True` で三角形。",
        "- ゲームパッド: `axis(\"left\")` / `pad(\"a\")` / `inject_pad`。`Walk` は左スティック移動・右スティック視点。実機 USB/XInput は EventLoop で gilrs（`inject_pad` が優先。CI は inject）。",
        "- 影は床・箱・Prop も落とす。`set_shadow_cascades(2)` で近／遠の 2 段（既定 1。Prop Garden は変えない）。屋外はテクセルスナップ。OSM ではない街 JSON は `load_city`。三角形当たりは `add_trimesh` / `Prop(..., mesh_hit=True)`。積み木は `add_box(..., is_static=False)`（落ちて積もり、Walk が乗る。Rapier クレートは wheel に入れない）。",
        "- 点光源 4: `set_point_light(..., slot=0..3)`。0 がキー（影は無し）。1..3 は埋め。スポットは `set_spot_light(..., slot=)`。室内の透視影はスロット 0 のスポットだけ。平行光は埋め。",
        "- HDRI: `set_hdri(\"studio\")` または正距円筒のパス。拡散は小さな irradiance キューブ。スペキュラは mip LOD。露出は `set_exposure`（既定 1）。ACES は `set_tonemap`（既定オフ）。",
        "- 坂は接平面、急斜面は滑る。接地は小さい足 AABB + 8 点リング + 接平面（太いカプセル AABB の max-Y は浮く。片側 max-Y も浮く。`debug_trace` で測る。Rapier は入れない — AABB で足りる）。デモは Pretty Room / Overworld / Crest Isle。",
        "- 汎用メッシュの金属/粗さ: `upload_mesh_3d(..., metallic=, roughness=)` / `Prop(..., metallic=)` / `set_mesh_pbr`。接空間法線は `normal_texture_id` / `Prop(..., normal=)` / `set_mesh_normal` / glTF `normalTexture`（cotangent frame。ストライドは 32）。MToon は触らない。",
        "- 色付きメッシュ: `solid_tex` + `sphere_mesh` / `cylinder_mesh` / `box_mesh`。",
        "- `kagra-shared` / `mobile/` は別の運転デモ。この Python スタックと混ぜない。",
        "- 頭脳: `kagra.brain(\"kairi\"|\"ollama\"|\"openai\")` / `KairiBrain`。既定は `https://kairi.onrender.com`（チャットは `KAIRI_API_TOKEN`）。モデルは wheel に入れない。`AiCharacter.set_llm_func(mind.ask)`。",
        "- Rust バインディングの整合は `tests/test_api_bindings.py` も参照。",
        "- 再生成: `python tools/gen_api_index.py`",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="既存ファイルと一致するか検証")
    args = ap.parse_args()

    items = _public_from_init()
    text = render_markdown(items)

    if args.check:
        if not OUT.exists():
            print(f"MISSING {OUT}", file=sys.stderr)
            return 1
        cur = OUT.read_text(encoding="utf-8")
        if cur != text:
            print(f"OUTDATED {OUT} — run: python tools/gen_api_index.py", file=sys.stderr)
            return 1
        print(f"OK {OUT} ({len(items)} entries)")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    print(f"Wrote {OUT} ({len(items)} entries)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
