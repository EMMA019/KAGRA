# Result — usable week APIs

- Picture: `set_tonemap` / specular mips / spot perspective shadow / cascade snap.
  Not normals. Not 4-cascade CSM.
- Play: lock, click, `animate`, `Label`/`Button`, carry, coyote, parent 2, `sound`.
  Not USB/XInput.
- Demos: Pretty Room / Overworld / Prop Garden. Garden `KAGRA_SMOKE` pixels stay
  without tonemap.
- Tests: `pytest tests -m "not golden"` — **272 passed**, 3 deselected
- Verify: GPU wheel missing on this VM. Scenarios already exist
  (`pretty_room_smoke.json`, `overworld_smoke.json`, `prop_garden_smoke.json`).
- Checkboxes in `docs/ROADMAP.ja.md` stay open until a stranger would watch 30s.
