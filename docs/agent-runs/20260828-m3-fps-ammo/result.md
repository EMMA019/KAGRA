# Close FPS ammo/reload on play_world

## Try
`python -m kagra.play_world kagra-shared/tests/fixtures/fps_range_world.json`

Space / Enter / click starts. WASD walk. Mouse / arrows look (eye camera). Click or J/Z/F fire (hitscan, spends a mag round). R reloads after a short delay. Empty click is dry (no kill). Mag count is dump `coins` (HUD pips). Local player mesh stays hidden.

## Verify (Emma's Windows, 2026-08-28)
- `cargo fmt -p kagra-shared`
- `cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings`
- `cargo test -p kagra-shared --locked --offline --lib` : 272 passed
- pytest tests -m "not golden" : 571 passed, 10 deselected
- `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --offline --locked`
- window example `--seconds 2` on fps_range_world.json exited 0

## Closed vs remaining
Closed: dump-visible mag (coins), fire spends a round, empty click does not hitscan/kill, R reload restores mag, eye camera + hitscan kept. Muzzle/hit flash unchanged. No SSAO. No extra shadow cascade.

Not this slice: recoil, weapons inventory, arms mesh, net, VRM, Rapier, RendererV2, SSAO/GI.