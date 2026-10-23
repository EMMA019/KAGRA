"""Meteor Cat — the same runner, space rocks and an ember palette. Spec: games/meteor_cat_minimal.json; see docs/runner_kit.md."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from kagra.runner import play  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(play("games/meteor_cat_minimal.json"))
