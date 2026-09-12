# Session — mainline cut + Bar sim

## Decisions

- `import kagra` no longer loads `kagra_core`. Root `pyproject.toml` builds
  `kagra-shared` as `kagra.kagra_shared` (package 0.2.0).
- Genre dispatch is `WorldDoc.genre` only. Prop names (`dock`, `door`, `stove`)
  are scenery. Bar room dump has no genre so RPG/shop/cook cannot steal it.
- `WorldProp.texture` + HUD `images` for generated PNG. Alpha test in shader3d.
- Assets from `tools/gen_assets/bar.py` (deterministic procedural stand-in;
  same paths an image model would overwrite). Play-time does not call a model.
- Game logic in `kagra/bar_sim.py` (Python only), same shape as bunny_garden.

## Stumbles

- `kagra/__init__.py` eager-imported `kagra_core`, so README quick start and
  `kagra.verify` died before any WorldPlay tick.
- rustfmt drift was the only red CI job for two weeks; `cargo fmt` +
  `rust-version = "1.88"` (as_chunks / is_multiple_of).
- Primitive meshes had UV = 0, so textures would have been a single texel
  without the box/plane/quad UV fix.
