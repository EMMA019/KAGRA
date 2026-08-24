# Session — 2026-08-24 Picture P6–P8

絵トラックの残りを一度に。P5 は #45 で master 済み。

## 判断

- 新しい面は既存の `set_*` に揃える。`set_point_light` / `set_hdri` / `set_mesh_pbr`。
- 点光源は 1、影は書かない。intensity=0 がオフ（既定のエンジン状態）。
- HDRI はキューブ。`studio` は内蔵グラデ。正距円筒ファイルも可。PMREM はまだ。
- 汎用メッシュだけ Cook-Torrance。metal=0 / rough=1 は旧 Lambert（スモーク画素を守る）。
- MToon は薄めない。点と HDRI は強度 0 なら足さない。
- glTF flatten が `pbrMetallicRoughness` を読む。Prop に `metallic` / `roughness`。
- Prop Garden のスモークは画素を変えない。非スモークだけクロム球 + studio + 点光。

## Verify

`pytest tests -m "not golden"`。`python3 tools/gen_api_index.py --check`。
Cargo 1.83 はこの VM で lock の indexmap 2.14 を読めない。GPU verify は未実行。
