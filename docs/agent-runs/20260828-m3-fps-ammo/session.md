1. Read docs/API_INDEX.md, kagra-shared/src/fps.rs, world_play.rs, WalkInput, survival/sim coins-as-meter pattern.
2. Mag is dump `coins` (MAG=6). Fire spends a round. Empty click is dump-visible `empty` and does not hitscan.
3. Official control: R (WalkInput.dodge). Reload delay RELOAD_TIME, dump-visible `reload`, then mag fills.
4. Tiny world_play.rs: restore MAG after collectathon recount on new/start; clear dodge after fps tick. No new ECS/RendererV2/VRM/Rapier/SSAO.
5. GPU-free tests: spend round, empty click does not kill, R reload, eye camera + hitscan kept. Other genres still own their dumps.