---
name: kagra-agent
description: >-
  Build, verify and ship KAGRA phone browser games (Rust kits on kagra-shared, JSON worlds and
  manifests, the preview page). Use when editing kagra-shared, games/*.json,
  scripts/preview_site, or when the user mentions KAGRA, a kit, Moonlit Swarm, HOP-BEAT or
  Neon Cat.
---

# KAGRA agent loop

The rules are in [`AGENTS.md`](../../../AGENTS.md); read it first. The short version:

1. A new game: `python scripts/new_kit.py <slug> "<Title>"`, then change the `GAME:` parts
   (`docs/making_a_game.md`). No genre branches in `play.html`; page features come from the
   manifest's `page` block.
2. Reuse `game_core` / `hud_kit` parts; move a reusable part into `game_core`.
3. Close the loop: fmt, clippy (with and without `render`), `cargo test -p kagra-shared`,
   related `pytest`, `node scripts/browser_smoke.mjs dist/preview <slug>`, a screenshot. Then
   ask the owner to try the phone preview link.
4. Assets CC0 / original / OFL, all in `assets/LEDGER.json`; no Quaternius; ask before paid.
5. Small PRs, Japanese report to the owner, merge only with their OK, never force-push.
6. 「直してここ」: `python scripts/repro_issue.py <issue>` before editing.

Coordinates: right-handed, Y up; with the camera looking toward -Z, screen right is +X and
screen up is -Z. Read `coordinates.note` and each camera's `screen_right` in a dump.

The old desktop / VRM agent guide: `old/docs-archive/cursor-SKILL-2026-10-legacy.md`.
