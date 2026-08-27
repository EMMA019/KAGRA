1. Read docs/API_INDEX.md, docs/AGENT.md, existing rpg.rs / save.rs / world_play.rs / WorldDoc dump schema.
2. SaveGame/session save is the driving demo path; WorldPlay RPG save is WorldDoc dump/load (query/dump/flag).
3. Extended rpg.rs: overlay menu (Space), party hero+ally walkers, talk-grant key into dump slots, held prop, dungeon turn overlay vs enemy, coins=hero HP, indoor 4 lights.
4. Tiny world_play.rs: freeze walk during overlay, restore HP coins after collectathon recount, Complete on win/lose, do not apply platformer checkpoint restore on RPG dumps.
5. GPU-free tests: inventory id, party names, dump roundtrip, turn HP. Other genres still own their dumps.
