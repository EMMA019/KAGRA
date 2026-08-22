# Changelog

## 0.1.0

First public-facing cut.

- `pip install kagra` wheels via tag-triggered publish (`v*`)
- `python -m kagra` / `kagra` — sing & dance demo; downloads a sample VRM once
- `VrmAvatar.sing()` / `dance()` and a built-in song synthesizer
- `kagra.line()` no longer raises `NameError`
- Engine no longer writes `keymap.json` into the working directory
- Examples use `kagra.font()` instead of a Windows-only Meiryo path
