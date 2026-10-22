"""飛行（ピッチ / バンク / 親子）の純ロジック。GPU も拡張も不要。"""
from tests.conftest import load_kagra_submodule

flight = load_kagra_submodule("flight")
gm = load_kagra_submodule("gameloop")


def test_level_forward_is_plus_z_and_pitch_lifts_nose():
    assert flight.forward_vector(0.0, 0.0, 0.0) == (0.0, 0.0, 1.0)
    fx, fy, fz = flight.forward_vector(0.0, 1.5707963, 0.0)
    assert abs(fx) < 1e-5
    assert fy > 0.99
    assert abs(fz) < 1e-4
    fx, fy, fz = flight.forward_vector(1.5707963, 0.0, 0.0)
    assert fx > 0.99
    assert abs(fy) < 1e-5
    assert abs(fz) < 1e-4


def test_scripted_flight_clears_three_rings():
    state = flight.scripted_flight(seconds=5.0)
    assert state["score"] == 3
    assert state["z"] > 38.0
    assert state["y"] > 10.0
    world = flight.make_world(state)
    assert world.get("genre") is None
    body = next(p for p in world["props"] if p["name"] == "fuselage")
    assert body["local"] is True
    assert body["parent"] == "prop:plane"
    assert body["gltf"] == "biplane"
    hinge = next(p for p in world["props"] if p["id"] == "prop:plane")
    assert hinge["enabled"] is False
    assert "pitch" in hinge and "roll" in hinge
    rings = [p for p in world["props"] if p["name"] == "ring"]
    assert len(rings) == 3
    assert all(p["model"] == "ring" for p in rings)


def test_rgba_to_ppm_header():
    ppm = gm.rgba_to_ppm(bytes([10, 20, 30, 255]) * 4, 2, 2)
    assert ppm.startswith(b"P6\n2 2\n255\n")
    assert len(ppm) == len(b"P6\n2 2\n255\n") + 2 * 2 * 3
