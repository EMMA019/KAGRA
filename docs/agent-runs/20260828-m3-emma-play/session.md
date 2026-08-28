# Session

Started from origin/master `01cabc1` on Emma's Windows PC (`D:\program\kagra`). Did not clone.

`assets/Emma.vrm` is on disk (~16.9 MB) and gitignored (`*.vrm`). Dump `kagra-shared/tests/fixtures/emma_walker_world.json` sets walker `gltf` to repo-relative `assets/Emma.vrm` (same path as existing `resolve_alias("emma")`). No `D:\`.

`load_skinned` / `gltf_mesh_for` resolve that path from cwd or `CARGO_MANIFEST_DIR/..` so cargo tests (crate cwd) and `play_world` (repo root) both find the file. Missing file falls back to bundled clip-less `tpose_humanoid.vrm` (Mixamo rest+roll already binds in `skinned_from_doc`).

`python -m kagra.play_world` with no args still defaults to Crest Isle capsule collectathon.

Did not rewrite `examples/vrm_open_world.py`. No Rapier / SSAO / second renderer / new ECS.
