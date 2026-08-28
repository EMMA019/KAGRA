# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok (Rust 1.98)

cargo test -p kagra-shared --locked --offline --lib
# 335 passed; 0 failed (10 action2d: dump, title, walk +X, melee hit/hurt,
# kill disable, contact retry, other genres, fire from range, hall<->den,
# den fixture)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 572 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/action_side_world.json
```

Space / Enter / click starts. A/D or W walks the player card along X on the 2D plane.
J / click hits the foe sprite when in reach; from range it fires a dump-visible shot
that moves and can hit/kill. Walk left into the brown trigger to swap hall -> den
(dump `scene` name + `flag`). Walk right into the den trigger to return.
Hurt/kill stay dump-visible (`name` / foe `enabled`). Side camera stays on the XY cards.
`--seconds` injects W + attack so walk + fire is visible without a human.

Den sibling:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/action_side_den_world.json
```

3D action arena and sprite card dumps are unchanged.

## Remaining 2D gaps

This slice closes the ROADMAP 2D action genre gap (walk/hit/kill + projectile + room switch).
Still not this engine: camera billboards, sprite-as-FPS-player, `enemy.chase` (not in API_INDEX),
empty/no-ammo, inventory, pygame / pyxel / Entity / tilemap (shelf), Rapier, a second 2D runtime.
