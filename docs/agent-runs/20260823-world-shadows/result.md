# Result — World shadows (P5)

- Fit: VRM + 床 / 箱 / Prop AABB。空メッシュ除外。half clamp 28
- Casters: 即時 Mesh3D、retained、インスタンス（専用 shadow VB）
- API: 変更なし
- Tests: pytest（golden 以外）通過。`shadow_fit` は Rust 側に追加。
  この VM の Cargo 1.83 では lock の indexmap 2.14 をパースできず未実行。
- GPU verify: 未実行
