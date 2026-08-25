# Result

Sticky Walk after releasing WASD/arrows on Windows (Crest Isle). Did not redo slope AABB or `kagra.stage`. No Rapier.

## Cause

#71 ignored `repeat=true` after key-up. On Win32, a KEYDOWN after `WM_KEYUP` has KF_REPEAT **clear**, so winit reports `repeat=false` and `held` stuck. Japanese IME (`set_ime_allowed(true)` + `NamedKey::Process` / Unidentified scancode) dropped the release for S and ↓.

Physics after #79: wish 0 on flat / grade 0.4 does **not** keep walk-speed. Tiny settle only.

## Change

- `apply_key`: block non-repeat re-down this frame and the next.
- `ingest_key`: Windows scancodes pair IME Process / Unidentified release to WASD/arrows.
- Game window: `set_ime_allowed(false)`. Text fields opt in via `set_ime_cursor_pos`.
- Focused `DeviceEvent::Key` **releases** only (raw repeats have no flag).
- `Walk`: if wish is idle and not steep-sliding, snap `vx/vz` to 0 after `world.update`.

## Verify

```
python tools/gen_api_index.py --check          # OK, 422 entries
pytest tests -m "not golden"                   # 378 passed, 10 deselected
cargo test -p kagra-core --no-default-features --locked input
  # 12 passed (including windows_keyup_then_nonrepeat_down_* and IME Process S/↓)
```

GPU smoke not re-run (no visual change; input/physics only).
**GitHub CI: 17 checks passed** on `9882ad0`.

PR: https://github.com/EMMA019/KAGRA/pull/80
