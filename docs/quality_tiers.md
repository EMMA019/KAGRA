# グラフィックの段階 / Graphics tiers (`safe` / `mid` / `high` / `max`)

どのゲームでも使える共通部品。プレイヤーは設定画面の **Graphics** 行で「おまかせ / せつやく / ふつう / きれい / さいこう」を選ぶ。
おまかせ（既定）は初回に端末を測って段階を選び、遊んでいる間は重くなったら下げ、軽ければ（静かな場面で）戻す。
Genre-neutral: the tier data, the apply step and the page logic know nothing about HOP-BEAT. A kit takes part by
saying so (`KitHost::supports_quality_tiers`) and by reading two hints; Neon Cat does neither and is unchanged.

## The tiers

One file is the source of truth: `kagra-shared/src/quality_tiers.json` (Rust `include_str!`, mirrored and checked in
`kagra/quality.py`, read by the page through `WasmSession.qualityJson()`).

| tier | render scale | picture | SSAO | TAA | shadow cascades | PCSS | bloom x | atmo (sky props) | particles |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| safe | 1.0 | low | off | off | 1 | off | 0 | 0.3 | 0.4 |
| mid | 1.5 | legacy | off | off | 1 | off | 1 | 0.75 | 0.75 |
| high | 2.0 | legacy | off | off | 2 | off | 1 | 1.0 | 1.0 |
| max | 2.5 | legacy | 4 | on | 3 | on (softness 2) | 1 | 1.4 | 1.0 |

- `picture` is the `WorldDoc.quality` fill (`legacy` = leave the kit's own default). Explicit fields win over the fill, so
  each tier sets `ssao`, `taa`, `shadow_cascades`, `shadow_pcss`, `shadow_softness` itself. `bloom x` multiplies the game's own bloom
  (a game without bloom stays without). `fx` is a map of the kit's runtime switches (`fx_post`, `fx_atmo`) the tier turns on or off.
- `atmo` is how much sky the kit draws (stars, clouds); `particles` is the share of sparks kept. They are picture only: the sim
  never reads them (a test runs the same inputs at two settings and compares state JSON).
- **`high` is about what the page drew before tiers** (scale cap 2, legacy picture, full sky). **`mid` is lighter than before**
  (cap 1.5, 75 % sky). **`max` is manual only**: auto never goes above `auto.ceiling` (`high`). On the 8-bit WebGPU path TAA and
  PCSS do nothing (the renderer says so in `pictureStatus()`), so `max` there is scale 2.5, 3 cascades and SSAO.
- Render scale is a cap: the page uses `min(devicePixelRatio, tier scale)`, and its older step-down ladder (`adapt`, 2 -> 1.5 -> 1.25
  -> 1) stays as an emergency brake **below** the tier's cap; a tier change resets it.

A game can override any of it with a top-level `quality_tiers` object in its `world.json` / manifest `game` block (key by key, same
bounds; unknown keys, a missing tier or a scale that falls from safe to max are refused). The session strips it before the kit's own
loader. `QualitySpec::validate` (Rust) and `kagra.quality.validate` (Python) hold the same numbers.

## Choosing: auto, probe, guard

Setting (`localStorage` key `kagra.quality`, `{mode, tier, ts}`; no other key, no network):

- `?quality=` wins for that visit and is **not saved** unless the player then chooses in the UI. Accepted: `auto`, `safe`, `mid`, `high`,
  `max`, and the old words `low` (= safe), `mobile` (= mid). Anything else is ignored.
- Default is `auto`. The Graphics button cycles auto -> safe -> mid -> high -> max.
- If storage is missing (private tab, sandboxed iframe) everything still works; the choice just is not remembered.

Auto, in `scripts/preview_site/quality.js` (pure state machines, tested under node):

1. **First-run probe.** Start at `auto.start` (mid). The probe only looks while a run is in `play` (a menu is not a measurement); each
   window is 1 s settle + 3 s of frames. If p95 <= 17.5 ms and p99 <= 22 ms it tries the next tier, up to `auto.ceiling`; at the first window
   that fails it keeps the last good tier; if even mid fails it drops to safe. A gap over 1 s (hidden tab) restarts the window. The result is
   stored and trusted for 7 days.
2. **Runtime guard.** Down: at least `down_drop_pct` (6 %) of the frames in the last 3 s were over 25 ms -> one tier down, and the tier it left is
   banned for `ban_secs` (180 s). Up: only at a calm moment (title, pause, result), after `up_secs` (25 s) with no change and a good window over
   those 25 s, never into a banned tier, with `cooldown_secs` (8 s) between any two moves and 1.5 s ignored after each. It never flaps by design:
   a step up that fails comes straight back down and bans itself. Manual tiers never move.
3. Everything is held (`kagraQuality.hold(true)`) while the recorder or Find my limit measures.

`window.kagraQuality` (dev and tests): `mode`, `tier`, `status()` (the last tier changes with a reason), `setMode`, `cycle`, `custom(over)`,
`show(tier)`, `restore()`, `hold(on)`.

## Wiring a kit

```rust
fn supports_quality_tiers(&self) -> bool { true }
fn quality_hints(&mut self, atmo: f32, particles: f32) { /* picture-only knobs */ }
```

The session applies the tier to the `WorldDoc` it is about to draw (after `kit.document()`, so no kit code runs differently), sets the kit's
switches from the tier's `fx` map and calls `quality_hints`. A kit that returns false has `qualityJson() == "{}"`: no row, no probe, no guard.

## The atmosphere jank (33 ms frames with atmo on)

What the Pixel 10 run showed: p99 16.9 ms with atmo off, 3.5 % / 1.8 % of frames at 33 ms with atmo on, `renderMs` ~6 ms (CPU submit),
no main-thread long tasks. What is known from the code (a throwaway release test that counted the props `build_doc` emits, `fx_atmo` off vs on):

| tier knobs (atmo / particles) | props, atmo off | props, atmo on | stars | clouds |
| --- | --- | --- | --- | --- |
| 0.3 / 0.4 (safe) | 109 | 124 | 12 | 0 |
| 0.75 / 0.75 (mid) | 112 | 149 | 30 | 4 |
| 1.0 / 1.0 (high) | 114 | 161 | 40 | 4 |
| 1.4 / 1.0 (max) | 114 | 179 | 56 | 6 |

(The rest of the "on" delta is the horizon band, 2 props, and the moon.) So atmo on adds 15 to 65 props, +40 % at high, and the stars and clouds are
`sphere` props (~576 triangles each, emissive): about 23 k extra triangles at 1.0, drawn again for every shadow cascade if they are casters
(not checked: the renderer's caster rule was not traced), plus emissive overdraw at render scale 2. Not GC: the wasm side allocates a few dozen
short strings per frame and the main thread was not blocked (`longTasks` 1).

**This is a hypothesis, not a measured cause.** The box has no phone GPU, so the cost could not be reproduced here. What changed:
the sky now scales with the tier (safe 12 stars and no clouds, mid 30 + 4, high 40 + 4) and the lower tiers also draw at a lower render scale,
which is the likelier lever on a tile-based PowerVR; the guard steps down if 33 ms frames reach 6 %. What to read on the phone: **Find my limit**
has `atmo 0.75`, `atmo 1` and `atmo 1.4` as separate steps on top of an otherwise identical picture, so a jump in p99 or `gpuMs` at one of
those steps is the atmo cost (use `&backend=gl` for `gpuMs`). If it is the sphere count, the next change is a cheaper star mesh.

## Tests and checks

- Rust: `kagra-shared/src/quality.rs` (bounds, merge, apply), `session.rs` / `hop_beat.rs` (tiers change the drawn doc and never the sim, a game
  can override them, autoplay plays a dozen bars and restarts after a game over, hints thin the sky and survive a restart).
- Python: `tests/test_quality_tiers.py` (mirror equals the JSON, same bounds as Rust, `quality_tiers` in a manifest game).
- JS: `tests/data/quality_harness.mjs` (aliases, probe, guard, setting, row, cycle, hold) and the perf harnesses (`docs/phone_measure.md`).

## Not verified

On a phone: the real size of the atmo cost, whether `high` holds 60 fps on the Pixel 10 (a 60 Hz screen, PowerVR D-Series via ANGLE), whether the
guard's thresholds feel right over a 10 minute run (heat), how the Graphics row looks on the card, and GPU time (needs `&backend=gl`).
