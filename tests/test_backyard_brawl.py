"""裏庭 3v3 旗取り（Backyard Rascals）の純ロジックテスト。"""
from tests.conftest import load_kagra_submodule

brawl = load_kagra_submodule("backyard_brawl")


def test_scripted_match_steals_and_captures_flag():
    state = brawl.scripted_match(brawl.default_state(), rounds=2)
    assert state["blue_score"] >= 2
    world = brawl.make_world(state)
    assert world.get("genre") is None
    assert len(world["walkers"]) == 6
    assert all(w.get("anim_blend") == 1.0 for w in world["walkers"])
    bat = next(p for p in world["props"] if p["name"] == "bat")
    assert bat["local"] is True
    assert bat["parent"] == "prop:player_hand"


def test_clock_expiry_ends_the_match():
    state = brawl.default_state()
    state["time_left"] = 0.01
    state["blue_score"] = 1
    brawl.step_brawl(state, 1.0 / 60.0, 0.0, 0.0, False, False)
    assert state["phase"] == "result"
    assert state["won"] is True
    assert "BLUE WINS" in state["banner"]


def test_swing_bonks_nearby_red_bot_and_drops_flag():
    state = brawl.default_state()
    p = state["player"]
    foe = next(b for b in state["bots"] if b["team"] == "red")
    foe["x"], foe["z"] = p["x"] + 1.0, p["z"]
    state["red_flag"]["carrier"] = foe["id"]
    ev = brawl.step_brawl(state, 1.0 / 60.0, 0.0, 0.0, True, False)
    assert ev["bonk"] is True
    assert foe["stun"] > 0.0
    assert state["red_flag"]["carrier"] is None
