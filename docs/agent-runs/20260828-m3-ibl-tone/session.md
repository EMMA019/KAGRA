# Session

- Branch: `master` at `a6f618f` (Emma.vrm play dump). No clone. Worked on Emma Windows PC `D:\program\kagra`.
- Inspected kagra-core V2 `set_hdri` / `set_tonemap` / `set_exposure`, `hdri.rs` studio equirect + 8^2 irradiance cube, `shaders.rs` `aces_tonemap` + `env_irr` sample.
- Official path stays kagra-shared wgpu 30 `play_world`. Did not add Bevy, RendererV2, a second renderer, cubemap bind, storage buffers, or 4K HDR.
- Port: `Globals.env` (x=IBL 0.35, y=exposure 1, w=ACES) in `shader3d.wgsl` + `render/mod.rs`. Procedural SH L1 hemisphere (studio sky/ground), Narkowicz ACES (same formula as V2). Metal GGX locals stay; toon IBL is irr*albedo*0.35.
- Optional WorldDoc `ibl` / `exposure` / `tonemap` (serde default, omit = outdoor ON). `compile_scene` fills Scene3D. Collectathon/driving get the same 3-line defaults. `world_play.rs` untouched.
- Vertex3 location 0/1/8 and instance 2..7 unchanged. Relic Run UV defaults stay. No Rapier / SSAO / GI.
