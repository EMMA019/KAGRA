# Android shell (placeholder)

将来: `wgpu` + JNI で `kagra-core` を共有ライブラリとしてリンクし、
`docs/schemas/input_events.json` のポインタを `poll_pointers()` 相当で流す。

今はプレースホルダ。ビルドは未対応。

```text
create_surface → render_frame → poll_pointers → pause/resume
```
