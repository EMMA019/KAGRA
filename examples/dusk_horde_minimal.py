"""Dusk Horde — Moonlit-Swarm-shaped survivor on skinned VRM walkers.

The world dump (`dusk_horde_world.json`, the canonical copy of the
survivor scene first staged by hand in the dev preview) runs the
WorldPlay survivor loop: Mixamo-baked zombie walkers seek the SWAT
player, who auto-fires; XP gems, level-up cards and weapon drops come
from `world_survivor`. The models themselves are external asset supply
(Mixamo terms: no raw-file redistribution), so a repo-only build stages
the scene without them and the release pack is built with an asset
root.

    python examples/dusk_horde_minimal.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from kagra.runner import play  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(play("games/dusk_horde_minimal.json"))
