# Session — pixel close (indoor shadow + tonemap)

- スポットは既に cascade 0 へ透視 VP を書いていた。色パスは `shadow_factor` を
  **平行光**に掛け、スポットは影なし加算。ランプが全面を照らし、マップは太陽を
  誤って暗くしていた。
- `ShadowU.params.y = 1` のときローカル光に影を掛け、平行光は埋めのまま。
  Mesh3D Lambert / PBR と MToon 3D の両方。
- 画素の閉じ方は committed PNG ではなくオン/オフのペアワイズ。基準画像が
  無い VM / git-lfs でも、差が閾値未満なら落ちる。
- シーンは VRM なし（床 + 箱 / クロム球）。Garden `KAGRA_SMOKE` はトーンマップ
  しない。30 秒デモのチェックは付けない。
- この VM に `kagra_core` GPU wheel は無い。閉じるのは CI の `golden` job。
- IBL 金属も同じペアを足した。屋外の這いはスナップの単体テストのみで、画素は未。
