# Result — Phase 7 完了（ローカライズ）

汎用エンジン化ロードマップ Phase 7（文字列テーブル + 言語切替）が完了した。
これでロードマップ Phase 2–7 が全て閉じた。

## 設計

`kagra/i18n.py` — 文字列テーブル + 言語切替。全て拡張非依存。

| API | 用途 |
|---|---|
| `t(key, **kw)` | 現在言語 → `ja` → キー文字列のフォールバック。`{name}` は kwargs で埋める |
| `set_lang(lang)` / `get_lang()` | 表示言語の切替 / 取得 |
| `add_table(lang, dict)` | テーブル追加（既存キー上書き） |
| `load_json(lang, path)` | JSON テーブル読み込み（非オブジェクトは ValueError） |
| `available_langs()` | 登録済み言語一覧 |

## 配線（実証）

- bunny_garden: メニュー（話す/飲み物/ほめる/閉店/やめる）+ HUD（DAY・好感度・
  在庫・操作説明）を `t()` 経由に。ja / en テーブル登録。
- torneko: メニュー（閉じる）+ HUD（操作・終了）+ 勝敗メッセージを `t()` 経由に。
- 既定は `ja` なので既存テスト・既存 PNG はそのまま。

## verify

- pytest: 628 パス（test_i18n.py 10 件: キーフォールバック / 言語切替 /
  ja フォールバック / プレースホルダ / 欠落 kwarg / JSON 読み込み /
  非オブジェクト拒否 / 言語一覧 / bunny 選択肢が lang に追従 /
  torneko メニュー文字列が lang に追従）
- bunny ヘッドレス: `--headless --days 2` 実行 OK（PNG 39648 bytes）
- `gen_api_index --check` OK（Rust 変更なし）
- 注意: テストは `autouse` フィクスチャで毎回 `set_lang("ja")` に戻す
  （グローバル言語状態のリーク防止）

## 次の山

ロードマップ Phase 1（Rapier 物理）はトリガー発生まで着手しない。残るは
ユーザー長期リストの SLG（3 本目ジャンル）— 移動範囲は `kagra.path.move_range`、
UI は `paged_menu`、セーブは `SlotStore` がそのまま使える。
