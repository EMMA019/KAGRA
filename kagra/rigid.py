"""Rapier 剛体物理（Python 側ラッパー）。汎用エンジン化 Phase 1。

ゲームロジックは Python のみ。`kagra_shared.PhysicsWorld`（Rust + Rapier）
を dict 世界で使えるように包む:

    from kagra.rigid import PhysicsWorld

    phys = PhysicsWorld(world_dict)     # props の is_static=false が動的剛体に
    phys.step(1 / 60)                   # 落ちる・積もる・衝突
    world_dict = phys.to_world()        # 位置が書き戻される

- `is_static: true`（既定）の prop → 壁・床・景観（動かない）
- `is_static: false` の prop → 落下・衝突・積み重なる動的剛体
- walker（player / walkers）→ カプセル剛体（床に立ち、箱に乗る）
- 地形は高さ場コライダー（`height_at` をサンプリング）
- 決定論: `enhanced-determinism`（同入力 → 同結果）

kagra_shared が無い環境では案内メッセージを出す（`kagra.gameloop` と同じ
方針）。旧 `kagra.physics`（2D ECS）・`kagra.physics3d`（AABB 自作）とは
別モジュール。
"""
from __future__ import annotations

from typing import Any

try:
    import kagra_shared as _ks
except ImportError:  # pragma: no cover - 未ビルド環境は案内
    _ks = None

__all__ = ["PhysicsWorld"]


def _missing_message() -> str:
    return (
        "kagra.rigid は kagra_shared（Rust 拡張、rapier3d）が必要です。"
        "kagra-shared で `maturin develop --features python,physics` を実行してください。"
    )


class PhysicsWorld:
    """Rapier 剛体ワールド（dict 世界 ↔ 剛体）。"""

    def __init__(self, world: dict[str, Any]) -> None:
        self._inner = self._build(world)

    @staticmethod
    def _build(world: dict[str, Any]):
        if _ks is None:
            raise RuntimeError(_missing_message())
        return _ks.PhysicsWorld.from_json(_json_dumps(world))

    def step(self, dt: float) -> None:
        """1 フレーム進める。`dt` は秒（1/60 など）。"""
        self._inner.step(float(dt))

    def to_world(self) -> dict[str, Any]:
        """剛体位置を書き戻した世界 dict。"""
        import json

        return json.loads(self._inner.to_json())

    def position(self, prop_id: str) -> list[float] | None:
        """動的剛体の現在位置 [x, y, z]。静的 or 未登録は None。"""
        p = self._inner.position(prop_id)
        return list(p) if p is not None else None

    def is_dynamic(self, prop_id: str) -> bool:
        return bool(self._inner.is_dynamic(prop_id))

    def set_velocity(self, prop_id: str, v: list[float]) -> bool:
        """動的剛体の速度を設定（投げる / 吹き飛ばす）。"""
        return bool(self._inner.set_velocity(prop_id, [float(x) for x in v]))

    def set_position(self, prop_id: str, p: list[float]) -> bool:
        """動的剛体の位置を直接設定（テレポート / リスポーン）。"""
        return bool(self._inner.set_position(prop_id, [float(x) for x in p]))


def _json_dumps(world: dict[str, Any]) -> str:
    import json

    return json.dumps(world, ensure_ascii=False)
