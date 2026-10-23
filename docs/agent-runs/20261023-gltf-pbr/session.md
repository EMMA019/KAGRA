The color PR stayed on `cursor/linear-color-2c16`. This import is `cursor/gltf-pbr-2c16`, stacked on that commit.

`Vertex3` grew from 32 to 64 bytes: linear `COLOR_0` and a tangent (xyz + handedness). Locations 13, 14, and the instance alpha sentinel at 15 fill the WebGL2 attribute budget of 16. Toon still owns `mtoon2.w`, so the cutoff is not packed there.

A material with an authored metallic or roughness factor and no texture becomes a 1×1 ORM. Omitted factors stay off the mesh. The spec default of 1 would have turned every glTF prop into metal and beaten the dump's satin roughness. `baseColorFactor` is multiplied in linear light and written back with the IEC curve. `emissiveStrength` stays on the instance because it can exceed 1. The dump wins when `emissive` is set.

`alphaMode` OPAQUE / MASK / BLEND uses one float. Procedural meshes keep the old `tex.a < 0.5` discard, which the contact blob still needs. `doubleSided` selects a no-cull pipeline for the color pass and the shadow pass.

Mip chains now run to 1×1 for any size, not only powers of two capped at 6. Anisotropy is 4 only when mag, min, and mip are all linear. wgpu 30 has no sampler-anisotropy limit field.

DamagedHelmet is CC-BY-NC, so it is not in the tree. The screenshot test runs when `KAGRA_GLTF_SAMPLE` points at a local glb. Its images are JPEG. `decode_png` returned nothing, the mesh stayed the 1×1 white default, and `emissiveFactor` [1,1,1] with no map added a white glow. JPEG now goes through the `image` crate (jpeg only, no rayon). PNG is unchanged.

Master gained the Pixel WebGPU fixes (#152) after this branch opened. The merge keeps that shadow path: nearest compare sampler, `textureSampleCompareLevel`, depth bias 8192 / slope 1.5 / clamp 0.008, and the receiver normal offset. Linear filtering of `depth32float` stays off (`depth32float-filterable` is not requested). The 256-byte `writeTexture` row pad stays.

The ORM rule is the union of the two branches. Omitted factors still stay on the dump. A written pair of metal 1 and rough 1 also stays on the dump, because Kenney stores that glTF default on every base-color material and a 1×1 of it replaces the instance lighting. Any other written pair still becomes a 1×1. A real `metallicRoughnessTexture` is still sampled, and a non-default factor is baked into those texels.
