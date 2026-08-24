# Result — indoor spot shadow + tonemap pairwise

- Shader: spot-owned 2048 map multiplies the local light (`params.y`), not the sun.
- Goldens: `indoor_spot` / `tonemap_on` / `ibl_metal` pairwise (no committed PNG).
- Tests: `pytest tests -m "not golden"` — **287 passed**, 6 deselected
- Verify: GPU wheel missing on this VM. Close is the Windows CI `golden` job.
- Checkboxes in `docs/ROADMAP.ja.md` stay open until a stranger would watch 30s.
  Indoor / tonemap / IBL items wait on CI green, then still not the 30s test.
