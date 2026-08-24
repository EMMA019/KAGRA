# Result — World shadows (P5)

- Fit: VRM + 床 / 箱 / Prop AABB。空メッシュ除外。half clamp 28
- Casters: 即時 Mesh3D、retained、インスタンス（専用 shadow VB）
- API: 変更なし
- Tests: `kagra-core` `shadow_fit` + pytest（golden 以外）
- GPU verify: 未実行
