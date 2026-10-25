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

## Models

The bunny, the three enemies (slime, beetle, bat), the three rooftop towers (with lit-window panels) and the lamp post are
`assets/models/hop_beat/*.glb`, written by `python3 tools/gen_assets/hop_models.py`
(plain Python, CC0, no input meshes; `--check` fails on drift). They add about 0.64 MB
raw to the bundle and are listed in the asset ledger as `project-hop-beat-models`.

Every model is optional. If a file is missing or does not parse, the page logs one
warning and HOP-BEAT draws that part with its primitives (ball hero, box towers, box
posts). A model the bundle could not fetch does not stop the game from starting.

## Enemies

The enemy logic is a shared, genre-neutral module (`game_core::enemies`). See [enemy_kit.md](../enemy_kit.md) for the module boundary and how another kit plugs in.

Three creatures, all defined in the spec (`enemies.types`, `course.phrases[].items[].enemy`,
`course.phrases[].weight`), none hard-coded. They stand and move on the beat grid, so
getting past one is still a rhythm action, and the one button is unchanged.

| Kind | Unlocks | Behaviour | What the player does |
|---|---|---|---|
| slime | level 0 (bars 0-8) | sits on the track | tap over it, or land on it |
| beetle | level 1 (bars 8-16) | walks towards you one step on every half beat | the same, but its walk shifts where it is when you land |
| bat | level 2 (bars 16-24) | hovers at head height above a spot where a hold hop climbs | tap: a tap hop stays under it. A hold hop flies into it |

- Landing on a slime or beetle from above is a **stomp**: +150, a short bounce that
  comes down on the next half beat, a "STOMP!" and the `stomp` cue. It does not need
  a perfect press; an early one stomps, a late one runs into its side and costs a life.
- Running into a side, or climbing into a bat, costs a life (death reason `enemy`).
- Falling into a gap stays a plain fail. Blocks are as before.
- A bat needs no press of its own and is not judged. It is only there to punish a hold.
- The checker (`HopSpec::course_problems`, run on every spec load and in `hop_measure`)
  plays every phrase with the hero at +-`judge.perfect_ms` (capped at 100 ms) around each
  ideal press and refuses a phrase where any offset takes damage, an unknown or still
  locked enemy, a stompable enemy without `rest_beats` and `bounce_beats` of room, or an
  item that runs past the end of the phrase. `kagra/hop_beat.py` mirrors the spec and the
  rules that need no simulation (`phrase_problems`).
- Measured, `docs/measure/hop_beat_baseline.json`: the perfect bot loses no life over 150 s
  on 8 seeds; the 25 ms human bot loses about one; the idle, hold-all and spam bots all
  die. Enemy windows are wide (about 350-420 ms for a tap over a slime), so enemy phrases
  are easier than gaps and the course gets denser per bar only slowly: about 0.6
  obstacles per bar at the start, about 1.7 late.
- The sounds are the existing effects plus `stomp.wav` and `warn.wav` (original, from `hop_audio.py`, CC0). Cue volumes are set under the music (repeating cues low, rewards and damage high; see `cue_volume`); `warn` sounds once when a flyer comes within 26 m.
  Each enemy model falls back to primitives (a dome, a shell, two wings) if its glb is missing.


## Falling

Falling into a gap is a plain fail (no catching, no rescue); the picture just plays it out. The bunny keeps its momentum over the ledge, the picture holds for a beat, dust and a glint puff up, `fall.wav` (a falling whoosh, original, CC0) sounds, and the bunny tumbles, stretches and drops while the camera trails behind and lets it fall out of frame. The edge of the screen darkens, and the game over card comes `fall.card_delay_secs` after the ledge. With lives left the bunny now comes back after a breather (see Recovering below); a revive puts it on a safe tile and gets the same READY count.

Everything is in the JSON `fall` block (all keys optional, Python mirror in `kagra/hop_beat.py`):

| key | default | range | meaning |
| --- | --- | --- | --- |
| `edge_stop_secs` | 0.07 | 0 to 0.5 | picture hold at the ledge |
| `tumble_turns_per_sec` | 1.1 | 0 to 4 | turns per second while falling |
| `stretch` | 0.3 | 0 to 1 | stretch along the drop |
| `carry` | 0.55 | 0 to 1 | how far it keeps running after the run ends (about `carry`/3 s of running) |
| `cam_lag` | 0.6 | 0 to 1 | how much of the speed the camera loses (1 = it stops) |
| `slow_scale` | 0.5 | 0.1 to 1 | speed of the drop after the run has ended |
| `card_delay_secs` | 1.0 | 0.3 to 3 | game over card, counted from the ledge |
| `vignette` | 0.7 | 0 to 1 | screen edge darkening |
| `dust` | 8 | 0 to 32 | dust particles at the ledge (half on the low tier) |

This is picture only: the run, score and baselines are unchanged, and the primitive fallback and every quality tier draw it too.

## Recovering after a fall

A fall that costs a life (and a Continue after a game over) no longer drops the bunny straight back in. The JSON `recover` block (Python mirror in `kagra/hop_beat.py`):

| key | default | range | meaning |
| --- | --- | --- | --- |
| `enabled` | true | | false = the old instant second wind |
| `drop_secs` | 0.9 | 0.2 to 3 | the drop plays out (slowed, music faded) before anything else |
| `ready_beats` | 3 | 0 to 8 | READY / 3 / 2 / 1: the world is held, one tick per beat; 0 = no count |
| `invuln_secs` | 2.4 | 0 to 10 | the bunny blinks and cannot be hurt this long once play resumes |
| `clear_beats` | 4 | 0 to 32 | beats of empty slab laid ahead of where it comes back |

Order: drop, then the bunny is put on the next bar line on empty slab and the world holds for `ready_beats` beats (blinking, a tick on each beat), then the music restarts on that bar line (the audio clock is re-synced by `begin()`) and play resumes with the blink. Nothing in the sim runs during the hold (song time, scroll and obstacles stand still), so scores, baselines and the measure bots are unchanged (the bots play with it off). A Continue uses the same READY count with `revive.invuln_secs` and `revive.clear_beats`.

## Flyers over gaps (fairness)

A bat is only there to punish a hold hop, so a tap must always be safe under it. `tap_window_secs` (a press that is let go within it is a tap) is now 0.16 s instead of 0.11 s: a thumb on glass often stays down 120-150 ms, and under a bat that used to turn into a hold hop that flew into it. The placement rules are also stricter: no flyer over a gap (the whole hop across the gap, plus 100 ms of press slack either way, must be clear of the bat: `HopSpec::course_problems` and `phrase_problems` in Python), a phrase with a flyer must hold at +-100 ms, and `HopSpec::seam_problems` plays every ordered pair of phrases with a bat in it back to back at +-100 ms (or the narrower of the two phrases' own windows). Tests replay 2000 seeds with every modifier (plain, sprint, no_breaks, midnight, stomp_party) and check each seam the generator lays.

## Look and finish

The picture is a JSON look preset (`neon_night`, `dusk`, `plain`) with overrides, plus a hero `skin` id; see [look_presets.md](../look_presets.md). `?fx_post=0|1` and `?fx_atmo=0|1` switch the finishing pass on the preview page; `?skin=` and `?look=` pick the ids.
