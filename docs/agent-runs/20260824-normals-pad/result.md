# Result — normals + USB pad

- Picture: Mesh3D tangent-space normals (`normal_texture_id` / `Prop(..., normal=)` /
  glTF `normalTexture`). Cotangent frame. Linear upload via `srgb=False`.
  Pretty Room brick wall + Prop Garden bump crate when not `KAGRA_SMOKE`.
  Not indoor-shadow pixels. Not tonemap pixels.
- Play: USB/XInput via gilrs on the EventLoop. `poll_pad` / `pad_axis` / `pad_down`
  on Engine. `inject_pad` still wins.
- Tests: `pytest tests -m "not golden"` — **285 passed**, 3 deselected
- Verify: GPU wheel missing on this VM. Existing smoke scenarios unchanged
  (normals are non-smoke only).
- Checkboxes in `docs/ROADMAP.ja.md` stay open until a stranger would watch 30s.
