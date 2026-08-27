# Session

- Read `td.rs`, `td_lane_world.json`, shop buy (J/click + coins), world_play tick.
- Puzzle `pad` would steal the dump (`is_puzzle`); place pads are `slot`.
- `refresh_coin_count` zeroed TD coins after seed; restore `td::START` like shop/fps.
- Place spends COST=5 from START=10 on a nearby slot; occupied slot name `used`.
- Existing auto tower + path walk kept. All towers fire. No extra lights/SSAO.
- Stumbled: first tests saw coins=0 until world_play restore; confirm() no-ops while Playing so retry uses start().
