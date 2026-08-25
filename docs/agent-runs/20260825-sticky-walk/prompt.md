# Prompt

Emma (Windows, D:\program\kagra) says Crest Isle feels better (slope sit) but keys still stick: release WASD/arrows and the avatar **keeps walking**. Confirm and fix on latest master of https://github.com/EMMA019/KAGRA (includes #71 sticky-walk, #79 slope AABB).

## Already shipped (#71) — still happening
#71 merged 2026-08-24: ignore OS key-repeat for held; release_all on focus loss; Unidentified arrows; Walk keyboard+pad add; pad 0-axis = released. Tests: tests/test_play.py, tests/test_pad.py, rust input late_repeat_after_up_does_not_rehold.

She still sees it on Windows after pulling today's master. Intermittent originally; now she reports it again while playing Crest Isle.

## Distinguish two bugs (do not assume)
1. **Input:** `held` / `kagra.key("DOWN")` still true after Released, or pad leftover wish. Then Walk wish is non-zero.
2. **Physics after #79:** wish is (0,0) but vx/vz persist (slope slide / snap-to-plane / leftover velocity). That would feel like sticky walk on hills even with perfect input.

Measure with existing debug_trace or a GPU-free test: after key-up, wish must be 0 AND horizontal speed must die immediately on flat ground. On a slope, wish 0 must not keep walking like a held key (a tiny slide-to-stop is OK; continued walk is not).

Windows-specific hypotheses to verify (discard if wrong): winit repeat flag; PhysicalKey::Unidentified on JIS; key-up dropped while pointer-locked; S vs ArrowDown; gilrs leftover stick; Walk.step not overwriting move_player when wish is 0.

## Done when
- Releasing any walk key stops immediately on flat ground, every time.
- GPU-free regression covers the actual remaining path, not only the #71 cases.
- pytest -m "not golden"; cargo test input if you touch Rust.
- PR. Do not redo slope AABB or kagra.stage. No Rapier.

Emma is on Windows; prefer winit paths that match that.
