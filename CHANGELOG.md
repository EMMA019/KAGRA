# Changelog

## Unreleased

- Publish / CI: drop macOS runners. Wheels are Linux + Windows only until a Mac
  can verify them (macos-13 also sat queued and blocked `v0.1.0` upload).
- Publish: Linux wheels now request CPython 3.10–3.12 inside manylinux_2_28
  (v0.1.0 failed with “Couldn't find any python interpreters from 'python3'”).

## 0.1.0

First public-facing cut.

- `pip install kagra` wheels via tag-triggered publish (`v*`)
- `python -m kagra` / `kagra` — sing & dance demo; downloads a sample VRM once
- `VrmAvatar.sing()` / `dance()` and a built-in song synthesizer
- `kagra.line()` no longer raises `NameError`
- Engine no longer writes `keymap.json` into the working directory
- Examples use `kagra.font()` instead of a Windows-only Meiryo path
