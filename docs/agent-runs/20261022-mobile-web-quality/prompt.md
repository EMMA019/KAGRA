# Prompt

Make KAGRA games look and feel 「おっ」 on a phone web browser, closer to Unity mobile. Target the wasm build first. Android is parked.

Audit the renderer against Unity URP mobile and three.js. Write `docs/quality_gap.md`. Implement the highest-impact WebGL2-safe items: post stack (HDR + bloom + ACES + vignette + optional LUT), a Python juice module (shake, hitstop, tween/ease, squash-stretch), and a simple particle emitter. Apply them to Box Hop with before/after screenshots. Keep CI green. Split later waves (PBR maps, real IBL, softer shadows, skinning quality) into follow-up PRs.
