# 2026-10-22 — Smooth UI Typography, Rounded Panels, Chess Contact AO & Vignette

## User request

Upgrade UI typography from pixel font to smooth anti-aliased sans-serif with rounded semi-transparent HUD panels, add contact shadows / AO under chess pieces, add background gradient + vignette, lower exposure to prevent white-piece clipping, and re-render Chess, Backyard Rascals 3v3 CTF, and Flight with PBR gloss and smooth curvature.

## Decisions

1. **Smooth Anti-Aliased UI Font + Rounded Semi-Transparent Panels**:
   - Embedded `kagra-shared/assets/PublicSans-Bold.ttf` (SIL OFL) in `kagra-shared/src/font.rs` for smooth anti-aliased Latin/ASCII UI rendering, while preserving `PixelMplus10-Regular.ttf` fallback for Japanese/CJK characters.
   - Added `radius` and `shadow` options to `kagra/ui2d.py::panel` and `list_lines` so HUD overlays render with rounded corners, soft drop shadows, and top glass highlights while keeping default `panel()` backward-compatible with `tests/test_ui2d.py`.
2. **Contact Shadows / AO & Directional Shadowing**:
   - Updated `shader3d.wgsl` so semi-transparent contact blobs (`MESH_PLANE`) render with radial soft ambient-occlusion falloff and skip shadow-map depth writes (`vs_shadow`).
   - Added dual contact + directional AO discs (`push_contact_blob_alpha`) directly on the board surface (`y = 0.033`) under every chess piece (`piece_*`) in `kagra-shared/src/world_doc.rs`, and applied `shadow_factor` (`sh`) to slot-0 key light in `local_lit` and `local_metal`.
3. **Background Gradient, Vignette & Exposure**:
   - Added a studio cyclorama backdrop + warm rim glow and sky dome (`Material::Sky`) in `kagra/chess_game.py` and `kagra-shared/src/world_doc.rs`, plus a radial vignette in `kagra-shared/src/render/bloom.wgsl`.
   - Tuned chess exposure (`0.80`) and warm ivory piece albedo (`[212, 198, 178]`, `roughness: 0.20`) so sculpted details on white pieces are crisp without overexposure.
4. **Upgraded Sphere/Torus Geometry & PBR Gloss for Flight & Backyard Brawl**:
   - Upgraded `MESH_SPHERE` to a true outward-facing UV sphere (`primitives::sphere_mesh(0.5, 24)`) and `MESH_TORUS` to `48x20` smooth segments so the biplane fuselage/wings/canopy and rings in `kagra/flight.py` show smooth curvature and specular highlights.

## Verify

- `cargo fmt -p kagra-shared -- --check` — clean
- `cargo test -p kagra-shared --lib --locked` — 398 passed
- `python3 -m pytest tests -m "not golden"` — 645 passed
- `python3 tools/gen_api_index.py --check` — OK (56 entries)
- `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json python3 -m kagra.verify examples/verify_scenarios/chess_smoke.json` — `"ok": true`
- `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json python3 -m kagra.verify examples/verify_scenarios/backyard_brawl_smoke.json` — `"ok": true`
- `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json python3 -m kagra.verify examples/verify_scenarios/flight_smoke.json` — `"ok": true`
