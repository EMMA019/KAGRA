# Moonlit Swarm — portal pack

Prepared for a CrazyGames-style listing. Nothing in this file has been submitted.

Preview (this branch's PR): the phone link is `https://emma019.github.io/KAGRA/pr/<n>/play.html?game=survivor_arena`.

## Short description (en)

Survive the moonlit swarm until dawn. Walk with one thumb, let the weapons fire, and pick one upgrade each level.

## Long description (en)

Moonlit Swarm is a two-minute arena survivor. A thumb on the stick walks the hero; every weapon fires on its own. Blue gems pull in as you pass them. Each level offers three cards — a weapon, a passive, or a heal — and the names keep their Japanese romaji (Crescent blades / Mikazuki no Yaiba, and the rest). Around 72 seconds the swarm closes into a ring. There is no boss in this build: last until the clock hits dawn, or fall and retry in one tap. Coins from a run unlock heroes on this device only.

## Tags

survival, arena, action, arcade, roguelite, one-thumb, casual

## Controls

- Phone: a virtual stick at the lower left. Slide it to walk. Weapons fire by themselves. Tap a foe to lock it (hybrid aim). Pause is the button at the top right.
- PC: WASD or the arrow keys to walk. Weapons still fire on their own. Esc or P pauses. The result card's Retry starts the next run.

## Age-rating notes

Not a submitted rating. Suggested band: 7+ / mild cartoon combat.

- Stylised Kenney characters. Hits flash and knock foes back. No blood, no dismemberment, no realistic violence.
- No text chat, no user accounts, no web requests in the default build.
- Coins and the shop stay on the device. The rewarded "double coins" button and the retry commercial call the portal adapter only. The adapter is a no-op unless a portal is switched on (`?portal=crazygames` or the build meta). No script is loaded otherwise.
- No gambling, no loot boxes bought with money, no external links on the play page.

## Build size

CrazyGames' guide asks for an initial download of about 50 MB or less. Measured on this branch, 2026-11-10:

| Piece | Raw | gzip -9 |
|---|---|---|
| `kagra_shared.wasm`, release, `opt-level = s`, `name` and `producers` sections removed | 8.77 MiB (9,200,016 B) | 2.94 MiB (3,080,754 B) |
| Manifest asset packs (55 files: Kenney models, floor, clips) | 2.43 MiB (2,551,380 B) | not separately gzipped; audio is the bulky part |
| World JSON | 30 KB | negligible |

The page loads those files before the first frame (the boot bar is that download), so the initial download is about **12 MiB raw / under 6 MiB gzip**, wasm plus assets plus the small page scripts. That is well under 50 MB. The packager script (`scripts/package_portal.py`) did not finish here: this environment has `python3` but not `python`, and `wasm-bindgen` is not on `PATH`, so the JS glue was not emitted. The wasm number above is the release build the script compiles, with the same two custom sections removed that `wasm-bindgen --remove-name-section --remove-producers-section` drops.

No cut is required to clear 50 MB. A later pass could still shrink the wasm (it is larger than the 6.6 MB raw noted in the design doc's earlier perf pass) or the music file, if a portal's budget is tighter than 50 MB.

## Thumbnail candidates

Bright frames, for a catalogue tile:

1. The launcher icon `assets/icons/moonlit_swarm.png` (original, CC0, drawn by `tools/gen_assets/moonlit_swarm_icon.py`).
2. Gameplay: the hero on the lighter paver floor, with the corner columns and candle posts.
3. The level-up card, English, with the heading clear of the subtitle.

Boss art is out of scope for this pack.
