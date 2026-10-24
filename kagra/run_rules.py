"""Run rules every kit shares: lives, per-run goals and the result banner.

This is the Python mirror of ``kagra-shared/src/game_core/{lives,missions,result}.rs``.
The spec sections (``LivesSpec``, ``MissionsSpec``) live in ``game_spec.py``; this is the
run-time half. ``kagra-shared/tests/fixtures/run_rules_vectors.json`` holds golden vectors
that the Rust tests and ``tests/test_run_rules.py`` both replay, so the two stay equal.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping

from .game_spec import LivesSpec, MissionsSpec

HURT_IDLE = 99.0  # ``hurt_t`` value that means no recent hit


@dataclass
class Lives:
    """Hits left, the blink of protection after one, and the time since it."""

    left: int = 0
    invuln: float = 0.0
    hurt_t: float = HURT_IDLE

    def reset(self, spec: LivesSpec) -> None:
        self.left, self.invuln, self.hurt_t = spec.count, 0.0, HURT_IDLE

    def tick(self, dt: float) -> None:
        self.invuln = max(self.invuln - dt, 0.0)
        self.hurt_t = min(self.hurt_t + dt, HURT_IDLE)

    @property
    def protected(self) -> bool:
        return self.invuln > 0.0

    def hit(self, spec: LivesSpec, last_chance: bool = False) -> str:
        """A hit: ``"absorbed"`` (a life went) or ``"fatal"`` (the run is over)."""
        if last_chance:
            self.left = 1
        if self.left > 1:
            self.left -= 1
            self.invuln = spec.invuln_secs
            self.hurt_t = 0.0
            return "absorbed"
        self.left = 0
        return "fatal"

    def regain(self, spec: LivesSpec) -> bool:
        if self.left < spec.count:
            self.left += 1
            return True
        return False

    def blink_shown(self, spec: LivesSpec) -> bool:
        hz = spec.blink_hz
        return self.invuln <= 0.0 or hz <= 0.0 or int(self.invuln * hz * 2.0) % 2 == 0


@dataclass
class MissionTracker:
    """Which goals are done this run. ``counts`` maps a kind name to the run's number."""

    done: list = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        self.done = []

    def reset(self) -> None:
        self.done = []

    def update(self, spec: MissionsSpec, counts: Mapping[str, int]) -> list:
        """Indices (into ``spec.list``) of goals finished since the last call."""
        if not spec.enabled:
            return []
        self.done += [False] * (len(spec.list) - len(self.done))
        fresh = []
        for i, m in enumerate(spec.list):
            if not self.done[i] and counts.get(m.kind, 0) >= m.target:
                self.done[i] = True
                fresh.append(i)
        return fresh

    def completed(self) -> int:
        return sum(self.done)

    def json(self, spec: MissionsSpec, counts: Mapping[str, int]) -> list:
        if not spec.enabled:
            return []
        return [
            {
                "kind": m.kind,
                "label": m.label or MISSION_LABELS[m.kind],
                "target": m.target,
                "value": min(counts.get(m.kind, 0), m.target),
                "done": i < len(self.done) and self.done[i],
            }
            for i, m in enumerate(spec.list)
        ]


MISSION_LABELS = {"coins": "COINS", "gates": "GATES", "perch": "PERCH", "passes": "PASSED"}


@dataclass
class ResultState:
    """The game-over cue that waits for the freeze, and the banner."""

    gameover_pending: bool = False
    banner: str = ""
    banner_t: float = 0.0

    def reset(self) -> None:
        self.banner_t, self.gameover_pending = 0.0, False

    def announce(self, text: str, secs: float) -> None:
        self.banner, self.banner_t = text, secs

    def tick(self, dt: float) -> None:
        self.banner_t = max(self.banner_t - dt, 0.0)

    def crashed(self) -> None:
        self.gameover_pending = True

    def take_gameover(self, hitstop: float) -> bool:
        if self.gameover_pending and hitstop <= 0.0:
            self.gameover_pending = False
            return True
        return False

    @staticmethod
    def best_shown(best: int, score: int) -> int:
        return max(best, score)
