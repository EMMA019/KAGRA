# Prompt

KAGRA quality wave 4, item 1 (linear workflow), from the external review in the task. Core stays Rust + Python. `palette` is a test oracle only (IEC sRGB). three.js source is not copied; the Narkowicz ACES fit already in the shaders is the same public approximation three.js uses.

Original request, item 1:

> Linear workflow audit: make sure base color/emissive textures are sampled as sRGB and data maps (normal/ORM) as linear on BOTH WebGPU and WebGL2; lighting happens in linear HDR; bloom thresholds in HDR before tonemapping; ACES and the LUT are applied last; there is one output encoding (no double gamma). Add pixel tests with known reference colors.
