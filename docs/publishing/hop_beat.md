# Hop Beat — store page draft / ストアページ案

Copy for a web-game portal listing (CrazyGames first, then GameMonetize and
GameDistribution). Nothing here has been submitted anywhere. Edit the wording
freely; the facts (controls, credits, AI statement) come from this repo.

Cover art: `docs/media/hop_beat/` (drawn by `tools/gen_assets/hop_cover.py`, CC0).
The sizes are the same eight as Neon Cat's table in `neon_cat.md`
(1920×1080, 1280×720, 800×1200, 800×800, 800×800 no text, 512×512, 512×384,
200×120 no text). Portals change these sizes; re-check the upload form before
using a file.

## English

**Title:** Hop Beat

**Tagline (one line):** Tap on the beat to hop a neon road. Land every gap in time with the music.

**Description:**
Hop Beat is a one-button rhythm runner. A round bunny hops down a neon road in
time with the music. Tap for a short hop, hold for a long one, and land each
gap on the beat. Hit the beat cleanly for PERFECT and GOOD ratings and build a
combo. You have three lives, and one comes back for every 12 clean landings.
Courses get tighter the further you go. A run takes a couple of minutes, and
the game keeps your best score.

**Controls**
- Phone or tablet: tap anywhere to hop, hold for a long hop.
- Keyboard: Space, ↑ or W to hop (hold for a long hop), P or Esc to pause, M to mute.
- Audio sync: if the hops feel late or early on your headphones, use the
  Audio sync − / + buttons on the pause screen.
- The game pauses by itself when you switch tabs or apps.

**Languages:** English, Japanese (picked from your browser; `?lang=en` or
`?lang=ja` forces one).

**Tags:** rhythm, music, one-button, runner, bunny, neon, arcade, casual, mobile

**Age / content:** all ages. No violence, no chat, no accounts, no in-app
purchases, no external links. Optional ads only where the portal provides them
(a break after a retry, and an optional "watch an ad to continue" once per run).

**How AI was used (disclosure):**
Hop Beat is built in the open on the KAGRA engine. An AI coding agent wrote
much of the code, the tests, the procedural music kit and the sound effects
(both are plain oscillator and noise maths in scripts in this repo, with no
samples and no audio generator) and the store art (drawn by a script, no image
generator). The game design, review and testing are by the developer. Nothing in the build calls an AI service at play time. The scripts
and run logs are public in the repository.

**Credits:** Engine and game: KAGRA (MIT). Music, sound effects and cover art:
generated in-repo (CC0 1.0, no attribution required).

## 日本語

**タイトル:** Hop Beat

**ひとこと:** ビートに合わせてタップでジャンプ。ネオンの道の穴を、音楽にぴったり合わせて越えよう。

**説明:**
Hop Beat は、ボタン 1 つで遊ぶリズムランナーです。まるいウサギが、音楽に合わせてネオンの道を
ホップします。タップで小ジャンプ、長押しで大ジャンプ。穴の手前でビートに合わせて着地すると
PERFECT や GOOD の判定が出て、コンボがつながります。ライフは 3 つで、きれいに 12 回
着地するごとに 1 つ回復します。進むほどコースはきびしくなります。1 プレイは数分ほどで、
ベストスコアが記録されます。

**操作**
- スマホ・タブレット: 画面のどこかをタップでジャンプ、長押しで大ジャンプ。
- キーボード: Space、↑、W でジャンプ（長押しで大ジャンプ）、P / Esc で一時停止、M でミュート。
- 音ズレ補正: イヤホンなどでジャンプのタイミングが遅い・早いと感じたら、一時停止画面の
  「音ズレ補正」の − / ＋ で調整できます。
- タブやアプリを切り替えると自動で一時停止します。

**言語:** 英語・日本語（ブラウザの設定で選択。`?lang=en` / `?lang=ja` で固定）

**タグ:** リズム, 音楽, ワンボタン, ランナー, ウサギ, ネオン, アーケード, カジュアル

**対象年齢:** 全年齢。暴力表現なし、チャット・アカウント・課金・外部リンクなし。広告は
ポータルが提供する場合のみ（リトライ後の休憩広告と、1 プレイ 1 回の「広告を見てつづける」）。

**AI の使用について:**
Hop Beat はゲームエンジン KAGRA の上で公開開発しています。コード、テスト、音楽ツールと
効果音（どちらもリポジトリ内のスクリプトによる波形計算で、サンプルや音声生成 AI は不使用）、
ストア画像（スクリプトで描画。画像生成 AI は不使用）の多くを AI コーディングエージェントが
書いています。ゲームの設計、確認、テストは開発者が行っています。プレイ中に
AI サービスを呼び出すことはありません。スクリプトとログはリポジトリで公開しています。

**クレジット:** エンジン・ゲーム: KAGRA (MIT)。音楽・効果音・カバー画像:
リポジトリ内で生成 (CC0 1.0、表記不要)。

## Check before you submit

- **Music authorship.** The music is synthesised by the repo's `kagra_music` kit; the
  ledger entry `project-hop-beat-music` records Emma's CC0 confirmation (2026-10-25).
  The AI statement above does not say who wrote the tunes; add that
  before submitting if the portal asks.
- The AI statement must match what the portal's form asks; some forms have a checkbox.
- Not verified on a real device: tap/hold feel, audio sync with Bluetooth
  headphones, Safari's gapless mp3 loop (it can add a few ms at the loop point),
  and the ad flow with a real SDK (the PR preview has no provider; use
  `?dev=1&ads=mock` to see the Continue button).
- Preview videos or GIFs have not been made.
