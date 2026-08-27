# Prompt

Close THIS slice on play_world: player can place a tower (click or J on a pad/slot).
Placement dump-visible (new tower entity or count). Cost: placing spends coins;
cannot place when coins are insufficient. Cost/remaining coins dump-visible.
Keep existing path + auto-fire. No clone. No SSAO/extra lights. No new ECS /
RendererV2 / VRM / Rapier. world_play.rs stays tiny. Do not rewrite other loops.
Junk never add: assets/library/, assets/models/, cache/, examples/*.mp4, python/
