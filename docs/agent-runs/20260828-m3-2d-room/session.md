# Session

- origin/master 407f87f (VRM look-at closed). 2D action walk/hit/kill already on play_world.
- Projectile: J/click from range spawns dump-visible `shot` sprite; it moves and can hit/kill.
  Close-range J still melees (existing hit/kill tests). Empty/no-ammo not this slice.
- Room switch: `trigger` on the left of hall. Crossing swaps hall <-> den (sibling dump).
  Dump scene/name/flag like RPG town <-> dungeon. Named `trigger` so RPG `door` stays RPG.
- Sprite/quad stays on WorldDoc. world_play.rs not rewritten. VRM/human untouched.
