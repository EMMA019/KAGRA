1. Read docs/API_INDEX.md, ROADMAP/REVIEW, puzzle.rs, puzzle_pad_world.json, WorldProp.parent, WalkInput.attack, fps ray (private; not extracted).
2. Rapier not required: parent id + kinematic offset is a fixed joint; look-ray vs sensor AABB is a line query. No new dump fields.
3. Lid (`parent: prop:crate`) follows crate after push. Click/J along facing opens latch (`name` open, player `ray`). Box-on-pad kept.
4. world_play.rs untouched. Tests: lid follows, ray opens, pad still solves, other genres own dumps. Clippy/lib tests/pytest/wasm32 green.