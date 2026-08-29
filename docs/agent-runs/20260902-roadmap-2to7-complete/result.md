# Result — 汎用エンジン化ロードマップ Phase 2–7 完了

2026-09-02 に Phase 2（アニメーションブレンド）から Phase 7（ローカライズ）まで
連続で完了した。Phase 0 は 2026-09-01 に完了済み。Phase 1（Rapier 物理）は
ユーザーの決定どおりトリガー発生まで着手しない。

## 完了スライス（コミット順）

| Phase | 内容 | コミット |
|---|---|---|
| 2 | アニメーションブレンド: anim_blend（rest↔clip）+ 上半身/下半身レイヤー分離（overlay ジェスチャー） | e3cfbfb |
| 3 | 経路探索: find_path（A*）/ move_range（SLG 移動範囲）/ LOS / simplify。torneko BFS を置換 | 2d04469 親 |
| 4 | 音 DSP: mix / reverb（Schroeder）/ crossfade / duck。WAV bytes 加工 | 0b9b451 親 |
| 5 | UI 成熟: scroll_window / paged_menu。torneko ログをスクロール窓化 | 0a005e8 |
| 6 | セーブ深化: version 付き + マイグレーション + .bak バックアップ + SlotStore | 単一コミット |
| 7 | ローカライズ: i18n テーブル + 言語切替。bunny/torneko の UI 文字列を t() 経由に | 単一コミット |

## 横断原則（全てのスライスで守った）

- **ゲームロジックは Python のみ**: 新 API は全部 `kagra/*.py`（拡張非依存）。
  テストは conftest の `load_kagra_submodule` でロード。
- **決定論**: 乱数なし。torneko ヘッドレスは save MD5 一致で再確認。
- **既存の緑を守る**: 既定 ja / 既定 version 1 / 後方互換（旧形式セーブ）で
  既存テスト 580+ 件を壊さずに追加 48 件。

## 最終状態

- pytest: 628 パス（10 deselected）
- Rust: 396 lib + 12 offscreen、clippy -D warnings クリーン、wasm32 OK
- `gen_api_index --check` OK

## 次の山（ユーザー長期リスト）

SLG（3 本目ジャンル）— 移動範囲は `kagra.path.move_range`、長いユニット一覧は
`kagra.ui2d.paged_menu`、複数セーブは `kagra.save.SlotStore` がそのまま使える。
