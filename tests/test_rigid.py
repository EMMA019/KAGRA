"""Rapier 剛体物理（kagra.rigid — Phase 1）のテスト。

kagra_shared に PhysicsWorld（physics feature）がある場合だけ実機検証し、
無ければスキップ（AGENTS.md: テストは拡張非依存が原則。ここは拡張 API の
テストなので存在確認後に実行）。
"""
import pytest

from tests.conftest import load_kagra_submodule

rigid = load_kagra_submodule("rigid")


def _world(**kw):
    world = {
        "version": 1,
        "half": 10.0,
        "floor_y": 0.0,
        "props": [
            {
                "id": "prop:box",
                "type": "prop",
                "name": "box",
                "model": "box",
                "position": [0.0, 5.0, 0.0],
                "scale": [1.0, 1.0, 1.0],
                "enabled": True,
                "is_static": False,
            }
        ],
        "lights": [],
        "cameras": [],
        "heightfield": None,
    }
    world.update(kw)
    return world


@pytest.fixture(scope="module")
def ks():
    try:
        import kagra_shared as ks
    except ImportError:
        return None
    return ks if hasattr(ks, "PhysicsWorld") else None


def test_missing_shared_raises_informative(ks):
    if ks is not None:
        pytest.skip("kagra_shared あり → 実機テストへ")
    with pytest.raises(RuntimeError, match="maturin develop"):
        rigid.PhysicsWorld(_world())


def test_dynamic_box_falls_to_floor(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    phys = rigid.PhysicsWorld(_world())
    assert phys.is_dynamic("prop:box")
    for _ in range(240):
        phys.step(1 / 60)
    y = phys.position("prop:box")[1]
    assert 0.4 < y < 1.6, f"箱は床に落ちて止まる, y={y}"


def test_static_prop_not_dynamic(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    w = _world()
    w["props"][0]["is_static"] = True
    phys = rigid.PhysicsWorld(w)
    assert not phys.is_dynamic("prop:box")
    assert phys.position("prop:box") is None, "静的 prop は position を返さない"


def test_to_world_roundtrip_writes_position(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    phys = rigid.PhysicsWorld(_world())
    for _ in range(240):
        phys.step(1 / 60)
    out = phys.to_world()
    box = [p for p in out["props"] if p["id"] == "prop:box"][0]
    assert 0.4 < box["position"][1] < 1.6


def test_set_velocity_moves_box(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    phys = rigid.PhysicsWorld(_world())
    assert phys.set_velocity("prop:box", [3.0, 0.0, 0.0])
    x0 = phys.position("prop:box")[0]
    for _ in range(30):
        phys.step(1 / 60)
    x1 = phys.position("prop:box")[0]
    assert x1 > x0 + 0.5, f"横に吹き飛ぶ, x0={x0} x1={x1}"


def test_deterministic(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    a = rigid.PhysicsWorld(_world())
    b = rigid.PhysicsWorld(_world())
    for _ in range(180):
        a.step(1 / 60)
        b.step(1 / 60)
    assert a.position("prop:box") == b.position("prop:box")


def _world_with_player(**kw):
    w = _world()
    w["player"] = {
        "id": "walker:player",
        "type": "walker",
        "name": "player",
        "position": [0.0, 1.0, 0.0],
        "on_ground": False,
    }
    w.update(kw)
    return w


def test_walker_is_kinematic_and_keeps_position(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    w = _world_with_player()
    phys = rigid.PhysicsWorld(w)
    assert phys.is_kinematic("walker:player")
    assert not phys.is_dynamic("walker:player")
    for _ in range(240):
        phys.sync_walkers(w)  # ゲームが位置を所有
        phys.step(1 / 60)
    out = phys.to_world()
    assert out["player"]["position"][1] == 1.0, "sync は歩行者位置を上書きしない"


def test_kinematic_walker_pushes_box(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    w = _world_with_player()
    w["props"] = [
        {
            "id": "prop:box",
            "type": "prop",
            "name": "box",
            "model": "box",
            "position": [2.0, 0.6, 0.0],
            "scale": [1.0, 1.0, 1.0],
            "enabled": True,
            "is_static": False,
        }
    ]
    phys = rigid.PhysicsWorld(w)
    px = 0.0
    for _ in range(240):
        px += 0.02
        w["player"]["position"][0] = px
        phys.sync_walkers(w)
        phys.step(1 / 60)
    box_x = phys.position("prop:box")[0]
    assert box_x > 2.2, f"歩行者が箱を押す, box_x={box_x}"


def test_sphere_and_capsule_fall(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    w = _world()
    w["props"] = [
        {
            "id": "prop:ball",
            "type": "prop",
            "name": "ball",
            "model": "sphere",
            "position": [0.0, 3.0, 0.0],
            "scale": [1.0, 1.0, 1.0],
            "enabled": True,
            "is_static": False,
        },
        {
            "id": "prop:pill",
            "type": "prop",
            "name": "pill",
            "model": "capsule",
            "position": [2.0, 4.0, 0.0],
            "scale": [0.6, 1.8, 0.6],
            "enabled": True,
            "is_static": False,
        },
    ]
    phys = rigid.PhysicsWorld(w)
    for _ in range(360):
        phys.step(1 / 60)
    by = phys.position("prop:ball")[1]
    py = phys.position("prop:pill")[1]
    assert 0.4 < by < 0.9, f"球は床に落ちる, by={by}"
    assert 0.2 < py < 1.1, f"カプセルは床に落ちて止まる, py={py}"
