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
    glb: str = "assets/models/quaternius/modular-dungeons-pack/Arch.glb"
    z0: float = 10.0
    gap: float = 24.0
    scale_x: float = 2.15


@dataclass
class DrawSpec:
    lead_secs: float = 2.5
    min_obstacles_ahead: int = 5
    fog_start: float = 38.0
    fog_end: float = 110.0


@dataclass
class RunnerKit:
    speed: SpeedCurve = field(default_factory=SpeedCurve)
    jump: JumpSpec = field(default_factory=JumpSpec)
    lanes: LaneSpec = field(default_factory=LaneSpec)
    obstacle_track: Track = field(default_factory=lambda: Track(26.0, 14.0, 1.2, 0.85))
    rings: RingSpec = field(default_factory=RingSpec)
    gates: GateSpec = field(default_factory=GateSpec)
    draw: DrawSpec = field(default_factory=DrawSpec)
