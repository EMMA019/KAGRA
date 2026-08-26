# Prompt

Repo: EMMA019/KAGRA, start from current master. Emma (EMMA019) is asleep and will check around 05:00 JST. Ship a PR she can merge in the morning. Do NOT merge. Do NOT clone extra repos.

THIS PR is "world as data" (15% → toward 35%). Drawing is not touched.

1. Replace docs/ROADMAP.ja.md with the new 80% definition (100% = agent ships a normal indie 2D/3D game with no human screen; 80% = that minus net/destruction/cloth/vehicles/GI bake/DOTS/HDRP/human editor/Shader Graph/Visual Scripting/Addressables/Terrain sculpt/ProBuilder/Cinemachine/PhysX-complete/VRM-on-Wasm; now ~15%; mountains M0–M4). Point agents away from "63%". Archive the old text.

2. Official public API: World / Prop / Walk / mesh-or-avatar / Camera / input / sound. Promote World3D to the one World. Take Entity, tilemap, and Tk editor off `import kagra`.

3. Every world object gets a stable string id (not a GPU integer).

4. world.query(...) filters by type, name, AABB. Bald leftover tiles queryable (type=terrain_tile, loaded/albedo_ok).

5. world.dump() / world.load() JSON. Schema at docs/schemas/world.json.

6. kagra.verify expect: world assertions. Rewrite at least one Crest Isle or Orb Rush verify scenario. GPU-free unit tests.

Done when an agent can ask without a screen: where is the player, how many coins, is this tile loaded / albedo_ok.
