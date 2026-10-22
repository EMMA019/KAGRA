# Fish and cook GenreSession

`fish::tick` and `cook::tick` stay the one-shot `fish_cast` / `cook_stove` loops on `WorldPlay` (held attack catches; attack when the stove is ready cooks). The day loop is a separate `fishcook` genre so those dumps do not change.

`FishCookSession` owns a `WorldDoc` plus phase. It calls new helpers beside the old ticks: `tick_cast` (bite does not land the fish), `tick_reel` (hold inside the moving bar fills, hold outside strains and snaps), `tick_heat` (the pan rises on its own; a tap inside the band plates, a tap below is early, a tap above or a full rise burns). `shop::credit_sale` adds 4 coins. The day timer and the coin goal are session state. Two sales meet the default goal of 8.

Python is `FishCookSession`. The example walks with WASD and the A/D/S/W pads, and uses J / Space / the HOLD pad for cast, reel, plate, and sell. W is screen-forward on the shore camera. Headless play confirms the title once, then the same scripted loop until the goal. HUD text is ASCII.

The pier is `crate.glb`. The yard uses the knight, gazebo, mill (stove), chest (stall), inn, tree, pine, and rock aliases.

`WorldPlay::from_json` on a `fishcook` dump does not start the fish or cook one-shots.

Clippy `-D warnings` wanted `RangeInclusive::contains` for the reel and heat bands, `f32::consts::PI` for the default yaw, and the meter bar packed into one struct.

Lavapipe (`VK_ICD_FILENAMES` = `lvp_icd.json`, `WGPU_BACKEND=vulkan`) drew the reel, the hob, and the day-end card.

Merged `origin/master` after the action / fight sessions landed. Shared files keep those sessions and this one.
