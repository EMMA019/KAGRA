"""Wheel に同梱する小さなランタイムデータ（ダンス BVH など）。"""
from pathlib import Path

DIR = Path(__file__).resolve().parent


def bundled(name: str) -> Path:
    return DIR / name
