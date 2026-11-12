"""VRM Survivor Loop (dev only) — Moonlit-Swarm-shaped survivor on skinned VRM walkers.

Not a shipping game. The world dump (`vrm_verify_survivor_world.json`, the
canonical copy of the survivor scene first staged by hand in the dev preview)
runs the WorldPlay survivor loop: Mixamo-baked zombie walkers seek the SWAT
player, who auto-fires; XP gems, level-up cards and weapon drops come from
`world_survivor`. The models themselves are external asset supply (Mixamo
terms: no raw-file redistribution), so a repo-only build stages the scene
without them and the release pack is built with an asset root.

    python examples/vrm_verify_survivor_minimal.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from kagra.runner import play  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(play("games/vrm_verify_survivor_minimal.json"))
