"""Ursina 級の Prop / Walk — GPU 不要。"""
from __future__ import annotations

import math

import pytest

from pathlib import Path

from tests.conftest import load_kagra_submodule

play = load_kagra_submodule("play")


@pytest.fixture(autouse=True)
def _clear_props():
    play.Prop.clear()
    yield
    play.Prop.clear()


def test_resolve_color_name_and_rgb():
    assert play.resolve_color("gold") == (240, 200, 70)
    assert play.resolve_color((10, 20, 30)) == (10, 20, 30)
    with pytest.raises(ValueError):
        play.resolve_color("not-a-color")


def test_walk_wish_yaw_zero_is_plus_z():
    fx, fz = play.walk_wish(1.0, 0.0, 0.0, speed=2.0)
    assert abs(fx) < 1e-9
    assert abs(fz - 2.0) < 1e-9


def test_walk_wish_yaw_half_pi_is_plus_x():
    fx, fz = play.walk_wish(1.0, 0.0, math.pi / 2, speed=3.0)
    assert abs(fx - 3.0) < 1e-9
    assert abs(fz) < 1e-9


def test_facing_yaw_turns_around_when_walking_toward_camera():
    """S / down is camera-relative back. Face must be π, not camera yaw 0."""
    vx, vz = play.walk_wish(-1.0, 0.0, 0.0, speed=4.2)
    assert abs(vx) < 1e-9
    assert vz < 0.0
    face = play.facing_yaw(vx, vz, fallback=0.0)
    assert abs(abs(face) - math.pi) < 1e-9


def test_facing_yaw_keeps_last_when_still():
    assert play.facing_yaw(0.0, 0.0, fallback=1.2) == 1.2
    assert play.facing_yaw(1.0, 0.0) == pytest.approx(math.pi / 2)


def test_pointer_look_delta_zero_is_not_mouse_pos_fallback():
    """Locked cursor recenters. Absolute pos jump must not pitch the camera down."""
    dx, dy, last = play.pointer_look_delta((0.0, 0.0), (480.0, 270.0), (0.0, 0.0))
    assert dx == 0.0 and dy == 0.0
    assert last is None
    dx, dy, last = play.pointer_look_delta(None, (480.0, 270.0), (0.0, 0.0))
    assert dx == pytest.approx(480.0)
    assert dy == pytest.approx(270.0)
    assert last == (480.0, 270.0)


def test_walk_face_follows_wish_not_camera_yaw():
    w = play.World3D(gravity=0.0)
    w.add_player(0.0, 0.0)
    walk = play.Walk(w, object(), yaw=0.0, speed=2.0)
    assert walk.face == pytest.approx(0.0)
    vx, vz = play.walk_wish(-1.0, 0.0, walk.yaw, walk.speed)
    walk.face = play.facing_yaw(vx, vz, walk.face)
    assert abs(abs(walk.face) - math.pi) < 1e-9
    assert walk.yaw == pytest.approx(0.0)


def test_walk_wish_strafe_is_camera_right():
    """D / 右スティックは画面右。視線 +Z では Camera3D の right が −X。"""
    cam_mod = load_kagra_submodule("camera3d")
    cam = cam_mod.Camera3D(800, 600, fov_deg=52.0)
    for yaw in (0.0, math.pi / 2, math.pi):
        fx, fz = play.walk_wish(0.0, 1.0, yaw, speed=1.0)
        cam.follow(
            0.0, 0.0, 0.0,
            distance=6.2, height=2.6, look_y=1.0, lerp=1.0, yaw=yaw,
        )
        origin = cam.world_to_screen(0.0, 1.0, 0.0)
        moved = cam.world_to_screen(fx, 1.0, fz)
        assert origin is not None and moved is not None
        assert moved[0] > origin[0]


def test_walk_key_axes_down_release_is_zero():
    """Hold ↓ then release → wish is idle. Dual S+↓ still walks until both are up."""
    held: set[str] = {"DOWN"}

    def down(name: str) -> bool:
        return name in held

    assert play.walk_key_axes(down) == (-1.0, 0.0)
    held.clear()
    assert play.walk_key_axes(down) == (0.0, 0.0)
    held.update({"S", "DOWN"})
    assert play.walk_key_axes(down) == (-1.0, 0.0)
    held.discard("DOWN")
    assert play.walk_key_axes(down) == (-1.0, 0.0)
    held.clear()
    assert play.walk_key_axes(down) == (0.0, 0.0)


def test_walk_axes_zero_pad_and_keys_clears_wish():
    """0-axis pad + no keys → (0, 0). Leftover inside deadzone cannot keep walking."""
    assert play.walk_axes(0.0, 0.0, 0.0, 0.0) == (0.0, 0.0)
    assert play.walk_axes(0.05, 0.05, 0.0, 0.0) == (0.0, 0.0)
    fwd, right = play.walk_axes(0.0, 0.04, -1.0, 0.0)
    assert fwd == pytest.approx(-1.0)
    assert right == 0.0
    fwd, right = play.walk_axes(0.0, 0.04, 0.0, 0.0)
    assert fwd == 0.0 and right == 0.0


def test_walk_axes_leftover_stick_does_not_mute_keyboard():
    """Live stick used to skip WASD; leftover then kept walking after ↓ release."""
    fwd, right = play.walk_axes(0.0, 0.5, -1.0, 0.0)
    assert fwd == pytest.approx(-1.5)
    assert right == 0.0


def test_walk_release_clears_player_velocity():
    w = play.World3D(gravity=0.0)
    p = w.add_player(0.0, 0.0)
    p.use_gravity = False
    fwd, right = play.walk_axes(0.0, 0.0, -1.0, 0.0)
    vx, vz = play.walk_wish(fwd, right, 0.0, speed=3.2)
    w.move_player(vx, vz)
    assert vz < 0.0
    fwd, right = play.walk_axes(0.0, 0.0, 0.0, 0.0)
    vx, vz = play.walk_wish(fwd, right, 0.0, speed=3.2)
    w.move_player(vx, vz)
    assert vx == 0.0 and vz == 0.0
    assert p.vx == 0.0 and p.vz == 0.0


def test_look_yaw_subtracts_mouse_x():
    assert play.look_yaw(0.0, 10.0, sens=0.01) == pytest.approx(-0.1)


def test_look_pitch_clamps():
    assert play.look_pitch(0.0, -10.0, sens=0.01) == pytest.approx(0.1)
    assert play.look_pitch(1.19, -10.0, sens=0.01) == pytest.approx(1.2)


def test_jump_vy_ground_water_and_air():
    assert play.jump_vy(True, False, 5.0) == pytest.approx(5.0)
    assert play.jump_vy(False, True, 5.0) == pytest.approx(2.1)
    assert play.jump_vy(False, False, 5.0) is None
    assert play.jump_vy(True, False, 0.0) is None


def test_first_person_eye_looks_along_yaw():
    pos, tgt = play.first_person_eye(0.0, 0.0, 0.0, 0.0, 0.0, eye_height=1.55)
    assert pos == pytest.approx((0.0, 1.55, 0.0))
    assert tgt[2] > pos[2]
    pos_r, tgt_r = play.first_person_eye(0.0, 0.0, 0.0, math.pi / 2, 0.0, eye_height=1.55)
    assert tgt_r[0] > pos_r[0]
    assert abs(tgt_r[2] - pos_r[2]) < 1e-9


def test_hovered_prop_picks_nearest_and_skips_plane():
    play.Prop("plane", x=0, y=0, z=2, scale=20.0, collision=False)
    near = play.Prop("box", x=0, y=0.5, z=2, scale=1.0, collision=False, color="orange")
    far = play.Prop("sphere", x=0, y=0.5, z=6, scale=1.0, collision=False, color="gold")
    hit = play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0)
    assert hit is near
    assert hit is not far
    play.Prop.clear()
    play.Prop("plane", x=0, y=0, z=3, scale=14.0, collision=False)
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None


def test_color_name_roundtrip():
    assert play.color_name("gold") == "gold"
    assert play.color_name((240, 200, 70)) == "gold"
    assert play.color_name((1, 2, 3)) is None


def test_prop_records_center_xform():
    p = play.Prop("box", x=2.0, y=0.5, z=-1.0, scale=(1.2, 1.0, 1.4), color="orange")
    inst = p.instance()
    assert inst[:3] == pytest.approx([2.0, 0.5, -1.0])
    assert inst[3:6] == pytest.approx([1.2, 1.0, 1.4])
    assert p.color == (240, 140, 50)
    assert p in play.Prop._all
    play.Prop.clear()
    assert play.Prop._all == []


def test_prop_world_verts_match_instance_scale():
    p = play.Prop("box", x=1.0, y=2.0, z=3.0, scale=2.0, collision=False)
    verts, _ = play._unit_mesh("box")
    world = p.world_verts(verts)
    xs = [v[0] for v in world]
    ys = [v[1] for v in world]
    assert min(xs) == pytest.approx(0.0)
    assert max(xs) == pytest.approx(2.0)
    assert min(ys) == pytest.approx(1.0)
    assert max(ys) == pytest.approx(3.0)


def test_prop_bake_without_engine_is_zero():
    p = play.Prop("sphere", color="gold", collision=False)
    assert p.bake() == 0
    assert p.mesh_id == 0
    assert play.Prop.bake_all() == [0]


def test_prop_blocks_player_via_world3d():
    w = play.World3D(gravity=0.0)
    play.Prop("box", x=1.2, y=0.5, z=0.0, scale=(0.8, 1.0, 1.6), world=w)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.95
    assert w.box_xforms == []


def test_prop_set_position_moves_collision():
    w = play.World3D(gravity=0.0)
    box = play.Prop("box", x=4.0, y=0.5, z=0.0, scale=(0.8, 1.0, 1.6), world=w)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    box.set_position(1.2, 0.5, 0.0)
    assert box.body.x == pytest.approx(1.2)
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.95


def test_prop_velocity_update_and_destroy():
    w = play.World3D(gravity=0.0)
    box = play.Prop("box", x=0.0, y=0.5, z=2.0, scale=1.0, world=w)
    box.vz = 2.0
    play.Prop.update_all(0.5)
    assert box.z == pytest.approx(3.0)
    assert box.body.z == pytest.approx(3.0)
    play.destroy(box)
    assert box not in play.Prop._all
    assert box.enabled is False
    assert box.body.active is False
    play.destroy(box)


def test_prop_sphere_allows_aabb_corner():
    w = play.World3D(gravity=0.0)
    play.Prop("sphere", x=0.0, y=0.5, z=0.0, scale=1.0, world=w)
    p = w.add_player(2.0, 2.0, radius=0.28, height=1.6)
    p.use_gravity = False
    p.friction = 0.0
    w.move_player(-3.0, -3.0)
    for _ in range(80):
        w.update(0.016)
    dist = math.hypot(p.x, p.z)
    assert dist < 0.92
    assert dist > 0.70
    assert play.Prop._all[0].body.shape == "sphere"


def test_hover_misses_sphere_aabb_corner():
    play.Prop("sphere", x=0.0, y=0.5, z=0.0, scale=1.0, collision=False)
    assert play.hovered_prop(0.45, 2.0, 0.45, 0.0, -1.0, 0.0) is None
    play.Prop("box", x=0.0, y=0.5, z=0.0, scale=1.0, collision=False)
    assert play.hovered_prop(0.45, 2.0, 0.45, 0.0, -1.0, 0.0).model == "box"


def test_hover_hits_cylinder_cap_not_square_corner():
    cyl = play.Prop("cylinder", x=0.0, y=1.0, z=0.0, scale=(0.6, 2.0, 0.6), collision=False)
    assert play.hovered_prop(0.0, 3.0, 0.0, 0.0, -1.0, 0.0) is cyl
    play.Prop.clear()
    play.Prop("cylinder", x=0.0, y=1.0, z=0.0, scale=(0.6, 2.0, 0.6), collision=False)
    assert play.hovered_prop(0.28, 3.0, 0.28, 0.0, -1.0, 0.0) is None


def test_prop_disabled_skipped_by_hover():
    box = play.Prop("box", x=0, y=0.5, z=2, scale=1.0, collision=False)
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is box
    box.enabled = False
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None
    box.enabled = True
    box.destroy()
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None


def test_prop_constructor_parent_is_local():
    parent = play.Prop("box", x=1.0, y=0.0, z=0.0, collision=False)
    child = play.Prop("box", x=2.0, y=0.4, z=0.0, parent=parent, collision=False)
    assert child.x == pytest.approx(2.0)
    assert child.world_x == pytest.approx(3.0)
    assert child.world_y == pytest.approx(0.4)
    assert child.parent is parent


def test_child_world_pose_follows_parent_move_and_yaw():
    parent = play.Prop("box", x=0.0, y=0.5, z=0.0, collision=False)
    child = play.Prop("box", x=2.0, y=0.3, z=0.0, collision=False)
    child.set_parent(parent, keep_world=False)
    assert child.world_x == pytest.approx(2.0)
    parent.x = 1.0
    assert child.world_x == pytest.approx(3.0)
    parent.yaw = math.pi / 2
    assert child.world_x == pytest.approx(1.0)
    assert child.world_z == pytest.approx(-2.0)
    assert child.world_yaw == pytest.approx(math.pi / 2)


def test_set_parent_keep_world_detach():
    parent = play.Prop("box", x=1.0, y=0.0, z=2.0, collision=False)
    child = play.Prop("box", x=3.0, y=0.5, z=2.0, collision=False)
    child.set_parent(parent, keep_world=True)
    assert child.x == pytest.approx(2.0)
    assert child.world_x == pytest.approx(3.0)
    child.set_parent(None, keep_world=True)
    assert child.parent is None
    assert child.x == pytest.approx(3.0)
    assert child.z == pytest.approx(2.0)


def test_jump_vy_coyote_allows_air():
    assert play.jump_vy(False, False, 6.0) is None
    assert play.jump_vy(False, False, 6.0, coyote=True) == pytest.approx(6.0)


def test_set_parent_allows_four_levels_rejects_fifth():
    chain = [
        play.Prop("box", x=float(i), y=0.5, z=0, collision=False)
        for i in range(6)
    ]
    for i in range(4):
        chain[i + 1].set_parent(chain[i], keep_world=False)
    assert play.PARENT_MAX_LEVELS == 4
    assert chain[4].parent is chain[3]
    assert chain[4].world_x == pytest.approx(10.0)
    with pytest.raises(ValueError, match="4 levels"):
        chain[5].set_parent(chain[4])


def test_set_parent_rejects_grafting_subtree_past_four():
    root = [play.Prop("box", x=float(i), y=0.5, z=0, collision=False) for i in range(4)]
    for i in range(3):
        root[i + 1].set_parent(root[i], keep_world=False)
    extra = [
        play.Prop("box", x=10.0 + i, y=0.5, z=0, collision=False) for i in range(2)
    ]
    extra[1].set_parent(extra[0], keep_world=False)
    with pytest.raises(ValueError, match="4 levels"):
        extra[0].set_parent(root[3])


def test_walk_carry_holds_and_clears():
    w = play.World3D(gravity=0.0)
    box = play.Prop("box", x=1.0, y=0.5, z=1.0, collision=False)
    walk = play.Walk(w, object())
    walk.carry(box)
    assert walk.held is box
    walk.carry(None)
    assert walk.held is None


def test_walk_lock_cursor_follows_first_person():
    w = play.World3D(gravity=0.0)
    third = play.Walk(w, object(), first_person=False)
    assert third.lock_cursor is None
    first = play.Walk(w, object(), first_person=True)
    assert first.lock_cursor is None
    off = play.Walk(w, object(), first_person=True, lock_cursor=False)
    assert off.lock_cursor is False


def test_child_collision_follows_parent():
    w = play.World3D(gravity=0.0)
    parent = play.Prop("box", x=4.0, y=0.5, z=0.0, scale=(0.8, 1.0, 1.6), collision=False)
    child = play.Prop("box", x=1.2, y=0.5, z=0.0, scale=(0.8, 1.0, 1.6), world=w)
    child.set_parent(parent, keep_world=True)
    assert child.body.x == pytest.approx(1.2)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.95
    parent.x = 10.0
    assert child.world_x == pytest.approx(7.2)
    assert child.body.x == pytest.approx(7.2)


def test_destroy_parent_destroys_child():
    parent = play.Prop("box", x=0, y=0.5, z=2, collision=False)
    child = play.Prop("box", x=0, y=0.5, z=3, parent=parent, collision=False)
    play.destroy(parent)
    assert parent not in play.Prop._all
    assert child not in play.Prop._all
    assert child.enabled is False
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is None


def test_hover_uses_child_world_position():
    parent = play.Prop("box", x=4.0, y=0.5, z=0.0, scale=0.4, collision=False)
    child = play.Prop("box", x=0.0, y=0.5, z=3.0, scale=1.0, collision=False, color="green")
    child.set_parent(parent, keep_world=True)
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is child
    parent.z = 2.0
    assert child.world_z == pytest.approx(5.0)
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is child


def test_prop_texture_id_bake_without_engine_is_zero():
    p = play.Prop("box", color="orange", texture=7, collision=False)
    assert p.texture == 7
    assert p.bake() == 0
    assert p.mesh_id == 0


def test_prop_normal_id_is_kept():
    p = play.Prop("box", color="orange", normal=9, collision=False)
    assert p.normal == 9
    assert p.normal_tex_id == 0
    assert p.bake() == 0


def test_prop_metallic_defaults_and_override():
    dull = play.Prop("sphere", color="white", collision=False)
    assert dull.metallic == pytest.approx(0.0)
    assert dull.roughness == pytest.approx(1.0)
    chrome = play.Prop("sphere", color="white", collision=False, metallic=1.0, roughness=0.1)
    assert chrome.metallic == pytest.approx(1.0)
    assert chrome.roughness == pytest.approx(0.1)


def test_prop_gltf_unit_cube_matches_box_hit():
    cube = Path(__file__).resolve().parents[1] / "kagra" / "data" / "unit_cube.glb"
    p = play.Prop(str(cube), x=0.0, y=0.5, z=2.0, collision=False, color="white")
    assert p.model == "gltf"
    assert p._mesh_sx == pytest.approx(1.0)
    assert play.prop_hit_extents(p) == pytest.approx((1.0, 1.0, 1.0))
    assert play.hovered_prop(0.0, 0.5, 0.0, 0.0, 0.0, 1.0) is p
    assert p.bake() == 0


def test_prop_mesh_hit_is_trimesh():
    w = play.World3D(gravity=0.0)
    cube = Path(__file__).resolve().parents[1] / "kagra" / "data" / "unit_cube.glb"
    prop = play.Prop(str(cube), x=0.0, y=0.5, z=0.0, world=w, mesh_hit=True)
    assert prop.body is not None
    assert prop.body.shape == "trimesh"
    assert prop.body.tris


def test_prop_gltf_alias_and_collision():
    w = play.World3D(gravity=0.0)
    play.Prop("cube.glb", x=1.2, y=0.5, z=0.0, world=w)
    p = w.add_player(0.0, 0.0, radius=0.28, height=1.6)
    p.use_gravity = False
    w.move_player(5.0, 0.0)
    for _ in range(40):
        w.update(0.016)
    assert p.x < 0.95


def test_prop_gltf_unknown_raises():
    with pytest.raises((ValueError, Exception)):
        play.Prop("definitely_missing_part_xyz.glb", collision=False)
