# Prompt

Close THIS slice: a water material (e.g. Material::Water) on kagra-shared, two scrolling normals OR procedural waves + Fresnel + IBL reflection (the new env SH is enough; no SSR). Depth fade if cheap. No physics water sim, no caustics required.

WorldDoc: a prop or plane with model/name water (match existing dump style; do not invent a second schema). Crest default may stay as-is unless a small water plane already exists and is readable.

Tiny fixture world JSON with a lake/quad so Emma can try: python -m kagra.play_world kagra-shared/tests/fixtures/water_plane_world.json (or similar). Keep emma_walker and Crest working.

WebGL2: no storage buffers, no base_instance. Vertex3 pos+normal+uv. Instance 2..7 unchanged. Clippy CI Rust 1.98 -D warnings (as_chunks).

Do NOT add Bevy. Do not rewrite vrm_open_world.py. No Rapier, SSAO, GI, Unreal feature list. world_play.rs tiny. Junk never: assets/library/, models/, cache/, mp4, python/, huge HDR. Do NOT invent Python APIs.
