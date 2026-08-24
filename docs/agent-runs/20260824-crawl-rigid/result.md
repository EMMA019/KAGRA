# Result — crawl pixels + rigid boxes

- Crawl: pairwise `outdoor_crawl` / `_nudge` / `_off`. CPU snap holds.
- Rigid: AABB equal. `Walk` stands on stacked crates. No Rapier crate.
- Tests: `pytest tests -m "not golden"` — **292 passed**, 9 deselected.
- Verify: GPU wheel missing on this VM. CI `golden` **failed twice**
  (`outdoor_crawl` mean_abs=0.000). Cause: 2-cascade shadow writes shared
  one VP uniform. Each layer now has its own buffer. Crawl pixels still
  close only when that job is green.
