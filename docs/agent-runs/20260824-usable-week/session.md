# Session — 2026-08-24 usable week

## 決断

- トーンマップはオプトイン。Prop Garden スモークとゴールデンを守る。
- 法線マップと USB/XInput（gilrs）はこのスライスに入れない（頂点レイアウトと依存）。
- Rapier / OSM / 4 段フィルム CSM はやらない。
- 完了チェックは API では上げない。見知らぬ人テストと GPU verify のあと。
- D-6 は始めない。

## 実装

- ACES: `cam.env.w`。`set_tonemap`。swapchain は sRGB なので追加ガンマ無し。
- スペキュラ: HDRI キューブ 4 mip + `textureSampleLevel(..., rough * 3)`。
- 室内影: スポットが同じ 2048 デプスマップに透視 VP を書く。点は影無し。
- 屋外: ワールド XZ を `(2*half)/2048` でスナップ。2 段のまま。
- 操作: ポインタロック、`clicked_prop`、`animate`/`sequence`、`Label`/`Button`、
  `Walk.carry`、coyote + ジャンプバッファ、親子 2 段、`sound()`。
- デモ: Pretty Room / Overworld / Prop Garden。Garden スモークはトーンマップ無し。

## 躓き

- `tone()` に `sound()` を差し込むと docstring が壊れ、構文エラーになった。直した。
- パッケージ正面の `Label`/`Button`/`Tween` は 2D `kagra.ui` と同名。正面は HUD / motion。
- `Walk.lock_cursor` を init 時に bool 化すると、Garden の F 切替でロックしない。
  `None` = 一人称に追従。
- この VM は rustc 1.83 と GPU wheel 無し。Rust は CI `@stable`、絵は未確認。
