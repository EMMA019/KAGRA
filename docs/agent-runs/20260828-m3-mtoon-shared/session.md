# Session

- origin/master was 3719382 (Mixamo rest+roll).
- Parsed VRM 1 VRMC_materials_mtoon and VRM 0 materialProperties (_ShadeColor / _ShadeToony) onto MeshData.mtoon.
- Material::Toon = 5. Shared shader half-Lambert mix(shade * albedo, albedo, t) plus a few-line global rim. Hair rimLift leftover V2.
- Instance locations 2..7 unchanged; UV 8; MToon shade+toony at location 9 offset 96. No storage buffers, no base_instance.
- walk_skinned.gltf got VRMC_materials_mtoon; walk_skinned.vrm got VRM0 shade fields. Crest dump still no walker glTF.
- examples/vrm_open_world.py and RendererV2 untouched. world_play.rs untouched. Mixamo / spring / look-at / morph leftover.
