Neon Cat PR C (requested by Emma, in Japanese; paraphrased from the brief):

1. A Stray-like atmosphere on the neon base look: no daytime colours, no city backdrop. Rain particles, neon signs and a procedural concrete wall texture from `tools/gen_assets`. Original assets only (no Quaternius); CC0 props only from the ledger. All tunable and switchable in JSON, with a low-quality fallback.
2. Web audio for BGM and SFX from JSON-named files (mp3/ogg/wav), working on browsers and phones with an unlock on first touch. Generated placeholder sounds until Emma supplies her own. Map knock, coin, perch, robot_on, robot_off, jump, hit, item, meow, gameover. Volumes in JSON.
3. Three per-run missions (collect N coins, pass M gates, perch once) and a high-score display. Keep the #181 ranking.

Rules: keep engine defaults and measurement baselines; add tests; update the asset ledger and CREDITS; fmt, clippy, pytest and CI green; say what is unverified.
