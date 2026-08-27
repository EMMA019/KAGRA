# Prompt

Puzzle close on origin/master (f0341a9, TD place+cost just closed). Box-on-pad
already landed. Still missing vs roadmap: hinge / fixed joint / ray.

Smallest slice: kinematic fixed joint (lid parented to crate; moving crate moves
lid) + look-ray click/J opens latch (AABB query along facing). Keep box-on-pad.
Prefer kagra-shared constraint, not Rapier. Existing fixture puzzle_pad_world.json.
No new ECS / RendererV2 / VRM. world_play.rs stays tiny.
