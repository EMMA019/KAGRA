"""SpringBone コライダーの押し出しと VRM 0/1 パース。袖の剛性パス。"""
from tests.conftest import load_kagra_submodule

vs = load_kagra_submodule("vrm_spring")
SpringBone = vs.SpringBone
_as_vec3 = vs._as_vec3
collide_capsule = vs.collide_capsule
collide_sphere = vs.collide_sphere
_Collider = vs._Collider
_Joint = vs._Joint
_Chain = vs._Chain


def test_collide_sphere_outside_unchanged():
    p = collide_sphere([2.0, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0)
    assert abs(p[0] - 2.0) < 1e-6


def test_collide_sphere_pushes_out():
    p = collide_sphere([0.1, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0)
    assert abs(p[0] - 1.0) < 1e-5
    assert abs(p[1]) < 1e-5


def test_collide_sphere_center_uses_fallback():
    p = collide_sphere([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.5, fallback_dir=[0.0, 1.0, 0.0])
    assert abs(p[1] - 0.5) < 1e-5


def test_collide_capsule_mid():
    p = collide_capsule([0.0, 0.5, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.2)
    # 中心線上なので X か Z に押し出される。距離は 0.2
    dist = (p[0] ** 2 + (p[1] - 0.5) ** 2 + p[2] ** 2) ** 0.5
    # ちょうど中心なので fallback Y ではなく、dist がほぼ 0 から押し出し
    # collide_sphere の center フォールバックは Y+
    assert abs(dist - 0.2) < 1e-4 or abs(p[1] - 0.7) < 1e-4 or abs(p[1] - 0.3) < 1e-4


def test_as_vec3_dict_and_list():
    assert _as_vec3({"x": 1, "y": 2, "z": 3}) == [1.0, 2.0, 3.0]
    assert _as_vec3([4, 5, 6]) == [4.0, 5.0, 6.0]


def test_parse_v0_and_v1_colliders():
    sb = object.__new__(SpringBone)
    sb.chains = []
    sb.colliders = []
    sb._nodes = [
        {"name": "root", "t": [0, 0, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [1], "parent": None},
        {"name": "hair", "t": [0, 1, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [2], "parent": 0},
        {"name": "hair2", "t": [0, 1, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [], "parent": 1},
        {"name": "chest", "t": [0, 0.5, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [], "parent": 0},
    ]
    gltf = {
        "nodes": [
            {"name": "root", "children": [1, 3]},
            {"name": "hair", "children": [2]},
            {"name": "hair2"},
            {"name": "chest"},
        ],
        "extensions": {
            "VRM": {
                "secondaryAnimation": {
                    "boneGroups": [{
                        "bones": [1],
                        "colliderGroups": [0],
                        "stiffiness": 1.0,
                        "dragForce": 0.4,
                        "hitRadius": 0.02,
                    }],
                    "colliderGroups": [{
                        "node": 3,
                        "colliders": [{"offset": {"x": 0, "y": 0, "z": 0}, "radius": 0.1}],
                    }],
                }
            },
            "VRMC_springBone": {
                "colliders": [{
                    "node": 3,
                    "shape": {"capsule": {"offset": [0, 0, 0], "tail": [0, 0.2, 0], "radius": 0.08}},
                }],
                "colliderGroups": [{"colliders": [0]}],
                "springs": [{
                    "joints": [
                        {"node": 1, "stiffness": 1.0},
                        {"node": 2, "stiffness": 1.0},
                    ],
                    "colliderGroups": [0],
                }],
            },
        },
    }
    # ノードは __init__ 相当を省略し _ingest 相当だけ
    sb._topo = [0, 1, 2, 3]
    sb._wmats = [[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]] * 4
    vrm0 = gltf["extensions"]["VRM"]
    sa = vrm0["secondaryAnimation"]
    v0_groups = []
    for cg in sa["colliderGroups"]:
        start = len(sb.colliders)
        node = cg.get("node", 0)
        for col in cg.get("colliders", []):
            sb.colliders.append(_Collider(
                node_idx=int(node),
                offset=_as_vec3(col.get("offset")),
                radius=float(col.get("radius", 0.05)),
            ))
        v0_groups.append(list(range(start, len(sb.colliders))))
    for g in sa["boneGroups"]:
        sb._parse_v0(g, v0_groups)
    assert len(sb.colliders) == 1
    assert len(sb.chains) >= 1
    assert sb.chains[0].collider_ids == [0]

    sb1 = gltf["extensions"]["VRMC_springBone"]
    v1_groups = []
    for col in sb1["colliders"]:
        cap = col["shape"]["capsule"]
        sb.colliders.append(_Collider(
            node_idx=int(col["node"]),
            offset=_as_vec3(cap.get("offset")),
            radius=float(cap.get("radius", 0.05)),
            tail=_as_vec3(cap.get("tail")),
        ))
    for cg in sb1["colliderGroups"]:
        v1_groups.append([int(i) for i in cg["colliders"]])
    before = len(sb.chains)
    for sp in sb1["springs"]:
        sb._parse_v1(sp, v1_groups)
    assert len(sb.chains) == before + 1
    assert sb.colliders[-1].tail is not None


def test_parse_v0_leaf_gets_virtual_tail():
    sb = object.__new__(SpringBone)
    sb.chains = []
    sb.colliders = []
    sb._nodes = [
        {"name": "head", "t": [0, 0, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [1], "parent": None},
        {"name": "ribbon_L", "t": [0, 0.2, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [], "parent": 0},
    ]
    sb._parse_v0({"bones": [1], "stiffiness": 2.0, "dragForce": 0.7, "hitRadius": 0.02}, [])
    assert len(sb.chains) == 1
    assert len(sb.chains[0].joints) == 2
    assert not sb.chains[0].joints[0].virtual_tail
    assert sb.chains[0].joints[1].virtual_tail
    assert abs(sb.chains[0].joints[1].bone_length - vs.VIRTUAL_TAIL_LEN) < 1e-6


def _identity():
    return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]


def _translate(x, y, z):
    m = _identity()
    m[12], m[13], m[14] = x, y, z
    return m


def test_stiffness_dt2_does_not_snap_like_paper():
    sb = object.__new__(SpringBone)
    sb._wind = [0.0, 0.0, 0.0]
    sb.colliders = []
    sb._wmats = [_translate(0, 0, 0), _translate(0, 1, 0)]
    sb._nodes = [
        {"name": "p", "t": [0, 0, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [1], "parent": None},
        {"name": "c", "t": [0, 1, 0], "r": [0, 0, 0, 1], "s": [1, 1, 1], "children": [], "parent": 0},
    ]
    parent = _Joint(node_idx=0, stiffness=1.0, drag=0.4, gravity=[0, 0, 0], radius=0.02)
    child = _Joint(
        node_idx=1, stiffness=1.0, drag=0.4, gravity=[0, 0, 0], radius=0.02,
        bone_length=1.0, rest_dir_local=[0.0, 1.0, 0.0],
    )
    child.curr = [0.6, 0.8, 0.0]
    child.prev = [0.6, 0.8, 0.0]
    chain = _Chain(joints=[parent, child], collider_ids=[])
    sb._simulate(chain, 1.0 / 60.0)
    dist = (
        (child.curr[0] - child.target[0]) ** 2
        + (child.curr[1] - child.target[1]) ** 2
        + (child.curr[2] - child.target[2]) ** 2
    ) ** 0.5
    assert dist > 0.25, f"stiffness*dt² must lag, dist={dist}"


def test_sleeve_follow_and_weight_transfer():
    assert vs.sleeve_follow(0.018) < 0.02
    assert vs.sleeve_follow(0.040) > 0.95
    assert vs.is_sleeve_bone_name("J_Sec_L_Sleeve")
    assert vs.is_sleeve_bone_name("袖_L")
    assert not vs.is_sleeve_bone_name("cloth")
    assert not vs.is_sleeve_bone_name("skirt_01_01")
    j, w = vs.transfer_sleeve_weights([3, 0, 0, 0], [1.0, 0.0, 0.0, 0.0], 3, 62, 0.82)
    helper = sum(w[i] for i in range(4) if j[i] == 62)
    arm = sum(w[i] for i in range(4) if j[i] == 3)
    assert abs(helper - 0.82) < 0.02
    assert abs(arm - 0.18) < 0.02
    j2, w2 = vs.transfer_sleeve_weights([3, 1, 2, 4], [0.7, 0.1, 0.1, 0.1], 3, 9, 0.0)
    assert j2 == [3, 1, 2, 4]
    assert abs(w2[0] - 0.7) < 1e-5
