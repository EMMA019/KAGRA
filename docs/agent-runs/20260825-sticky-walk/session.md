# Session — 2026-08-25 sticky Walk after key-up (Crest Isle / Windows)

Master at start: `88d074c` (#79 slope AABB + #71 sticky-walk already merged). Did not redo foot AABB, `kagra.stage`, or Rapier.

## API / code search

- `InputState.apply_key` (#71) ignored `repeat=true` after key-up.
- Window always called `set_ime_allowed(true)`.
- `Walk.update` always `move_player` then `world.update` (including wish 0). `Walk.step` is an alias.
- Slope `_integrate` zeros `_slope_vx` on walkable grades; steep slide is a separate path.
- `debug_trace` is float vs terrain, not wish. Measurement for this bug is GPU-free `move_player` + `World3D.update`.

## Hypotheses (measured)

| Hypothesis | Result |
|---|---|
| #71 `repeat=true` after up | **Incomplete.** After a real `WM_KEYUP`, the next `WM_KEYDOWN` has KF_REPEAT=0, so winit reports `repeat=false`. #71's test used `repeat=true` and never hit the Windows path. |
| IME `NamedKey::Process` / JIS Unidentified release | **Kept.** Press is `PhysicalKey::Code(KeyS)`; release is `Unidentified(Windows(scan))` + `Process` → `resolve_keycode` returned None → `held` stuck. Game window had IME on. |
| Pointer-lock drops WindowEvent key-up | **Partial.** Crest Isle is third person (no lock). Still apply focused `DeviceEvent::Key` **releases only** (raw repeats have no `repeat` flag). |
| S vs ArrowDown | **Discarded.** Same wish axis; both now pair through the Windows scancode map. |
| gilrs leftover stick above deadzone | **Not the Crest Isle report.** Analog above 0.2 still walks (by design). 0-axis / NaN already idle after #71. |
| Walk.step skips `move_player` when wish is 0 | **Discarded.** It always writes. Added post-`world.update` idle snap so collision kicks cannot look like a held key. Steep `_slope_vx` is kept. |
| #79 snap-to-plane keeps walk-speed | **Discarded on grade 0.4.** Wish 0 + `vx=vz=0` each frame settles; not held-key distance. Tiny slide-to-stop allowed. |

## Decisions

- `apply_key`: also ignore a non-repeat down if the code is in `released` this frame or `rehold_block` (last frame's ups). Real re-press works after that window (~2 frames).
- `ingest_key`: native token + Windows scancodes for WASD/arrows/space so IME Process release still clears `held`.
- `set_ime_allowed(false)` on the game window. `set_ime_cursor_pos` opts IME back on (text fields) and `release_all` first.
- `Ime::Enabled` → `release_all`.
- Focused `DeviceEvent::Key` releases only.
- GPU-free tests: Windows `repeat=false` after up; IME Process release of S and ↓; wish 0 then `World3D.update` on flat and grade 0.4.

## Stumbles

- Trusting winit `repeat` after key-up is wrong on Win32: bit 30 is clear once the key is up, so a queued/spurious KEYDOWN looks like a real press.
- Handling `DeviceEvent::Key` **presses** would re-hold from raw auto-repeat (no repeat flag). Releases only.
- Wiring idle-stop without checking `_slope_vx` would flatten the steep-slide path.
