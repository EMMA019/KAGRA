"""スタジオ 3D チェスの純ロジックテスト。"""
from tests.conftest import load_kagra_submodule

chess = load_kagra_submodule("chess_game")


def test_scripted_opening_advances_six_ply_and_highlights_legal_moves():
    state = chess.scripted_opening()
    assert state["move_count"] == 6
    assert len(state["legal"]) >= 4
    world = chess.make_world(state)
    assert world.get("genre") is None
    tiles = [p for p in world["props"] if p["name"] == "tile"]
    assert len(tiles) == 64
    kings = [p for p in world["props"] if p["name"] in ("piece_w_K", "piece_b_K")]
    assert len(kings) == 2
    assert all(p["gltf"] == "chess_king" for p in kings)
    assert all(p["roughness"] < 0.25 for p in kings)
