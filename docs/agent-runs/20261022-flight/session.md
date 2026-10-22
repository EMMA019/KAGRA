# Session — flight pose on a free WorldDoc

## Decisions

- Do not add a flight genre to `WorldPlay`. The game is `kagra/flight.py` + `examples/flight_minimal.py` on a free dump (`genre` omitted).
- `position` stays world-space unless `local: true`. Crest coins already set `parent` as a link; composing that as TRS would move them.
- Pose is intrinsic yaw, then pitch (nose up), then roll. Positive yaw turns +Z toward +X.
- A disabled prop is still a hinge, so the airframe root is not a stray box.
- Rings are `model: "ring"` (XY torus, hole along +Z), mesh id 7. glTF slots start at 8. Ids stay dense.
- Window frames: `Scene._frame_rgba` from `render_rgba`, presented as PPM. Headless files stay PNG.

## Stumbles

- Exhaustive `WorldProp { ... }` literals in action2d / rpg / td needed `pitch` / `roll` / `local`. Literals that already used `..Default::default()` did not.
- `pip install` of the wheel lands in user site-packages. `examples/` inserts the repo on `sys.path`, so the `.so` has to sit in `kagra/` (gitignored) for a local import. Verify then rendered on lavapipe.
- A chase camera 16 m behind looked through the last ring and shrank the airframe to a speck. Side-chase (7.5 m back, 3.4 m right, 2.6 m up) keeps the wings readable.
