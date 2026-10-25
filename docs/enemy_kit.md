# Enemy kit: `game_core::enemies`

Enemies for any kit: a runner, a puzzle, a rhythm game. The creature types, how they move,
when they unlock, how often they are picked, what touching one counts as, and the fairness
checks all live in `kagra-shared/src/game_core/enemies.rs`. It knows nothing about beats,
lanes, stages or pixels. HOP-BEAT (`kagra-shared/src/hop_beat.rs`) is the first user.

## Where the line is

| In `game_core::enemies` (shared) | In the kit (HOP-BEAT today) |
|---|---|
| `EnemyType` / `EnemySpec`: the JSON schema (`deny_unknown_fields`) and `validate(who, looks)` | The `looks` it can draw (glb names plus a primitive fallback) and the `enemies` key in its GameSpec |
| `unlocked(level)`: which types a stage may use | Its notion of "level" (HOP-BEAT: the phrase level, 0 to 4) |
| `weighted_pick(weights, roll)`: spawn weights | The random source and what is picked (HOP-BEAT: phrases) |
| `walk_dx(type, step_now, meet_step)`: the walk; `motion(type, step_pos, meet_step)`: walk plus the jump (`dx`, `lift`) | The clock (HOP-BEAT: `floor(t / beat * steps_per_beat)`) |
| `touch(type, x0, x1, &HeroBox)` (and `touch_lifted(.., lift, ..)` for one in the air) gives `Touch::None / Hit / Stomp` | What Hit and Stomp do: a life, +150, a bounce, a grade, a sound cue |
| `Slot` + `spacing_problems`: unlock, rest and bounce room, no overlap | Turning its placements into `Slot`s (a start and a length in its time unit) |
| `Clearable` + `sweep`: runs a pattern at every offset in the judge window | Playing the pattern with its allowed inputs (HOP-BEAT: `sequence_survives`) |

A kit supplies three things: **placement**, **response** and **clearability**.

## The JSON

```json
"enemies": {
  "body_h": 0.6, "stomp_depth": 0.3, "stomp_score": 150,
  "bounce_height": 0.7, "bounce_beats": 0.5, "rest_beats": 0.5,
  "types": [
    {"id": "slime",  "look": "slime",  "unlock_level": 0, "y_hi": 0.38, "stompable": true},
    {"id": "beetle", "look": "beetle", "unlock_level": 1, "y_hi": 0.42, "stompable": true,
     "step_m": 1.0, "steps_per_beat": 2, "walk_steps": 8},
    {"id": "bat",    "look": "bat",    "unlock_level": 2, "y_lo": 1.45, "y_hi": 2.1},
    {"id": "hopper", "look": "frog",   "unlock_level": 3, "y_hi": 0.42, "stompable": true,
     "step_m": 3.0, "steps_per_beat": 1, "walk_steps": 4,
     "jump_height": 0.4, "jump_air": 0.5, "telegraph": 0.25}
  ]
}
```

| Field | Meaning |
|---|---|
| `unlock_level` | Placements below this level fail the checker |
| `y_lo`, `y_hi` | The hurt band above the ground (metres). `y_lo` above 0 is a flyer |
| `stompable` | Coming down within `stomp_depth` of `y_hi` is a Stomp, not a Hit. Ground enemies only |
| `step_m`, `steps_per_beat`, `walk_steps` | A walker moves `step_m` per step toward the hero and appears `walk_steps` steps out. `step_m: 0` stays put |
| `jump_height`, `jump_air`, `telegraph` | A **jumper** (`jump_height` above 0): see below |
| `look`, `color` | Picture and tint, owned by the kit |

### The jumper

A jumper walks like a walker (same `step_m`, `steps_per_beat`, `walk_steps`) but covers each step as
one leap. It sits on the ground for the first `1 - jump_air` of the step and then flies to the next
step's spot, so it lands exactly on the step grid (with HOP-BEAT: on the beat). The jump period is
`steps_per_beat`; the speed is `step_m * steps_per_beat` metres per beat; the arc is a parabola of
`jump_height` metres. The last `telegraph` of its rest (the fraction of a step just before takeoff)
is the tell: the kit squashes it there. While it is in the air its whole hurt band is shifted up by
the lift, so a hero on the ground passes under it and one in the band is hit (a stomp from above
still counts, at the lifted `y_hi`). `look` does not matter to the rules; HOP-BEAT draws `"frog"`.

Rules the validator enforces: `y_lo` is 0 (a jumper starts on the ground), `step_m` above 0,
`jump_height` 0 to 2, `jump_air` 0.2 to 1, `telegraph` 0 to 1 and `telegraph + jump_air` at most 1.

The stock beetle moves 2 m a beat and the stock hopper 3 m a beat (50% more): the speed contrast is
JSON, change `step_m` or `steps_per_beat` to change it.

HOP-BEAT's own pieces are in `course.phrases[].items[]`: `{"kind": "enemy", "enemy": "<id>", ...}`.

## Adding a creature: JSON only

If the picture and the rules already exist, it is only a new entry. A quick red beetle, with no
new code and no new model:

```json
{"id": "dasher", "look": "beetle", "unlock_level": 3, "y_hi": 0.42, "stompable": true,
 "step_m": 0.75, "steps_per_beat": 4, "walk_steps": 12, "color": [255, 70, 70]}
```

Then add phrases that use `"enemy": "dasher"` and run `python -m kagra.hop_beat --check`. The test
`a_fourth_enemy_is_only_json` in `hop_beat.rs` does exactly this. A new silhouette needs a model in
`tools/gen_assets/hop_models.py` (the stock frog is `hop_frog.glb`; the model ledger entry is
`project-hop-beat-models`). Until the model exists the kit draws the
primitive fallback.

## Plugging it into another kit

1. **Schema.** Put `pub enemies: EnemySpec` in your spec (`#[serde(default)]`). Call
   `enemies.validate("my spec", &["slime", "beetle", "bat", "frog"])` with your own look list. Mirror the
   defaults in Python as HOP-BEAT does (`kagra/hop_beat.py`) if you bake a fixture.
2. **Spawning.** For the current stage, `enemies.unlocked(level)` lists the usable types. Pick with
   `weighted_pick(&weights, rng_roll)`. A runner would spawn one enemy per gap in its lane. A puzzle
   would place it on a cell.
3. **Movement.** Convert your clock to a step number and call `walk_dx(ty, step_now, meet_step)`.
   `None` means it has not appeared yet. Otherwise it is that many metres ahead of its meeting spot.
   For a jumper use `motion(ty, step_pos, meet_step)` with the fractional step: it returns
   `(dx, lift)`. `crouch_window(ty)` gives the step fractions of the tell and the takeoff.
4. **Contact.** Build a `HeroBox { x, y, half_width, descending }` and call
   `touch(ty, x0, x1, &hero)` (`touch_lifted(ty, x0, x1, lift, &hero)` with the lift from `motion`). React to `Touch::Hit` and `Touch::Stomp` however your game wants.
   A runner might kill the enemy on a stomp, and a puzzle might ignore `y`.
5. **Fairness.** Make `Slot { item, enemy, at, len }` for each placement in a pattern and run
   `spacing_problems(&enemies, "pattern 3", level, pattern_end, &slots)`. Then implement `Clearable`
   (`survives(offset)`: does a bot that acts `offset` seconds early or late still clear the whole
   pattern?) and call `sweep(&pattern, half_window, 0.005)`. A `Some(offset)` is a placement a
   player cannot clear. HOP-BEAT runs both inside `HopSpec::course_problems`, which runs on every spec load, so a
   bad pattern makes the spec fail to load (the builtin phrases are covered by the unit tests).
6. **Tests.** Copy the unit tests at the bottom of `enemies.rs` for the neutral rules. Add one test
   per kit that every shipped pattern passes the checker.

## What HOP-BEAT does with it

- Placement: items on the beat grid (`beat` and `beats` give a `Slot`'s `at` and `len`).
- Response: Hit costs a life (death reason `enemy`). Stomp gives +150, a bounce (height 0.7) that
  lands on the next half beat, a "STOMP!" grade and the `stomp` cue.
- Clearability: `sequence_survives` plays the phrase with one press per hop at the offset, which
  `sweep` runs over plus or minus min(perfect_ms, 100 ms) in 5 ms steps.

## The fairness checker and the airborne enemies

`HopSpec::course_problems` runs on every load. Besides the neutral spacing rules and the sweep it
has rules for what is not on the ground:

- **Bats** (`y_lo` above 0, no press): `flyer_gap_problems` keeps them clear of the hop across a
  gap (+100 ms of slack), the phrase is swept at +-100 ms, and every ordered pair of phrases
  (`seam_problems`) is played at that window.
- **Jumpers**: `jumper_gap_problems` finds when the leap is over the hero's lane (the lifted band
  within one beat of the hero) and keeps it clear of the hop across a gap, +100 ms of slack too.
  The sweep half-window is the Perfect window (the lifted band makes the late side a bit
  tighter than a slime's, +-90 ms is tested), and `seam_problems` covers phrases with a jumper too,
  both orders. `sequence_survives` needs no special case: `touch` uses the lifted band.
- **Different speeds close together** (`enemy_crowd_problems`): when either of two enemies in a
  phrase moves, they are played over the last beat before the hero reaches the nearer one, and
  their bodies may not overlap. A hopper (3 m a beat) placed just behind a beetle (2 m a beat)
  closes the gap and is refused, as is the reverse. Further out they may share space (one is
  still spawning behind the other) because nothing can touch the hero from there.

The 2000-seed laid-courses test plays every seam the generator produces at those windows.

## Late phrases (HOP-BEAT)

The bat appears only in its own level-2 debut phrase. The late phrases that had a bat over a beetle
or over a slime use the hopper instead: beetle then hopper then a gap, slime-hopper-slime, a hopper
between two hold gaps, and frog-beetle-frog-slime. The hopper debuts alone in level 3.

## Swarm parts (Moonlit Swarm, the survivor-arena kit)

`game_core::swarm` is the crowd side of the same kit idea: `Foes` (a fixed-capacity struct-of-arrays pool, no allocation in the frame),
`Grid` (a spatial hash for contact and push-apart in O(N)), `Budget` (how many to spawn this frame from a rate, with the remainder carried)
and `Rng` (a small seeded generator, so a daily seed replays). A type opts in with `EnemyType.swarm`
(`SwarmStats`: hp, speed, radius, touch damage, xp, unlock seconds, weight); `validate` bounds every number. The first user is
`kagra-shared/src/survivor_arena.rs`; the design and the asset list are in `docs/design/survivor_arena.md`.
