# Publishing a KAGRA game to a web-game portal

(The PyPI release guide is `docs/PUBLISHING.md`. This file is about web portals
such as CrazyGames, GameMonetize and GameDistribution. The store copy for Neon
Cat is `docs/publishing/neon_cat.md`; Hop Beat is `docs/publishing/hop_beat.md`.)

Status: **prepared, not submitted.** The shim, the packager, the cover art and
the store text are in the repo and tested with fake SDKs. Nothing has run on a
real portal, no account exists, and no agreement is signed.

## What was built

| Piece | File |
|---|---|
| Portal host shim and provider adapters | `scripts/preview_site/portal.js` |
| English / Japanese strings | `scripts/preview_site/i18n.js` |
| `platform.ads` in the manifest (validated in Python, normalised again in JS) | `kagra/platform_spec.py`, `kagra/distribute.py`, `games/neon_cat_minimal.json` |
| Single-game zip with a size check | `scripts/package_portal.py` |
| Cover art generator (stdlib only) | `tools/gen_assets/cover_art.py` → `docs/media/neon_cat/*.png` |
| Store text (EN/JA), AI statement, controls | `docs/publishing/neon_cat.md` |
| Tests | `tests/test_portal_shim.py` (+ `tests/data/portal_harness.mjs`), `tests/test_package_portal.py`, `tests/test_platform_spec.py`, `tests/test_cover_art.py` |

## Choosing a portal

The page reads the choice in this order:

1. build flag `<meta name="kagra-portal" content="crazygames|gamemonetize|none">`
   (the packager writes it; when it names a portal it wins, so a portal's iframe
   cannot be pointed at another SDK with a query string);
2. `?portal=crazygames|gamemonetize|none`;
3. `none`.

`none` is the default. It loads no SDK and makes **no external request**; the
tests and the packager both check this. The SDK script URL appears only in
`portal.js` (`SDK_URL`), and the script tag is added only after a provider is
picked. A blocked or missing SDK (an ad blocker) never stops the game: the host
falls back to no ads and the game plays on.

`?portal_id=<id>` / `<meta name="kagra-portal-id">` carries the GameMonetize
game id. It is not a secret.

### What each provider does

| Host call | CrazyGames (SDK v3) | GameMonetize | `none` / stub |
|---|---|---|---|
| init | `CrazyGames.SDK.init()` | `SDK_OPTIONS`, waits for `SDK_READY` | resolves at once |
| loading | `game.loadingStart/Stop` | – | – |
| play / interrupt | `game.gameplayStart/Stop` (never on blur alone) | – | – |
| midgame ad | `ad.requestAd("midgame", …)` | `sdk.showBanner()`; game paused on `SDK_GAME_PAUSE` and resumed on `SDK_GAME_START` | skipped |
| rewarded ad | `ad.requestAd("rewarded", …)` | not offered (no rewarded API) | not offered |

`createStubProvider(name)` in `portal.js` is a logging provider for a portal
whose SDK is not wired yet: copy it, add the three SDK calls, add a name to
`PORTALS` and `SDK_URL`.

During an ad the host mutes the game, pauses it, blocks input, and restores the
previous mute state afterwards (an ad that errors or times out also restores).
Gameplay start/stop are never sent twice in a row and never during an ad.

## `platform.ads`

```json
"platform": {
  "ads": {
    "commercial_on": ["retry"],
    "rewarded": [],
    "min_interval_secs": 90
  }
}
```

- `commercial_on`: where a midgame ad may be asked for. `retry` = the player
  taps retry on the result card (heading back into play, the break portals
  prefer). `game_over` = as the result card comes up. Default `["retry"]`.
- `rewarded`: slots `{slot, at: "game_over"|"title", effect: "none", per_run: 1–9}`.
  The shim and the validator support them, but **the Neon Cat page shows no
  rewarded button**: a rewarded ad must pay out something, and the only payout
  that exists without an engine change is `effect: "none"`. Adding a revive or
  extra-life needs a spec effect in the simulation (`neon_cat.rs`, `game_spec.rs`),
  which this change did not touch. Follow-up.
- `min_interval_secs`: the shortest gap between two midgame ads, 0–3600, default 90.

It lives in the JSON manifest layer (`GameManifest.platform`, copied into
`bundle.json` and read by the page), **not** in the Rust / Python `GameSpec` /
`RunnerSpec` mirrors. That was deliberate: an ad break is a page and portal
matter, not a rule of the simulation, and a spec field would change every baked
fixture and the Rust mirror. Deferred: if the engine ever needs to know (a
rewarded revive), add `platform.ads` to the spec then, mirrored in Rust and
Python in one change.

HOP-BEAT now has a second rewarded slot, `double_coins` (`effect: "none"`: the
page, not the simulation, pays it out). It is offered only where the host can
(`canOfferRewarded`), so on GameMonetize and in a no-portal build the button is
absent. Nothing is skipped or forced: Retry works without it. See
[`docs/meta_core.md`](../meta_core.md).

## Page behaviour for portals (all builds)

- Pauses on `visibilitychange`, `pagehide`, window blur, `Esc` and `P`; a pause
  card resumes it.
- Audio is muted during ads and while paused; the mute button state is kept.
- Wheel, PageUp/PageDown, pinch and context-menu scrolling are blocked, as the
  portals' review lists ask.
- Strings: every UI string is in `i18n.js` (English and Japanese). `?lang=en|ja`
  forces one; otherwise `navigator.language` decides, and anything that is not
  Japanese gets English.
- `?debug=1` and `?backend=` are ignored unless `?dev=1` is also set, so a
  portal build carries no debug overlay by default.

## Building the zip

```bash
./scripts/build_wasm.sh                       # wasm-bindgen output in kagra-shared/www/pkg
python scripts/package_portal.py --game neon_cat_minimal --portal crazygames
python scripts/package_portal.py --game neon_cat_minimal --portal crazygames --wasm-opt
python scripts/package_portal.py --help       # --out, --keep-tree, --strict, budgets
```

The zip has `index.html` at the root, relative paths only, that game's files
only (no catalog, no other games, audio pruned to the clips the game uses), and
no external request except the SDK script when a portal is built in. The
packager scans every text file for `http(s)://` URLs and fails on any that is
not on its short allowlist.

### Size (Neon Cat, measured 2026-10-24, Asia/Tokyo)

"initial" = what loads before the first frame; "total" = the whole zip content
(audio streams in after the first frame).

| Build | initial raw | initial gzip | total raw | total gzip | zip file |
|---|---|---|---|---|---|
| plain wasm | 9.23 MB | 2.82 MB | 9.90 MB | 3.40 MB | 3.41 MB |
| `--wasm-opt` (`-Oz`) | 7.10 MB | 2.61 MB | 7.77 MB | 3.18 MB | 3.19 MB |

Budget checked: 5 MB initial / 8 MB total. The packager warns when the raw or
the gzip number is over, because portals disagree on which they count (Poki
documents 5 / 8 MB without saying which). Today: **gzip passes both with
room; raw passes total only with wasm-opt and still misses 5 MB initial.** The
wasm itself is 8.55 MB raw → 6.42 MB with wasm-opt. Getting raw initial under
5 MB would need a smaller wasm build (`opt-level="z"`, `panic=abort`,
`codegen-units=1`, `strip`, dropping render features the page does not use), not
tried here.

### wasm-opt

- Works: `wasm-opt -Oz` (binaryen version_133) on the `wasm-bindgen` output
  gives −2.1 MB raw, −0.24 MB gzip; the optimised build booted and ran a game in
  headless Chrome, with the same console messages as the plain build.
- Not enabled in CI or in `scripts/build_wasm.sh`. To use it in CI the runner
  needs binaryen: `apt-get install binaryen` is version 120 on `ubuntu-latest`
  (older than the one tested; retest) or download a pinned release tarball and
  verify its sha256. It takes about 7–9 s. A job like `portal-zip` that builds the wasm,
  runs `package_portal.py --wasm-opt --strict` and uploads the zip as an
  artifact is feasible and costs no money on the free runners; it is not added
  here so the PR does not change CI time or fail on a size budget nobody has
  settled yet.
- The Pages preview keeps using the plain wasm.

## What is unverified

- The **real** CrazyGames / GameMonetize SDKs on the real portals: the adapters
  follow the published SDK docs and are tested only against fakes that mimic
  them. Use CrazyGames' QA tool and a draft upload for the first real check.
- A midgame ad on a real page: in the headless Chrome used here the engine
  crashes on retry with `recursive use of an object detected` after a wgpu
  validation error under the software renderer, **also on the untouched master
  page**, so the retry-then-ad path was checked in the node harness and by
  event sequence, not through a real run in a browser.
- Real phones, real touch scrolling, real autoplay and audio-unlock rules.
- Portal review outcomes, per-portal thumbnail and size rules (they change),
  whether raw or gzip is counted.

## What only Emma can do

- Create the developer accounts (CrazyGames developer portal first; then
  GameMonetize, GameDistribution), pass any identity checks, and accept the
  terms. Keep passwords and tokens out of chat and out of the repo.
- Read and accept each portal's agreement. Check exclusivity wording yourself
  (CrazyGames is non-exclusive in practice; Poki's default is a 5-year web
  exclusivity, which would conflict with the others; see the research note).
- Payments and tax: payout method, any tax forms the portal asks for, and how
  the income is reported in Japan (ask a tax adviser; this repo gives no advice).
- Choose to submit: upload the zip, pick the covers, paste the store text, answer
  the AI-use question, and make the preview video or GIF the forms ask for.
- Decide what to do about rewarded ads (needs an engine change) and about
  raw size (needs a wasm size pass).
