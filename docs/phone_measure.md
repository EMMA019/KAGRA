# スマホ実測ツール / On-phone measurement (`?dev=1`)

どのゲームでも使える共通部品。ページのURLに `&dev=1` を付けると、フレーム時間のオーバーレイと
A/B 記録ボタンが出る。数字はこの端末で測ったものだけを信じる（PCの数字は参考にならない）。
Genre-neutral: any game page, any kit. `scripts/preview_site/perf.js` is loaded by `play.html`
only for `?dev=1`; with no `dev=1` nothing is fetched, parsed or run.

## Emma の手順 (Pixel 10)

1. プレビューURLに `&dev=1` を付けて開く。例: `https://emma019.github.io/KAGRA/pr/<N>/play.html?game=hop_beat&dev=1`
   （キャッシュが怖いときは末尾に `&_b=<好きな数字>`）。
2. 充電ケーブルを抜き、画面の明るさは普段どおり、画面を消さない（スリープ設定は長めに）。
3. ゲームを始めて、遊びながら **Record 60 s** を押す。約60秒（120Hz画面ならスキップ検証の15秒を足して約75秒）、
   そのまま遊び続ける。タブを切り替えない。
4. 表が出たら **Copy JSON** を押して、チャットに貼る。コピーできない端末では下にテキスト欄が出るので長押しで全選択してコピー。
5. 余裕があれば、ゲームを10分遊んでから表の下の `thermal:` 行も教えてほしい（熱での失速の目安）。

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

## Copy JSON

One line: `tool`, `v`, `when` (UTC, to the minute), `page` (game, build id, backend, whitelisted query keys only), `device`
(user agent, platform, cores, memory, dpr, screen, viewport, model from `userAgentData` when the browser gives it),
`adapter`, `display.hz`, `canvas`, `scale`, `quality`, `thermal`, `overlayMs`, `longTasks`, `steps[]`.
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

- Frame gaps are what the page sees; a long GPU frame shows up as a gap, but GPU time itself is not measured.
- Thermal needs a long run; one minute only says "n/a" or a weak hint. The ten-minute test is the real one.
- The heap number is JS only and quantised by the browser.
- Measure on battery, brightness fixed, same time of day, a few minutes after the phone has settled.

Tests: `tests/test_phone_measure.py` (perf.js under node with a virtual clock, build and portal checks) and
`kagra-shared/src/hop_beat.rs` (`picture_switches_flip_at_runtime_and_leave_the_sim_alone`).
