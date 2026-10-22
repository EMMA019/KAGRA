KAGRA (Rust kagra-shared + Python). Read AGENTS.md, kagra-shared/src/genre_session.rs and the existing GenreSession bindings on master first.

Goal (Phase 2-4, step 1 of online play; smartphones are the main target):
1. Local multiplayer: let a GenreSession take per-player input (player slots) so two people can play on one device. Wire it into one existing genre that suits versus play (sports 1v1 or action/fight) as examples/versus_local.py: split controls on one keyboard, and on touch the screen halves for P1/P2.
2. Online foundation, logic in Rust so Android/Windows/wasm can share it: a deterministic input-sync or state-sync layer (your call, justify it in the PR) with a serializable message format, a tiny relay/room server that runs locally (e.g. a small Rust or Python WebSocket server under tools/ or server/) and a client in the bridge. Demo: examples/versus_online.py where two local processes join the same room via the local server and play the same match. Do NOT deploy anything or add paid services; document free hosting options (e.g. Cloudflare Workers/Durable Objects, Render free tier) in docs only.
Tests: unit tests for the sync/message layer, and a verify or pytest that runs two headless clients against the local server for N ticks and checks both states match.

Other agents work in parallel (FPS, card game): keep shared-file edits small and additive; before finishing, merge latest master into your branch (no force-push) and resolve conflicts.

Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings (default, render), pytest tests -m "not golden" and verify scenarios pass (demo_live known failure). Save a headless screenshot of the local versus match as artifact. Open a PR against master with a short Japanese summary. Do not merge.
