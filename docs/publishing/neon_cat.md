# Neon Cat — store page draft / ストアページ案

Copy for a web-game portal listing (CrazyGames first, then GameMonetize and
GameDistribution). Nothing here has been submitted anywhere. Edit the wording
freely; the facts (controls, credits, AI statement) come from this repo.

Cover art: `docs/media/neon_cat/` (drawn by `tools/gen_assets/cover_art.py`, CC0).

| File | Size | Use |
|---|---|---|
| `landscape_1920x1080.png` | 1920×1080 | CrazyGames landscape cover (16:9) |
| `landscape_1280x720.png` | 1280×720 | general 16:9 cover, GameMonetize |
| `portrait_800x1200.png` | 800×1200 | CrazyGames portrait cover (2:3) |
| `square_800x800.png` | 800×800 | CrazyGames square cover |
| `square_512x512.png` | 512×512 | GameDistribution / GameMonetize icon |
| `landscape_512x384.png` | 512×384 | GameDistribution 4:3 |
| `square_800x800_notext.png` | 800×800 | portals that ban text on thumbnails (Poki wants ≥628 square, no text) |
| `landscape_200x120_notext.png` | 200×120 | GameDistribution small thumbnail |

Portals change these sizes. Re-check the upload form before using a file; the
generator takes any size you add to `IMAGES`.

## English

**Title:** Neon Cat

**Tagline (one line):** Run a rainy neon street as a cat. Grab coins, dodge signs, beat your best.

**Description:**
Neon Cat is a one-thumb endless runner set on a rainy night street. Switch
lanes, jump the barriers, roll under the awnings and collect coins before the
street catches you. Every run has three small goals (collect 40 coins, pass 5
gates, land one perch) and the game keeps your best score. A run takes about a
minute, so it fits a short break.

**Controls**
- Phone or tablet: keep a finger down and drag left or right to switch lanes;
  flick up to jump; flick down to drop or roll.
- Keyboard: A / D or ← → to move, Space to jump, S or ↓ for down (drop or roll), J for
  the action, P or Esc to pause, M to mute.
- The game pauses by itself when you switch tabs or apps.

**Languages:** English, Japanese (picked from your browser; `?lang=en` or
`?lang=ja` forces one).

**Tags:** runner, endless, cat, neon, arcade, one-touch, casual, mobile

**Age / content:** all ages. No violence beyond bumping into a sign, no chat, no
accounts, no in-app purchases, no external links.

**How AI was used (disclosure):**
Neon Cat is built in the open on the KAGRA engine, and an AI coding agent wrote
much of the code, the tests, the placeholder sound effects and music (generated
by scripts in this repo), the street textures and this store art (also drawn by
scripts, no image generator). The game design, direction, review and testing
are done by the developer. The 3D models are Kenney's CC0 packs. Nothing in the
build calls an AI service at play time. The scripts, prompts and run logs are
public in the repository, so every step can be shown on request.

**Credits:** Engine and game: KAGRA (MIT). 3D models: Kenney (CC0 1.0). Sounds,
music, textures and cover art: generated in-repo (CC0 1.0).

## 日本語

**タイトル:** Neon Cat

**ひとこと:** 雨の夜のネオン街をネコで走ろう。コインを集めて、看板をよけて、自己ベストを更新。

**説明:**
Neon Cat は、雨の夜の街を走る片手プレイのエンドレスランナーです。レーンを切り替え、
ジャンプで柵を越え、ひさしの下はころがって抜けて、コインを集めましょう。1 回のプレイには
3 つの小さな目標（コイン 40 枚、ゲート 5 つ、屋根の上に 1 回着地）があり、ベストスコアが
記録されます。1 プレイは 1 分ほどで、すきま時間に向いています。

**操作**
- スマホ・タブレット: 指を置いたまま左右にドラッグでレーン移動、上フリックでジャンプ、
  下フリックで急降下・ころがり。
- キーボード: A / D または ← → で移動、Space でジャンプ、S または ↓ で下（急降下・ころがり）、
  J でアクション、P / Esc で一時停止、M でミュート。
- タブやアプリを切り替えると自動で一時停止します。

**言語:** 英語・日本語（ブラウザの設定で選択。`?lang=en` / `?lang=ja` で固定）

**タグ:** ランナー, エンドレス, ネコ, ネオン, アーケード, ワンタッチ, カジュアル

**対象年齢:** 全年齢。暴力表現なし（看板にぶつかるだけ）、チャット・アカウント・課金・外部リンクなし。

**AI の使用について:**
Neon Cat はゲームエンジン KAGRA の上で公開開発しており、コード、テスト、仮の効果音と音楽
（リポジトリ内のスクリプトで生成）、街のテクスチャ、このストア画像（これもスクリプトで描画。
画像生成 AI は不使用）の多くを AI コーディングエージェントが書いています。ゲームの設計、
方針、確認、テストは開発者が行っています。3D モデルは Kenney の CC0 パックです。プレイ中に
AI サービスを呼び出すことはありません。スクリプトとログはリポジトリで公開しているので、
求められれば過程を示せます。

**クレジット:** エンジン・ゲーム: KAGRA (MIT)。3D モデル: Kenney (CC0 1.0)。
音・音楽・テクスチャ・カバー画像: リポジトリ内で生成 (CC0 1.0)。

## Check before you submit

- The AI statement above must match what the portal's form asks; some forms have
  a dedicated checkbox. Answer it the same way.
- Controls text assumes the swipe layout is the default on touch. Confirm on a
  real phone (not verified here).
- Preview videos or GIFs (portals ask for them) have not been made.
