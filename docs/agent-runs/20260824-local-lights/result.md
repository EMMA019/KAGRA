# Result — local lights 4 slots

- API: `set_point_light(..., slot=)` / `set_spot_light(..., slot=)`.
- Shader: `local_lit` = key * loc_sh + fill. Pairwise `local_four`.
- Tests: `pytest tests -m "not golden"` — **289 passed**, 8 deselected.
- Verify: Windows CI `golden` **passed** (7 tests, including `local_four`). Local-four pixels are closed.

