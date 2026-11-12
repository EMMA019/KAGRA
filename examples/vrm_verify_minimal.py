"""VRM Verify (dev only) — skinned VRM humanoid + Mixamo locomotion check.

Not a shipping game. The world dump puts the embedded T-pose humanoid VRM
(`tpose_humanoid.vrm`, a clip-less humanoid that `mixamo::bind_locomotion`
retargets the bundled Mixamo walk onto) in a WorldPlay world, plus a second
walker frozen mid-stride. WASD walks the player; the walk clip, the idle
blend back to rest, and the per-frame re-skinning are what is being verified.

    python examples/vrm_verify_minimal.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from kagra.runner import play  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(play("games/vrm_verify_minimal.json"))
