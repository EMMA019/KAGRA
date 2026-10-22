# Versus slots and input sync

## Decision

`fight_hitstun` is the versus genre. Slot 0 stays the existing walker.
Slot 1 replaces the opponent AI. Other `GenreSession` impls keep a default
that ignores extra slots, so puzzle / sports / card / fps do not change.

Online is lockstep input sync, not state sync. The round is a pure function
of the start dump, both sticks, and a fixed dt. Phones send quantized
integers. A tick runs only when every slot for that tick has arrived.
Prediction and rollback are not in this step. Sockets stay out of
`kagra-shared`. `server/netplay_relay.py` is a local WebSocket room
(stdlib only). `kagra/netplay.py` is the bridge client.

## Stumbles

- `WalkInput` has no `PartialEq`, so the lockstep test compares axes with
  `to_bits` instead of deriving on the shared input struct.
- `drive_body` started with 8 arguments and clippy rejected it. A small
  `BodyStep` carries stun / attack / done / stick.
- Reporting slot 0 with `slot or -1` treated 0 as missing. Both sims still
  matched; the checker then rejected the pair. Compare `is not None`.
- `physics_stack_smoke` still fails with a missing PNG. That scenario only
  writes the file when the physics feature is built. `demo_live_smoke` was
  not run (known failure). `rpg_town_smoke` passed.

## Verify

Headless local match: `examples/versus_local.py --headless` on lavapipe,
640x360, both capsules and the split touch pads. Online: two processes,
90 ticks, identical fingerprints and dumps.
