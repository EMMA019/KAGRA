# Result — indoor umbra / parent 4 / normal pairwise

- Shader: spot-owned umbra mix 0.16. Golden: side lamp, non-casting floor (extent skip).
- Play: `PARENT_MAX_LEVELS = 4`. Grafting a subtree past 4 is rejected.
- Goldens: `indoor_spot` restaged; pairwise `normal_bump` added.
- Rust: side-lamp NDC matches the golden; floor skip; near-cascade snap holds on 0.2 texel eye move.
- Tests: `pytest tests -m "not golden"` — **288 passed**, 7 deselected.
- Verify: Windows CI `golden` **passed** (6 golden tests, including `indoor_spot` and `normal_bump`). Indoor and normal pixels are closed.

