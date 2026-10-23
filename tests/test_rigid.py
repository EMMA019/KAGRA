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


def test_body_type_and_kinematic_prop(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    w = _world()
    w["props"] = [
        {
            "id": "prop:wall",
            "type": "prop",
            "name": "wall",
            "model": "box",
            "position": [0.0, 1.0, 0.0],
            "scale": [1.0, 1.0, 1.0],
            "enabled": True,
            "is_static": True,
        },
        {
            "id": "prop:ram",
            "type": "prop",
            "name": "ram",
            "model": "box",
            "position": [2.0, 1.5, 0.0],
            "scale": [0.5, 0.5, 0.5],
            "enabled": True,
            "is_static": True,
            "body": "kinematic",
        },
    ]
    phys = rigid.PhysicsWorld(w)
    assert phys.body_type("prop:wall") == "static"
    assert phys.position("prop:wall") is None
    assert phys.body_type("prop:ram") == "kinematic"
    assert phys.is_kinematic("prop:ram")
    for _ in range(60):
        phys.step(1 / 60)
    assert abs(phys.position("prop:ram")[1] - 1.5) < 0.05
    assert phys.set_kinematic_target("prop:ram", [2.4, 0.4, 0.0])
    assert abs(phys.position("prop:ram")[0] - 2.4) < 1e-3


def test_impulse_force_and_raycast(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    phys = rigid.PhysicsWorld(_world())
    assert phys.apply_impulse("prop:box", [2.0, 0.0, 0.0])
    phys.step(1 / 60)
    assert phys.velocity("prop:box")[0] > 0.2
    x0 = phys.position("prop:box")[0]
    for _ in range(20):
        assert phys.apply_force("prop:box", [30.0, 0.0, 0.0])
        phys.step(1 / 60)
    x1 = phys.position("prop:box")[0]
    assert x1 > x0 + 0.15
    hit = phys.raycast([x1, 8.0, 0.0], [0.0, -1.0, 0.0], 20.0)
    assert hit is not None and hit["id"] == "prop:box"
    assert hit["toi"] > 0.5
    missed = phys.raycast([x1, 8.0, 0.0], [0.0, -1.0, 0.0], 20.0, exclude="prop:box")
    assert missed is None or missed["id"] != "prop:box"
    assert phys.apply_torque_impulse("prop:box", [0.0, 0.5, 0.0])


def test_collision_events_drain(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    phys = rigid.PhysicsWorld(_world())
    for _ in range(240):
        phys.step(1 / 60)
    hits = phys.take_events("collision")
    assert any(
        ev.get("name") == "collision"
        and (ev.get("data") or {}).get("state") == "started"
        and "prop:box" in ((ev.get("data") or {}).get("a"), (ev.get("data") or {}).get("b"))
        for ev in hits
    )
    assert phys.take_events("collision") == []
    contacts = phys.take_events("contact")
    assert any("prop:box" in ((ev.get("data") or {}).get("a"), (ev.get("data") or {}).get("b")) for ev in contacts)


def test_revolute_joint_and_unknown_kind(ks):
    if ks is None:
        pytest.skip("no kagra_shared")
    w = _world(half=12.0)
    w["props"] = [
        {
            "id": "prop:pivot",
            "type": "prop",
            "name": "pivot",
            "model": "box",
            "position": [0.0, 3.15, 0.0],
            "scale": [0.25, 0.25, 0.25],
            "enabled": True,
            "is_static": True,
        },
        {
            "id": "prop:bob",
            "type": "prop",
            "name": "bob",
            "model": "sphere",
            "position": [1.60, 2.57, 0.0],
            "scale": [0.7, 0.7, 0.7],
            "enabled": True,
            "is_static": False,
            "mass": 5.0,
            "friction": 0.4,
            "restitution": 0.05,
        },
    ]
    w["joints"] = [
        {
            "id": "joint:swing",
            "type": "joint",
            "name": "revolute",
            "body_a": "prop:pivot",
            "body_b": "prop:bob",
            "anchor_a": [0.0, 0.0, 0.0],
            "anchor_b": [-1.60, 0.58, 0.0],
            "axis": [0.0, 0.0, 1.0],
        }
    ]
    phys = rigid.PhysicsWorld(w)
    assert phys.body_type("prop:pivot") == "static"
    assert phys.body_type("prop:bob") == "dynamic"
    assert not phys.add_joint("nope", "prop:pivot", "prop:bob", [0, 0, 0], [0, 0, 0])
    for _ in range(180):
        phys.step(1 / 60)
    bob = phys.position("prop:bob")
    dist = (bob[0] ** 2 + (bob[1] - 3.15) ** 2 + bob[2] ** 2) ** 0.5
    assert abs(dist - 1.70) < 0.25, f"revolute holds, dist={dist} bob={bob}"
    assert bob[1] > 0.9


def test_character_controller_slides_on_a_wall(ks):
    if ks is None or not hasattr(ks.PhysicsWorld, "move_character"):
        pytest.skip("no kinematic character controller")
    w = _world(half=8.0)
    w["props"] = [
        {
            "id": "prop:wall",
            "type": "prop",
            "name": "wall",
            "model": "box",
            "position": [1.7, 1.0, 0.0],
            "scale": [1.0, 2.0, 1.0],
            "enabled": True,
            "is_static": True,
        }
    ]
    w["player"] = {"id": "walker:player", "position": [0.0, 1.0, 0.0]}
    phys = rigid.PhysicsWorld(w)
    phys.step(1 / 60)
    blocked = phys.move_character("walker:player", [3.0, -0.2, 0.0], 1 / 60)
    assert blocked is not None
    assert blocked["translation"][0] < 1.4
    assert blocked["position"][0] < 1.5
    assert blocked["grounded"] is True
    assert phys.move_character("prop:wall", [1.0, 0.0, 0.0], 1 / 60) is None
