"""Rapier 剛体物理（Python 側ラッパー）。ロジックは Rust（`kagra_shared.PhysicsWorld`）。

    from kagra.rigid import PhysicsWorld

    phys = PhysicsWorld(world_dict)
    phys.step(1 / 60)
    world_dict = phys.to_world()        # 位置・姿勢が書き戻される

- `body`: `static` / `dynamic` / `kinematic`。省略時は `is_static`
- `collider`: `box` / `sphere` / `capsule` / `cylinder` / `cone` / `none`
- `joints`: `fixed` / `revolute` / `spherical`（アンカーは各剛体のローカル）
- `apply_impulse` / `apply_force` / `raycast` / `take_events`
- walker（player / walkers）はキネマティックカプセル。`sync` は上書きしない
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
        if _ks is None or not hasattr(_ks, "PhysicsWorld"):
            raise RuntimeError(_missing_message())
        return _ks.PhysicsWorld.from_json(_json_dumps(world))

    def step(self, dt: float) -> None:
        """1 フレーム進める。`dt` は秒（1/60 など）。

        歩行者（player / walkers）を動かすゲームは、step の前に
        ``sync_walkers(world_dict)`` を呼ぶ（キネマティック剛体の位置更新）。
        """
        self._inner.step(float(dt))

    def sync_walkers(self, world: dict[str, Any]) -> None:
        """歩行者のゲーム側位置をキネマティック剛体へ押し込む。

        WorldPlay の WASD 移動 / Python の位置更新と共存するため、毎フレーム
        ``step`` の前に呼ぶ。呼ばないと歩行者は前回位置に留まる。
        """
        self._inner.sync_walkers(_json_dumps(world))

    def set_walker_position(self, prop_id: str, p: list[float]) -> bool:
        """歩行者 1 体の位置を押し込む（``sync_walkers`` の単体版）。"""
        return bool(self._inner.set_walker_position(prop_id, [float(x) for x in p]))

    def to_world(self) -> dict[str, Any]:
        """剛体位置を書き戻した世界 dict。"""
        import json

        return json.loads(self._inner.to_json())

    def position(self, prop_id: str) -> list[float] | None:
        """動的・キネマティック剛体の位置 [x, y, z]。静的 or 未登録は None。"""
        p = self._inner.position(prop_id)
        return list(p) if p is not None else None

    def is_dynamic(self, prop_id: str) -> bool:
        return bool(self._inner.is_dynamic(prop_id))

    def is_kinematic(self, prop_id: str) -> bool:
        """歩行者、または ``body: kinematic`` の prop。"""
        return bool(self._inner.is_kinematic(prop_id))

    def set_velocity(self, prop_id: str, v: list[float]) -> bool:
        """動的剛体の速度を設定（投げる / 吹き飛ばす）。"""
        return bool(self._inner.set_velocity(prop_id, [float(x) for x in v]))

    def set_position(self, prop_id: str, p: list[float]) -> bool:
        """動的剛体のテレポート、またはキネマティック prop の位置。

        歩行者は ``set_walker_position`` / ``sync_walkers``。
        """
        return bool(self._inner.set_position(prop_id, [float(x) for x in p]))

    def set_kinematic_target(self, prop_id: str, p: list[float]) -> bool:
        """キネマティック prop または歩行者の次姿勢。"""
        return bool(self._inner.set_kinematic_target(prop_id, _vec3(p)))

    def apply_impulse(self, prop_id: str, impulse: list[float]) -> bool:
        """動的剛体へ瞬間的な力積（kg·m/s）。"""
        return bool(self._inner.apply_impulse(prop_id, _vec3(impulse)))

    def apply_force(self, prop_id: str, force: list[float]) -> bool:
        """動的剛体へ力（ニュートン）。次の step まで蓄積される。"""
        return bool(self._inner.apply_force(prop_id, _vec3(force)))

    def apply_torque_impulse(self, prop_id: str, torque: list[float]) -> bool:
        """動的剛体へ瞬間的な角力積。"""
        return bool(self._inner.apply_torque_impulse(prop_id, _vec3(torque)))

    def velocity(self, prop_id: str) -> list[float] | None:
        v = self._inner.velocity(prop_id)
        return list(v) if v is not None else None

    def angular_velocity(self, prop_id: str) -> list[float] | None:
        v = self._inner.angular_velocity(prop_id)
        return list(v) if v is not None else None

    def rotation(self, prop_id: str) -> list[float] | None:
        """オイラー角 ``[yaw, pitch, roll]``。``WorldProp`` と同じ規約。"""
        r = self._inner.rotation_euler(prop_id)
        return list(r) if r is not None else None

    def body_type(self, prop_id: str) -> str | None:
        """``static`` / ``dynamic`` / ``kinematic``。未登録は None。"""
        kind = self._inner.body_type(prop_id)
        return str(kind) if kind is not None else None

    def add_joint(
        self,
        kind: str,
        body_a: str,
        body_b: str,
        anchor_a: list[float],
        anchor_b: list[float],
        axis: list[float] | None = None,
    ) -> bool:
        """``fixed`` / ``revolute`` / ``spherical``。アンカーは各剛体のローカル。"""
        ax = [0.0, 1.0, 0.0] if axis is None else _vec3(axis)
        return bool(
            self._inner.add_joint(
                kind,
                body_a,
                body_b,
                _vec3(anchor_a),
                _vec3(anchor_b),
                ax,
            )
        )

    def take_events(self, name: str) -> list[dict[str, Any]]:
        """``collision`` または ``contact`` を取り出して空にする。"""
        import json

        out: list[dict[str, Any]] = []
        for raw in self._inner.take_events(name):
            out.append(json.loads(raw))
        return out

    def raycast(
        self,
        origin: list[float],
        direction: list[float],
        max_toi: float = 100.0,
        exclude: str | None = None,
    ) -> dict[str, Any] | None:
        """最近傍ヒット。``direction`` は内部で正規化される。``toi`` はメートル。

        ブロードフェーズは ``step`` で更新されるので、1 step 後に呼ぶ。
        ``exclude`` は発射体の id。
        """
        hit = self._inner.raycast(_vec3(origin), _vec3(direction), float(max_toi), exclude)
        if hit is None:
            return None
        ident, toi, point, normal = hit
        return {
            "id": str(ident),
            "toi": float(toi),
            "point": list(point),
            "normal": list(normal),
        }


def _vec3(v: list[float]) -> list[float]:
    return [float(v[0]), float(v[1]), float(v[2])]


def _json_dumps(world: dict[str, Any]) -> str:
    import json

    return json.dumps(world, ensure_ascii=False)
