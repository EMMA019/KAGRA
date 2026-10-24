"""Runner kit section of a ``GameSpec``: forward scroll, lanes, spawn-ahead.

Pure data (no engine imports). Mirrors ``RunnerKit`` in
``kagra-shared/src/game_spec.rs``. Defaults are Neon Cat.
"""
from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class SpeedCurve:
    start: float = 10.5
    ramp: float = 0.22
    cap: float = 20.0
    max_step: float = 0.08


@dataclass
class JumpSpec:
    gravity: float = -22.0
    velocity: float = 7.1
    grace_secs: float = 1.6


@dataclass
class LaneSpec:
    count: int = 3
    width: float = 1.2
    limit: float = 2.15
    steer: float = 5.6


@dataclass
class Track:
    z0: float = 0.0
    gap: float = 10.0
    amp: float = 0.0
    freq: float = 1.0


@dataclass
class RingSpec:
    glb: str = "assets/models/kenney/racing-kit/overheadRound.glb"
    track: Track = field(default_factory=lambda: Track(14.0, 17.0, 0.95, 1.2))
    score: int = 100


@dataclass
class GateSpec:
    glb: str = "assets/models/neon_cat/gate_arch.glb"
    z0: float = 10.0
    gap: float = 24.0
    scale_x: float = 2.15


@dataclass
class DrawSpec:
    lead_secs: float = 2.5
    min_obstacles_ahead: int = 5
    fog_start: float = 38.0
    fog_end: float = 110.0


SECTION_KINDS = ("intro", "learn", "challenge", "rest", "reward")
# Share of obstacle slots kept when ``density`` is None (mirrors the engine).
SECTION_DENSITY = {"intro": 0.35, "learn": 0.7, "challenge": 1.0, "rest": 0.0, "reward": 0.0}


BEHAVIORS = ("solid", "jump_over", "roll_under", "rideable", "cat_gap")


@dataclass
class BodySpec:
    """The cat's body for collision (metres above the feet)."""

    stand_height: float = 0.62
    roll_height: float = 0.3
    roll_secs: float = 0.55
    step_tol: float = 0.18
    recover_secs: float = 0.2  # standing back up after a roll: body and pose rise together
    crouch: list[float] = field(default_factory=lambda: [1.05, 0.68, 1.2])  # pose scale at full crouch


@dataclass
class CoinSpec:
    """Coin lines: arcs over hops, low lines through gaps, trails on tops,
    weaving lines in rest/reward sections."""

    enabled: bool = True
    glb: str = "assets/models/coin.glb"
    score: int = 10
    per_line: int = 5
    free_line: int = 8
    spacing: float = 0.85
    arc_height: float = 0.85


@dataclass
class Section:
    """A stretch of track in metres; the list repeats. See docs/runner_kit.md."""

    kind: str = "challenge"
    length: float = 120.0
    level: int = 3
    density: float | None = None
    # Obstacle behaviours this stretch may use; empty = all.
    allow: list = field(default_factory=list)

    def __post_init__(self):
        for b in self.allow:
            if b not in BEHAVIORS:
                raise ValueError(f"section allow {b!r} not in {BEHAVIORS}")
        if self.kind not in SECTION_KINDS:
            raise ValueError(f"section kind {self.kind!r} not in {SECTION_KINDS}")
        if self.length <= 0 or not 0 <= self.level <= 5:
            raise ValueError("section needs length > 0 and level 0..5")
        if self.density is not None and not 0 <= self.density <= 1:
            raise ValueError("section density must be 0..1")


@dataclass
class ScoreSpec:
    """Score = per_meter * distance + per_gate * gates + ring and coin scores."""

    per_meter: float = 2.2
    per_gate: int = 50  # bonus for each stone arch the cat runs through


@dataclass
class RunnerKit:
    speed: SpeedCurve = field(default_factory=SpeedCurve)
    jump: JumpSpec = field(default_factory=JumpSpec)
    lanes: LaneSpec = field(default_factory=LaneSpec)
    obstacle_track: Track = field(default_factory=lambda: Track(26.0, 18.0, 1.2, 0.85))
    rings: RingSpec = field(default_factory=RingSpec)
    gates: GateSpec = field(default_factory=GateSpec)
    draw: DrawSpec = field(default_factory=DrawSpec)
    # Optional pacing script (intro/learn/challenge/rest/reward). Empty = uniform.
    sections: list = field(default_factory=lambda: default_sections())
    body: BodySpec = field(default_factory=BodySpec)
    coins: CoinSpec = field(default_factory=CoinSpec)
    score: ScoreSpec = field(default_factory=ScoreSpec)


def default_sections() -> list:
    """Neon Cat's pacing: teach hops, add rolls, mix everything, then breathe."""
    return [
        Section("intro", 140, 1, None, ["jump_over"]),
        Section("learn", 200, 2, None, ["jump_over", "roll_under"]),
        Section("challenge", 260, 3, None, []),
        Section("rest", 150, 0, None, []),
        Section("challenge", 300, 3, None, []),
        Section("reward", 120, 0, None, []),
    ]
