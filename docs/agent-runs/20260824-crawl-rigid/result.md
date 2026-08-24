# Result — crawl pixels + rigid boxes

- Crawl: pairwise `outdoor_crawl` / `_nudge` / `_off`. CPU snap holds.
- Rigid: AABB equal. `Walk` stands on stacked crates. No Rapier crate.
- Tests: `pytest tests -m "not golden"` — **292 passed**, 9 deselected.
- Verify: Windows CI `golden` **passed** (#65, 17/17). Crawl pixels closed.
  Rigid AABB closed on #64. Picture ~85%, engine ~63%. 30s demos still open.
