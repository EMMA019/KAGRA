# M3 novel pages (slice 1)

Official `python -m kagra.play_world` / wgpu 30 WorldDoc. Capsule/room, not VRM.
Close this slice (not a full VN engine): overlay text pages; Space/click advance;
one 2-way choice writes a dump flag; result/end visible. Title -> play -> result
reuses WorldPlay overlay. Sibling `novel.rs`; do not rewrite `rpg.rs`.
`world_play.rs` touch tiny. Room readable (props / lights 4 slots). Text is overlay
state, not screenshot-only.

Branch: `local/m3-novel-pages` from `local/m3-fight-hitstun`. Do not merge master.
No force-push. No RendererV2 / Rapier / SSAO / GI / new ECS.

Not: branching save editor, portraits atlas, inventory, net, fighting combos, vehicles.
