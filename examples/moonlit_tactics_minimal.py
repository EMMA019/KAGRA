"""Moonlit Tactics: made from the Starter template (scripts/new_kit.py).

The sim is Rust (kagra-shared/src/moonlit_tactics.rs); the page is
``scripts/preview_site/play.html?game=moonlit_tactics``. The game is data: this file only checks the
shipped world, there is no Python module for the kit.

    python examples/starter_minimal.py --check   # the world is well formed and its files exist

The Rust test ``the_baked_fixture_is_the_default_world`` keeps the world equal to the default
spec; bake it with ``KAGRA_BAKE=1 cargo test -p kagra-shared --lib moonlit_tactics::``.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GAME_ID = "moonlit_tactics"
WORLD = ROOT / f"kagra-shared/tests/fixtures/{GAME_ID}_world.json"


def referenced_files(node):
    """Every string in the world that looks like a repo asset path."""
    if isinstance(node, dict):
        for value in node.values():
            yield from referenced_files(value)
    elif isinstance(node, list):
        for value in node:
            yield from referenced_files(value)
    elif isinstance(node, str) and node.startswith("assets/") and "." in node.rsplit("/", 1)[-1]:
        yield node


def check() -> list[str]:
    world = json.loads(WORLD.read_text(encoding="utf-8"))
    problems = []
    if world.get("genre") != GAME_ID:
        problems.append(f"genre must be {GAME_ID}")
    game = world.get("game")
    if not isinstance(game, dict) or game.get("id") != GAME_ID:
        problems.append(f"world.game.id must be {GAME_ID}")
    for rel in sorted(set(referenced_files(world))):
        if not (ROOT / rel).is_file():
            problems.append(f"missing file {rel}")
    return problems


def main(argv: list[str]) -> int:
    if argv != ["--check"]:
        print(__doc__)
        return 2
    problems = check()
    for p in problems:
        print("error:", p)
    print(f"{GAME_ID} world ok" if not problems else f"{len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:] or ["--check"]))
