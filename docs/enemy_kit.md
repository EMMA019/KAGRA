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
| `walk_dx(type, step_now, meet_step)`: the movement pattern | The clock (HOP-BEAT: `floor(t / beat * steps_per_beat)`) |
| `touch(type, x0, x1, &HeroBox)` gives `Touch::None / Hit / Stomp` | What Hit and Stomp do: a life, +150, a bounce, a grade, a sound cue |
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
    {"id": "bat",    "look": "bat",    "unlock_level": 2, "y_lo": 1.45, "y_hi": 2.1}
  ]
}
```

| Field | Meaning |
|---|---|
| `unlock_level` | Placements below this level fail the checker |
| `y_lo`, `y_hi` | The hurt band above the ground (metres). `y_lo` above 0 is a flyer |
| `stompable` | Coming down within `stomp_depth` of `y_hi` is a Stomp, not a Hit. Ground enemies only |
| `step_m`, `steps_per_beat`, `walk_steps` | A walker moves `step_m` per step toward the hero and appears `walk_steps` steps out. `step_m: 0` stays put |
| `look`, `color` | Picture and tint, owned by the kit |

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
`tools/gen_assets/hop_models.py` and a ledger entry. Until the model exists the kit draws the
primitive fallback.

## Plugging it into another kit

1. **Schema.** Put `pub enemies: EnemySpec` in your spec (`#[serde(default)]`). Call
   `enemies.validate("my spec", &["slime", "beetle", "bat"])` with your own look list. Mirror the
   defaults in Python as HOP-BEAT does (`kagra/hop_beat.py`) if you bake a fixture.
2. **Spawning.** For the current stage, `enemies.unlocked(level)` lists the usable types. Pick with
   `weighted_pick(&weights, rng_roll)`. A runner would spawn one enemy per gap in its lane. A puzzle
   would place it on a cell.
3. **Movement.** Convert your clock to a step number and call `walk_dx(ty, step_now, meet_step)`.
   `None` means it has not appeared yet. Otherwise it is that many metres ahead of its meeting spot.
4. **Contact.** Build a `HeroBox { x, y, half_width, descending }` and call
   `touch(ty, x0, x1, &hero)`. React to `Touch::Hit` and `Touch::Stomp` however your game wants.
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
