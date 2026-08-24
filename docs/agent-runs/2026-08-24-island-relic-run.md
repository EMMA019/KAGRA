# Island Relic Run — agent build log (2026-08-24)

## Goal

30 秒ショーケース「島の遺跡集め」。見知らぬ人に「このエンジンいいかも」「AIエージェントが作った」と思わせる。
Title → Play → Score。屋外 overworld。公開 API のみ。

## Prompt (要約)

Build Island Relic Run on EMMA019/KAGRA via user-Github MCP (no git clone).
Files: `relic_run_rules.py`, `vrm_relic_run.py`, smoke JSON, GPU-free tests,
agent log, README sample lines. Branch `cursor/island-relic-run-qa` → PR to master.

## Session

1. `GetDynamicTools` / GetMcpTools on `user-Github` — ready.
2. Read master references: `vrm_overworld.py`, `vrm_orb_rush.py`, `vrm_prop_garden.py`,
   `kagra/land.py`, `docs/API_INDEX.md`, smoke JSON templates, README samples,
   `prop_garden_rules` / heart-catch / dodge-room test patterns.
3. Verified relic / start / tree / stone XZ against `island_height` + `biome_at`
   (grass above water; avoided west bay / NE mountain).
4. Implemented pure rules + full outdoor Walk/Prop scene (third person only,
   `avatar.set_yaw(walk.face)`, `CAM_DISTANCE=6.6`, procedural glow + `tone` SFX,
   Alicia/Dwango credit in header and title banner).
5. **Blocker:** MCP `create_branch` / `push_files` both failed:

   ```
   POST https://api.github.com/repos/EMMA019/KAGRA/git/refs: 404 Not Found
   ```

   Read APIs (`get_file_contents`, `list_branches`, `search_code`) work as EMMA019.
   Writes that create git refs do not — classic missing Contents:Write on the MCP token.
   Smart-mode retry of `create_branch` returned the same GitHub 404.

## Verify (intended)

```bash
pytest tests/test_relic_run.py
python -m kagra.verify examples/verify_scenarios/relic_run_smoke.json
python examples/vrm_relic_run.py
```

## License notes

- Code: MIT (repo LICENSE).
- Procedural textures / tones only — no third-party art blobs.
- Sample VRM from `ensure_vrm` is Alicia Solid © Dwango; credit when posting shots.

## Status

**PR not opened** — blocked on MCP write scope. Drafts prepared under agent workspace
`/workspace/island_relic_run/` for a follow-up push once Contents:Write is granted.

## Exact MCP write errors

| Call | Error |
|---|---|
| `create_branch` (`cursor/island-relic-run-qa` from `master`) | `POST https://api.github.com/repos/EMMA019/KAGRA/git/refs: 404 Not Found []` |
| `create_branch` + Smart Mode retry | same 404 |
| `push_files` | `failed to create branch from default: ... git/refs: 404 Not Found []` |
| `create_or_update_file` (`scratch/mcp_write_probe.txt` on master) | `PUT .../contents/scratch/mcp_write_probe.txt: 404 Not Found []` |
| `add_issue_comment` (probe) | `403 Must have admin rights to Repository. []` |

Reads (`get_me` → EMMA019, `list_branches`, `get_file_contents`, `search_code`) succeed.
