# Result — crawl pixels + rigid boxes

- Crawl: pairwise `outdoor_crawl` / `_nudge` / `_off`. CPU snap holds.
- Rigid: AABB equal. `Walk` stands on stacked crates. No Rapier crate.
- Tests: `pytest tests -m "not golden"` — **292 passed**, 9 deselected.
- Verify: GPU wheel missing on this VM. Crawl pixels close on Windows CI `golden`.
