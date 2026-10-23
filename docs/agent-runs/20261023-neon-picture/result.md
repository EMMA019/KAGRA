# Result

The Pixel frame was clipping neon to white before tonemap, and the chase camera was looking at the trail edge-on.

## Causes

- WebGPU frames are 8-bit. Emissive values clamped to 1 before ACES, so saturated neon became white. The cool LUT then painted that white pink across the top of the frame. That bar was the scene, not CSS.
- The speeder's Kenney materials are factor-only (metal 1, rough 1, no textures). Those factors were dropped, so the instance roughness 0.32 drove a glossy highlight. Normals on `craft_speederC` point outward (370/446 verts). The dark cockpit glass is the authored `dark` material, which read as a hole once the hull clipped to white.
- Contact discs were still emitted on a flying scene with no floor. Coplanar discs z-fought into black streaks. Shadow softness was 3, which smeared the acne.
- The trail folded Z into Y and drew vertical quads. A camera looking along +Z saw the edge.
- The nearest corridor ribs (emissive 3.4, including one behind the ship) crossed the lens as white bars.

## What changed

- 8-bit frames tonemap in the mesh shader. The composite still does the LUT, vignette, and sRGB encode, and does not run ACES a second time. 3D instance and fog colors always decode as sRGB.
- Authored metallic/roughness factors with no texture become a 1×1 ORM. Missing keys stay scalar.
- `WorldDoc` can turn contact discs off and set fog distances. Lumen does both. Dark skies get a magenta horizon; day skies stay on the old curve.
- Ship scale 0.36 (about 0.78× the old hull), camera back at 11.4 m, fov 50. Hull emissive 0.15. Pink engines and a cyan canopy sit on the mesh. Pass rings are cyan. Hazards are pink-tinted Kenney meshes. A neon grid floor, closer fog, and a denser star field replace the flat navy.
- The trail is a horizontal ribbon (pitch −π/2), magenta tail to cyan head, wider toward the ship.
- Score sits on a dark plate. BOOST is a 64px glass button. Corridor and Crest show only on the title screen. Boost eases the camera back, opens the fov, and draws speed lines. Gate sparks are larger.

## Verify

- `cargo test -p kagra-shared --lib neon_run` and the factor-ORM test passed.
- `cargo clippy -p kagra-shared --all-targets --features render -- -D warnings` and `cargo fmt -p kagra-shared -- --check` were clean.
- `pytest tests/test_neon_run.py` passed.
- Portrait 390×844 lavapipe shots and a phone-size Chromium shot (`?backend=gl`, SwiftShader). This VM has no WebGPU adapter (`No available adapters`); `?backend=webgpu` falls back to WebGL2. Near-white pixels on the phone frame are about 0.03%.
