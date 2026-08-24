# Result — local lights 4 slots

- API: `set_point_light(..., slot=)` / `set_spot_light(..., slot=)`.
- Shader: `local_lit` = key * loc_sh + fill. Pairwise `local_four`.
- Tests: `pytest tests -m "not golden"` — **289 passed**, 8 deselected.
- Verify: GPU wheel missing. Close is Windows CI `golden` (`local_four` and still `indoor_spot`).
- Do not call 4 local lights or indoor pixels closed until that job is green.
