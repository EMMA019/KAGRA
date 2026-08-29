# Result — Phase 5 完了（UI 成熟）

汎用エンジン化ロードマップ Phase 5（UI 成熟）が完了した。

## 設計

`kagra/ui2d.py` に 2 つの部品を追加した。純ロジック部分は kagra_shared 非依存
（近似幅でも動く）で、hud dict への変換は既存部品と同じ契約。

| API | 用途 |
|---|---|
| `page_count(n, per_page)` | ページ数（純ロジック） |
| `clamp_scroll(offset, n, visible)` | スクロールオフセットのクランプ（純ロジック） |
| `scroll_window(lines, offset, visible, ...)` | ログ末尾表示のスクロール窓。offset 補正済みの行を hud texts に。`_offset` を返す |
| `paged_menu(options, selected, x, y, w, *, per_page)` | 長い選択肢のページ送り + 「n/N」表示。`_page` / `_pages` を返す |

## torneko 配線（実証）

手動で末尾 3 行を ` / ` 連結していたログ表示を `scroll_window` に置換し、
履歴を 8 行保持（表示は末尾 3 行のパネル付き窓）。ヘッドレス決定論を
**2 回実行の MD5 一致**で再確認（stdout JSON も一致）。

## verify

- pytest: 606 パス（test_ui2d.py 8 件追加: page_count / clamp_scroll /
  scroll_window の tail 表示とクランプ / paged_menu のページ切替・カーソル
  相対位置・最終ページ・空）
- torneko ヘッドレス: `--seed 12345 --turns 300` 2 回 → save MD5 一致
- `gen_api_index --check` OK（Rust 変更なし）
- 落とし穴: ヘッドレスランナーが `--seed` を保存ファイル名として誤解釈し
  `--seed` ファイルを生成 → コミットに混入。削除 + `.gitignore` で再発防止

## 次の山

Phase 6 — セーブ深化（オートセーブ / スロット複数 / バージョン付きマイグレーション）。
バニーガーデンとトルネコのセーブを共通 `kagra.save` に寄せる。
