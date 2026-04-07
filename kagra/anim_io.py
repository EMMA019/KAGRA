# kagra/anim_io.py
# アニメーションデータの保存・読み込み一元管理
#
# ── 保存フォーマット ──────────────────────────────────────────
#
# anim/
#   {name}_timeline.json      ← Timeline（キーフレームアニメ）
#   {name}_statemachine.json  ← AnimStateMachine（スプライトアニメ）
#   {name}_clips.json         ← AnimationClip 定義群
#
# ── 使い方 ───────────────────────────────────────────────────
#
# 保存:
#   from kagra.anim_io import save_timeline, save_state_machine, save_clips
#   save_timeline(my_timeline, "anim/player_walk")
#   save_state_machine(my_machine, "anim/player_anim")
#   save_clips(my_animator, "anim/enemy_clips")
#
# 読み込み:
#   tl      = load_timeline("anim/player_walk")
#   machine = load_state_machine("anim/player_anim")
#   load_clips_into(my_animator, "anim/enemy_clips")
#
# エンティティの再接続（Timeline のみ）:
#   tl.bind_entities({"Player": player_entity, "Enemy": enemy_entity})

from __future__ import annotations

import json
import os
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.timeline    import Timeline
    from kagra.anim_state  import AnimStateMachine
    from kagra.animation   import Animator


# ── 内部ヘルパー ──────────────────────────────────────────────

def _ensure_dir(path: str):
    d = os.path.dirname(path)
    if d:
        os.makedirs(d, exist_ok=True)


def _json_path(base: str, suffix: str) -> str:
    """'anim/player_walk' + '_timeline' → 'anim/player_walk_timeline.json'"""
    if not base.endswith(".json"):
        base = base + suffix + ".json"
    return base


# ── Timeline ──────────────────────────────────────────────────

def save_timeline(timeline: "Timeline", base_path: str):
    """Timeline を JSON に保存する。

    Args:
        timeline:  保存する Timeline オブジェクト
        base_path: 拡張子なしのパス（例: "anim/player_walk"）
                   → "anim/player_walk_timeline.json" に保存される
    """
    path = _json_path(base_path, "_timeline")
    _ensure_dir(path)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(timeline.to_dict(), f, ensure_ascii=False, indent=2)
    return path


def load_timeline(base_path: str, entity_map: dict = None) -> "Timeline":
    """JSON から Timeline を読み込む。

    Args:
        base_path:   拡張子なしのパス（または完全なJSONパス）
        entity_map:  {"entity_name": entity_obj, ...}  省略時はバインドしない

    Returns:
        Timeline オブジェクト（entity_map を渡すと target が自動接続される）
    """
    from kagra.timeline import Timeline
    path = _json_path(base_path, "_timeline")
    with open(path, "r", encoding="utf-8") as f:
        tl = Timeline.from_dict(json.load(f))
    if entity_map:
        tl.bind_entities(entity_map)
    return tl


# ── AnimStateMachine ──────────────────────────────────────────

def save_state_machine(machine: "AnimStateMachine", base_path: str):
    """AnimStateMachine のステート定義を JSON に保存する。

    Args:
        machine:   保存する AnimStateMachine
        base_path: 拡張子なしのパス（例: "anim/player_anim"）
    """
    path = _json_path(base_path, "_statemachine")
    _ensure_dir(path)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(machine.to_dict(), f, ensure_ascii=False, indent=2)
    return path


def load_state_machine(base_path: str) -> "AnimStateMachine":
    """JSON から AnimStateMachine を読み込む。

    Note:
        tileset や tex_dict は保存されないため、ロード後に設定する必要がある。

        machine = load_state_machine("anim/player_anim")
        machine._tileset = my_tileset   # 再接続
        machine.play("idle_front")
    """
    from kagra.anim_state import AnimStateMachine
    path = _json_path(base_path, "_statemachine")
    with open(path, "r", encoding="utf-8") as f:
        return AnimStateMachine.from_dict(json.load(f))


# ── AnimationClip (Animator) ──────────────────────────────────

def save_clips(animator: "Animator", base_path: str):
    """Animator のクリップ定義を JSON に保存する。

    Args:
        animator:  保存する Animator
        base_path: 拡張子なしのパス（例: "anim/enemy_clips"）
    """
    path = _json_path(base_path, "_clips")
    _ensure_dir(path)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(animator.clips_to_dict(), f, ensure_ascii=False, indent=2)
    return path


def load_clips_into(animator: "Animator", base_path: str):
    """JSON から Animator にクリップ定義を読み込む（既存クリップは保持）。"""
    path = _json_path(base_path, "_clips")
    with open(path, "r", encoding="utf-8") as f:
        animator.load_clips_from_dict(json.load(f))


# ── 一括保存 / 読み込み ───────────────────────────────────────

def save_all(
    base_path: str,
    timeline:      "Timeline"         = None,
    state_machine: "AnimStateMachine" = None,
    animator:      "Animator"         = None,
) -> list[str]:
    """複数のアニメデータをまとめて保存する。

    Returns:
        保存したファイルパスのリスト
    """
    saved = []
    if timeline is not None:
        saved.append(save_timeline(timeline, base_path))
    if state_machine is not None:
        saved.append(save_state_machine(state_machine, base_path))
    if animator is not None:
        saved.append(save_clips(animator, base_path))
    return saved


def list_saved(directory: str) -> dict[str, list[str]]:
    """指定ディレクトリ内の保存済みアニメファイルをリストアップする。

    Returns:
        {"timeline": [...], "statemachine": [...], "clips": [...]}
    """
    result = {"timeline": [], "statemachine": [], "clips": []}
    if not os.path.isdir(directory):
        return result
    for fname in os.listdir(directory):
        path = os.path.join(directory, fname)
        if fname.endswith("_timeline.json"):
            result["timeline"].append(path)
        elif fname.endswith("_statemachine.json"):
            result["statemachine"].append(path)
        elif fname.endswith("_clips.json"):
            result["clips"].append(path)
    return result
