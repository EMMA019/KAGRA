# Horde Gates（ホードゲート）

Slug `zombie_rush`. A phone crowd-runner: the survivors run forward on their own,
a drag or the stick moves the whole crowd sideways, and each gate is a pair of
number operations. The head count is the rule. A zombie wave subtracts its
count when the crowd is strictly larger, and wipes the run otherwise.

## Loop

Five levels, 32–58 seconds of running each, in the world JSON (`game.levels`).
Speed and the size of the waves rise. A level ends on the boss wave. The clear
card has retry and next; the wipe card has retry and one continue. The last
next opens the page's result card.

Continue calls `ads.rewarded("continue")`. Level clear and a wipe call
`ads.break("level_end" | "death")`. Both the Rust hook (`game_core::ads`) and
`scripts/preview_site/ads.js` are no-ops: nothing is shown, and a reward is not
granted by the stub. The preview still hands back half the people (at least 4)
once per level so the button does something until a portal SDK replaces
`Ads::rewarded` and `KagraAds.ads.rewarded`.

## Picture

Sunny sky, saturated grass, a pale road, big gate numbers. The crowd and the
zombies are instanced pose meshes (one mesh per flipbook frame), the same idea
as Moonlit Swarm's many bodies: the GPU holds a handful of models and draws
many instances. Far bodies use a lower mesh. The on-screen cap is about 44 per
side on the mobile tier; the HUD number is the real count, which can be higher.

## What is animated

| | Run | Attack | Death |
|---|---|---|---|
| Zombies (Rikindle3D Male City Zombie, CC0) | 4 baked frames of Zombie Running | 1 baked frame of Zombie Attack | 1 baked frame of Zombie Dying |
| Survivors (Rocketbox Military_Male_01, MIT) | 4 baked frames of `m_run_neutral` | a procedural lean | a procedural fall |

The soldier library has no attack or death clip. Those are not retargeted from
the zombie, and no Mixamo file is in the repo. A little shamble on the
transform keeps neighbours from stepping in lockstep.

## Assets

Decimated offline to about 4k triangles (about 1.1k for the far mesh), textures
baked down to small JPEGs inside the GLBs. Road dressing is MilkAndBanana's
CC0 traffic props. Credits and the MIT notice are in the ledger.
