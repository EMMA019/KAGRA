Work on Emma's Windows PC. Do NOT clone. Repo D:\program\kagra (EMMA019/KAGRA).

Current origin/master is a2398aa. Implement ROADMAP GAPS so RPG actually CLOSES.

This slice CLOSES remaining RPG (item 4):
- menu as overlay state
- party: at least 2 named members visible in dump
- inventory: talk-grant an item; dump-visible slots; using/holding queryable
- save: roundtrip party+inventory+flags+scene via WorldDoc dump
- turn combat: small encounter, two sides, pick an action, HP in dump, win/lose

Reuse official save/query/dump/flag. No new ECS, RendererV2, VRM, Rapier/SSAO/GI.
