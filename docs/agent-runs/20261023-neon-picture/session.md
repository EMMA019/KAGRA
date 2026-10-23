# Session

The Pixel shot was the stable page after the Kenney GLB change. The hull, rings, and the top of the frame were clipped white, the floor had black streaks, and the trail was a row of dashes.

Kenney Space Kit materials have no textures. `metallicFactor` and `roughnessFactor` are both 1 and were ignored, so the ship used a glossy instance roughness. Speeder normals face outward. The dark center is the `dark` material, not a flipped normal.

WebGPU cannot keep an Rgba16Float frame (Chrome samples it as white), so the frame is 8-bit and clamped before ACES. The fix tonemaps in the shader on that path only. HDR (WebGL2, lavapipe) still tonemaps in the bloom composite.

The trail quad stands in XY. Folding flight into Y left it edge-on to a +Z chase camera. Pitch −π/2 lays it flat. The first version was a white wedge because the head was emissive and glossy; the albedo is now dark and the gradient is the emissive color.

The first browser shots were of a stale server on port 8765 (`scratch/web`). The new tree is what the phone frames show. WebGPU has no adapter on this VM, so both `?backend=webgpu` and `?backend=gl` present through SwiftShader WebGL2.

Contact discs are optional now. Turning them off only for Lumen leaves indoor rooms alone. Factor ORM is baked only when the JSON keys exist, so other untextured meshes keep their instance roughness.
