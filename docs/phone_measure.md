# スマホ実測ツール / On-phone measurement (`?dev=1`)

どのゲームでも使える共通部品。ページのURLに `&dev=1` を付けると、フレーム時間のオーバーレイと
A/B 記録ボタンが出る。数字はこの端末で測ったものだけを信じる（PCの数字は参考にならない）。
Genre-neutral: any game page, any kit. `scripts/preview_site/perf.js` is loaded by `play.html`
only for `?dev=1`; with no `dev=1` nothing is fetched, parsed or run.

## Emma の手順 (Pixel 10)

1. プレビューURLに `&dev=1` を付けて開く。例: `https://emma019.github.io/KAGRA/pr/<N>/play.html?game=hop_beat&dev=1`
   （キャッシュが怖いときは末尾に `&_b=<好きな数字>`）。
2. 充電ケーブルを抜き、画面の明るさは普段どおり、画面を消さない（スリープ設定は長めに）。
   端末の「スムーズディスプレイ」が 60 Hz 固定になっていないか、バッテリーセーバーが切れているかも確認。
3. **Record 60 s**: 押すだけ。ゲームの自動プレイ（bot）が走りを続けるので、触らなくてよい。
   ゲームオーバーになっても bot が次の走りを始める。約60秒（120Hz画面ならスキップ検証の15秒を足して約75秒）。
   タブを切り替えない。1ステップは「実時間 15 s」で、描画が止まった時間は表の `note` に出る。
4. **Find my limit**: 押すだけ（約 1〜2.5 分）。負荷を一段ずつ足して 60 fps が保てた最後の段を出す。
   GPU 時間も欲しいときは URL に `&backend=gl` を付けて開き直す（WebGL2 の `EXT_disjoint_timer_query_webgl2` を使う。
   WebGPU のままだと GPU 時間は「unavailable」になる）。
5. 表が出たら **Copy JSON** を押して、チャットに貼る。コピーできない端末では下にテキスト欄が出るので長押しで全選択してコピー。
6. 余裕があれば、ゲームを10分遊んでから表の下の `thermal:` 行も教えてほしい（熱での失速の目安）。

ボタン: `⇅` 上下の位置を切替、`-` 小さくする。

## What the overlay shows

| line | meaning |
| --- | --- |
| fps, p50 / p95 / p99 ms | rAF frame gaps over the last 5 s |
| jank `a/b` | frames over 2 display intervals since start (5 s window in brackets); hitch = over 50 ms |
| disp ~N Hz | display refresh, from the fast end (p10) of rAF gaps, snapped to 60/72/90/120/144 Hz |
| backend, scale cap, canvas, dpr | WebGL2 / WebGPU, render-scale ladder step, canvas pixels |
| sim / render ms (js) | JS-side time of `requestFrame` / `render` per frame. GPU time is not in it |
| draws/s | `render()` calls per second (halves with `render_skip` on 120 Hz) |
| heap | `performance.memory` (Chrome only, coarse); wasm memory is not in it |
| ovh | the overlay's own cost per update (ms) |
| thermal | last 30 s against the first 30 s of the session, frame time and CPU ms per frame |
| adapter | WebGPU `adapter.info` (vendor, architecture, device, description) or the WebGL2 renderer string |
| tier, switches | `quality`, `fx_post`, `fx_atmo`, `render_skip` |

## Record 60 s

Four 15 s steps, in this order, each step drops its first 1.5 s (pipeline rebuild after a switch):

| step | fx_post | fx_atmo |
| --- | --- | --- |
| all-off | off | off |
| post | on | off |
| atmo | off | on |
| both | on | on |

On a screen of 100 Hz or more, a fifth 15 s step `both+skip` repeats `both` with `render_skip` on.
Switches go back to their values afterwards. Games without switches (Neon Cat today) get one `as-is` window.
The switches are a runtime setter (`WasmSession.setSwitch(name, on)`, `switchesJson()`) on the HOP-BEAT kit;
other kits can add the same two methods and appear in the plan with no page change.

The result table has, per step: fps, p50 / p95 / p99 frame gap (ms), jank frames, dropped-vsync %, mean JS CPU ms
per frame, draws per second, and a grade against the display interval `T`:
good = p50 <= 1.05 T, p95 <= 1.3 T, p99 <= 2 T and under 1 % dropped; ok = p50 <= 1.5 T and p95 <= 2 T; else poor.

### Robustness: a step is a wall time, and the run keeps going

- Each step is a **fixed wall time**. A rAF gap over 1 s (hidden tab, a long stall, screen off) is not a frame; before it was
  dropped silently, which is how a "15 s" step could hold 278 frames. Now the step row carries `wallMs`, `coverage` (drawn time /
  wall time), `stalls`, `stallMs`, `maxGapMs` and `hiddenMs`, and a `note` such as `drawing stopped for 2.5 s (1x)`. A row
  with `coverage < 0.75` or a hidden page has `stalled: true`.
- The recorder keeps a **run going** while it measures: if the game has an `autoplay` switch (HOP-BEAT does, a bot that plays the
  Perfect line, starts a run from the title and restarts after a game over) it turns it on for the recording and back off after.
  Rows have `bot`, `runs` (runs started during the step), `runEnds` and `playPct` (share of the step with a run in `play`); a
  note says so when a run was going less than 60 % of the time. A game with no bot gets a note that nobody was playing.
- A paused run (the page pauses itself when the tab was hidden) is resumed after 1 s (`resumes`).
- The quality guard (see `docs/quality_tiers.md`) is held for the whole recording so the picture does not change under it.

## Find my limit

A headroom probe, for any game that has graphics tiers (the button is hidden otherwise). It takes the tier file's own numbers
(`qualityJson`) and walks up from the **safe** tier one knob at a time, tier by tier, each step on top of the one before. On a
phone with dpr 2.6 the ladder is:

`safe`, `scale 1.25`, `scale 1.5`, `bloom 1`, `atmo 0.75`, `particles 0.75` (= **mid**), `scale 2`, `atmo 1`, `particles 1`,
`shadows 2c` (= **high**), `scale 2.5`, `atmo 1.4`, `shadows 3c`, `ssao 4`, `soft shadows`, `taa` (= **max**).

Scale steps above the display's pixel ratio are left out (nothing changes). Every step runs 9 s (the first 2 s are dropped:
pipelines rebuild), with the bot playing. A step **holds** when it has at least 60 frames, p95 <= 17.2 ms (16.7 + 0.5 ms rAF
jitter), p99 <= 20 ms and was not stalled. The first step that misses ends the search; a step the page could not draw
(stalled / hidden) is measured once more before it counts. Afterwards the tier, the bot and the guard go back to what they were.

The report's `limit` object has `steps[]` (each with p95, p99, dropPct, `pass`, GPU ms), `lastGood`, `firstFail`, `complete`
(every step held), `reached` (the knob values of the last good step), `suggestedTier` (the highest tier whose every knob is at or
below `reached`; `max` is never picked by auto) and `gpu`.

**GPU time.** `session.render()` is wrapped in an `EXT_disjoint_timer_query_webgl2` query when the canvas is WebGL2 and the
browser offers the extension; each step then has `gpuMs`, `gpuP95Ms`, `gpuN`. On the WebGPU path the report says why there is
none: the browser's `timestamp-query` feature is noted (`gpu.webgpuTimestamp`), but KAGRA's web renderer does not request it
(timestamp queries are native-only in the Rust renderer today), so the page cannot read it. Open with `&backend=gl` to get a
number. Without either, `gpu.available` is false and the reason is in `gpu.why`. The timer only runs during a recording.

## Copy JSON

One line: `tool`, `v`, `when` (UTC, to the minute), `page` (game, build id, backend, whitelisted query keys only), `device`
(user agent, platform, cores, memory, dpr, screen, viewport, model from `userAgentData` when the browser gives it),
`adapter` (with `timestampQuery`), `display.hz`, `canvas`, `scale`, `quality`, `thermal`, `overlayMs`, `longTasks`, `steps[]`,
`limit` (null until Find my limit has run). `v` is 2.
No URL, no cookies, no storage, no account or network identifiers. Nothing is sent anywhere: the tool makes no network
request, and the clipboard / text area is the only way out.

## `render_skip`

On a 120 Hz screen the sim still steps at 60 Hz, so every second `render()` draws an identical picture. With
`render_skip` on, a frame where the sim did not step is not drawn (the canvas keeps the last picture), halving GPU and
render-call work with the same look.

- Default **off**. Turn on with `?render_skip=1` (or `0` to force off), or `"render_skip": true` at the top level of
  `world.json`. The URL wins.
- Never skips a frame the sim stepped, after a canvas resize (3 frames are redrawn), or for kits that step every frame.
- Open question until measured on the phone: whether anything animates on render-only frames (the HOP-BEAT picture
  is driven by sim state, so nothing should), and how much it saves (the `both+skip` row against `both`).

## Cost and shipping

- Without `?dev=1`: no script is requested, no wrapper is installed, no timer runs.
- Portal zips: `package_portal.py` cuts the `<kagra-devtools>` block out of `play.html`, never copies `perf.js`
  (`DEV_SCRIPTS` in `build_browser_preview.py` goes to the preview site only), and the page's own check also refuses
  to load it when the build flag is `portal`. A test fails the build if the block goes missing.
- With `?dev=1`: two wrappers, one rAF callback and a 4 Hz overlay update. `ovh` shows the overlay's own cost.

## Limits

- Frame gaps are what the page sees; a long GPU frame shows up as a gap. GPU time is measured only through the WebGL2 timer
  (`&backend=gl`); see Find my limit.
- Thermal needs a long run; one minute only says "n/a" or a weak hint. The ten-minute test is the real one.
- The heap number is JS only and quantised by the browser.
- Measure on battery, brightness fixed, same time of day, a few minutes after the phone has settled.

Tests: `tests/test_phone_measure.py` (perf.js under node with a virtual clock, `tests/data/perf_harness.mjs` and
`tests/data/limit_harness.mjs`: recorder, stalls, bot, ladder, GPU timer; build and portal checks) and
`kagra-shared/src/hop_beat.rs` (`picture_switches_flip_at_runtime_and_leave_the_sim_alone`).
