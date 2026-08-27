# Session

- Branch: `local/m3-shared-picture` from `local/m3-stealth-hide` (`75ff481`). origin/master still `9792702` (#106). Not merged.
- Inspected `docs/API_INDEX.md`, `docs/schemas/world.json`, `kagra-shared` `compile_scene` / `draw_lights` / `LocalLight` slots 0..3 / `Material::Metal` / `MESH_PLANE` + ALPHA_BLEND.
- Empty dump lights (orb_rush `lights: []`) left all 4 slots OFF. Crest already dumps key+fill+rim. Default key+fill only when the dump is empty; occupied slots stay 1:1 (no leak). Max 4 indoor slots.
- Shared path had no contact blob (V2 indoor umbra is a different renderer; not ported). Added ground `MESH_PLANE` discs with instance alpha under capsules and props (`height_at` + 0.03). Skip floor `model: plane` and FPS local body.
- `Material::Metal` already maps coin name / `metallic>=0.5`. Shader GGX was sun-only then Lambert `local_lit`, so fill lights made coins read plastic. Metal locals now GGX spec (metallic=1, roughness=0.12). Lambert locals stay on Solid/Grass/Road.
- Did not rewrite `world_play.rs` genre loops. No RendererV2, VRM, SSAO, GI, Rapier, new renderer, 0.19+30 mix.