Emma wants Crest Isle (open-world collectathon, PR branch cursor/open-world-collectathon-5395 / https://github.com/EMMA019/KAGRA/pull/73) playable on MOBILE next. Desktop is done; she checks at 18:00 JST.

Start FROM that branch (starting_ref: cursor/open-world-collectathon-5395) so you inherit the game + Kenney assets.

## Constraints (KAGRA product)
- kagra-shared + mobile/ is a SEPARATE renderer from Python kagra-core. Do NOT merge the two renderers into one process.
- Existing mobile/ is a driving demo (roads/trucks). Don't pretend Python VRM runs on iOS as-is.
- Goal: a playable mobile slice of the SAME fantasy — grassland, sea, mountains, run/jump, collect crests/coins — using kagra-shared / wasm / Android (and iOS shell if feasible).
- Touch controls (virtual stick + jump). CC0 Kenney assets already in examples/assets/open_world/ — reuse, don't pirate Nintendo.
- Keep 5MB Python wheel story intact (don't dump Kenney into the pip package).

## Deliver
A PR (or commits on a branch off the open-world branch) with how to run on Android and/or wasm. Document in README.ja.md a mobile sample line. Agent log under docs/agent-runs/.

If full VRM on mobile is impossible this sprint, ship a Kenney-prop player capsule that PLAYS the same collectathon on Android debug APK + wasm, and say clearly what is/isn't VRM.

Follow AGENTS.md. pytest / cargo as applicable. No Rapier/OSM.
