"""Click annotations — GPU 不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule


def _ann():
    return load_kagra_submodule("annotate")


def test_make_note_omits_missing_fields():
    m = _ann()
    rec = m.make_note(120.0, 80.0, timestamp=1.5)
    assert rec["sx"] == 120.0
    assert rec["sy"] == 80.0
    assert rec["timestamp"] == 1.5
    assert "wx" not in rec
    assert "bone" not in rec
    assert "prop_id" not in rec


def test_make_note_has_world_bone_prop():
    m = _ann()
    rec = m.make_note(
        10, 20,
        world=(1.0, 2.0, 3.0),
        bone="head",
        prop_id=7,
        screenshot="scratch/a.png",
        note="ここもう少し右",
        timestamp=9.0,
        frame=12,
    )
    assert rec["wx"] == 1.0 and rec["wy"] == 2.0 and rec["wz"] == 3.0
    assert rec["bone"] == "head"
    assert rec["prop_id"] == 7
    assert rec["screenshot"] == "scratch/a.png"
    assert rec["note"] == "ここもう少し右"
    assert rec["frame"] == 12


def test_append_jsonl(tmp_path):
    m = _ann()
    path = tmp_path / "notes.jsonl"
    m.append_jsonl({"sx": 1, "sy": 2}, path)
    m.append_jsonl({"sx": 3, "sy": 4}, path)
    lines = path.read_text(encoding="utf-8").strip().splitlines()
    assert len(lines) == 2
    assert '"sx": 3' in lines[1]


def test_plane_hit_center():
    m = _ann()
    hit = m.plane_hit((0.0, 2.0, 0.0), (0.0, -1.0, 0.0), y=0.0)
    assert hit is not None
    assert abs(hit[1]) < 1e-6
    assert abs(hit[0]) < 1e-6


def test_height_hit_slope():
    m = _ann()

    def h(x, z):
        return 0.2 * x

    hit = m.height_hit((0.0, 1.0, 0.0), (1.0, -0.5, 0.0), h, max_dist=8.0, steps=40)
    assert hit is not None
    assert abs(hit[1] - h(hit[0], hit[2])) < 0.08


def test_annotate_from_cam_ray_floor(tmp_path):
    cam_m = load_kagra_submodule("camera3d")
    m = _ann()
    cam = cam_m.Camera3D(800, 600, fov_deg=45.0)
    cam.position = (0.0, 2.0, 4.0)
    cam.target = (0.0, 0.0, 0.0)
    rec = m.annotate(
        400, 300, cam=cam, persist=True, path=tmp_path / "a.jsonl", timestamp=0.0,
    )
    assert rec["sx"] == 400
    assert "wx" in rec
    assert rec["wy"] <= 0.05
    assert (tmp_path / "a.jsonl").is_file()


def test_annotate_prop_id():
    play = load_kagra_submodule("play")
    play.Prop.clear()
    m = _ann()
    p = play.Prop("box", x=0.0, y=0.5, z=2.0, scale=1.0)
    rec = m.annotate(
        0, 0,
        origin=(0.0, 0.5, 0.0),
        direction=(0.0, 0.0, 1.0),
        persist=False,
        timestamp=0.0,
    )
    assert rec["prop_id"] == p.id
    play.Prop.clear()


def test_annotate_fake_bone():
    m = _ann()

    class Av:
        def pick(self, sx, sy, camera=None):
            return "leftHand"

    rec = m.annotate(8, 9, avatar=Av(), persist=False, timestamp=0.0)
    assert rec["bone"] == "leftHand"
