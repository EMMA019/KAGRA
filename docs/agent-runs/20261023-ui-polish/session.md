# UI polish

`nine_slice` and `styled_button` live in `kagra.ui2d`. They emit HUD quads and one text. No genre, no renderer change. A pressed button shifts down and drops the shadow.

HUD text still uses ab_glyph coverage quads (no new shader, wasm-safe). The raster is 2× the requested pixel size and each sample is a 0.5px quad, so edges are finer than the old 1px stamps.
