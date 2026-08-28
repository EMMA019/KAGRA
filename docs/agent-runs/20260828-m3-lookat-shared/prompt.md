# Prompt

Close THIS slice on Emma PC `D:\program\kagra` (do not clone). origin/master was 1fb0000 (morph/blendshapes).

Parse VRM 0 firstPerson.lookAt / VRM 1 VRMC_vrm.lookAt enough to yaw/pitch head (and eyes if cheap) toward the play_world camera. Rest/roll compensation if neck bones (same Mixamo rule, not raw bind*delta). Dump-visible look yaw/pitch on the walker. Keep morph blink, spring hair, Mixamo, MToon, albedo, CPU skin. Tiny fixture: vrm_walker_world.json is enough if the VRM has a head bone. Crest Isle stays capsule. wasm/WebGL2: no storage buffers, no base_instance. Clippy CI Rust 1.98 -D warnings.

NOT RendererV2. Do not rewrite examples/vrm_open_world.py. No Rapier, SSAO, second renderer. world_play.rs tiny. Genre loops untouched. Push origin master when green.
