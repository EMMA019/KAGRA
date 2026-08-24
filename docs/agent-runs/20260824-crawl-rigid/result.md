# Result — crawl pixels + rigid boxes

- Crawl: pairwise `outdoor_crawl` / `_nudge` / `_off`. CPU snap holds.
- Rigid: AABB equal. `Walk` stands on stacked crates. No Rapier crate.
- Tests: `pytest tests -m "not golden"` — **292 passed**, 9 deselected.
- Verify: GPU wheel missing on this VM. First CI `golden` **failed**
  (`outdoor_crawl` mean_abs=0.000). Scene restaged to orbit + side sun.
  Crawl pixels still close only when that job is green.
