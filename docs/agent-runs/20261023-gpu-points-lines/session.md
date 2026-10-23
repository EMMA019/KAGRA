# Session

Dump fields `points` and `lines` (`PointCloud`, `FatLine`). `compile_scene` copies them onto the draw list. The renderer expands points to camera-facing quads and lines to clip-space quads, one instanced draw each, additive, depth-test, no depth write, emissive mask so 8-bit bloom can see them. Caps 2048. Python `point_cloud` / `fat_line` only build the dicts. Neon samples a ribbon behind the ship. Props named `trail` stay absent. CPU `Emitter` and `Trail` are unchanged.
