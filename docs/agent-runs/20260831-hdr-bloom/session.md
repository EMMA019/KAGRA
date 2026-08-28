# Session

## 移植元の調査

- `kagra-core/src/renderer/bloom.rs`（wgpu 0.19）: 閾値 + ソフト膝の抽出、
  半解像度 Rgba16Float ×2、ガウシアン 9 タップ H/V、composite で
  sharp + bloom*intensity。`BLOOM_SHADER` は `renderer/shaders.rs`。
- 0.30 側 `render/mod.rs` は 3D + HUD を 1 パスで swapchain/offscreen に
  直書き（sRGB）。`shader3d.wgsl` の fs_main が最後に `tone_map()`（exposure +
  ACES）を適用していた。

## 実装の往復

1. **最初の試み（0.19 と同構成）**: 3D+HUD を sRGB フレームテクスチャに描き、
   Bloom を適用。白四角のオフスクリーンテストは通ったが、
   **crest_isle_world では効かなかった** — fs_main が ACES 適用後の値しか
   書かず、ハイライトが 1.0 に圧縮されて「閾値 0.85 超えの画素ゼロ」
   （max 輝度 228/255）。原因はトーン後の抽出。
2. **HDR フレーム化**: 3D パスを Rgba16Float（線形、トーンなし）に。スカイも
   トーンなし。Bloom（extract → blur → composite）を HDR 空間で。
   composite で exposure + ACES + sRGB を適用。crest で diff 902 px
   （最大 20）が出る = コイン/水面ハイライトがにじむ。
3. **HUD の分離**: HUD は Bloom 対象にしない（トーン後の色を保つ）。
   composite 後に同じターゲットへ Load で重ねる。
4. **躓き 1**: PowerShell の `Set-Content` で `mod.rs` の日本語コメントが
   文字化け（Shift-JIS 誤読）→ `git checkout` で戻し、edit ツール
   （UTF-8 安全）で組み込みをやり直し。**以後、ファイル書き換えは
   PowerShell を使わない**。
5. **躓き 2**: 最初の bloom テストは HUD の白四角だったが、HUD が Bloom
   対象外になったため効かない → 3D の白 box（Solid、ambient=1 → リニア 1.0）
   に変更。
6. **躓き 3**: `upload_compile_meshes` は heightfield 無し dump で MeshId が
   飛び dense チェックに失敗 → box だけ個別 `upload_mesh`。

## 最終状態

- 3D パス → 線形 HDR フレーム（`BloomPass::frame_view`）
- Bloom: extract/blur（HDR 半解像度）→ composite（sharp + bloom*intensity →
  exposure → ACES → sRGB ターゲット）。intensity 0 でも composite が通る
  （= 見た目は従来通り exposure+ACES）
- HUD パス → composite 後のターゲットに Load
- `set_bloom(threshold, intensity)` 公開。play_world / offscreen example は
  デフォルト 0.85/0.35、offscreen は `--no-bloom` で比較可能

## 次（未実施）

- 空のにじみ（crest の地平線が +10 程度）は「空気感」として許容。夕日シーンで
  確認して、気になるならスカイの輝度クランプか抽出閾値の調整
- FXAA / 完全 MToon は次のスライス
