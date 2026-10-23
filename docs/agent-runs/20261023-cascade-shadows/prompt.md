# Prompt

KAGRA quality wave 4, item 4 (cascaded shadows), after linear color (#146), glTF PBR (#149), and the post pass graph (#150). Core stays Rust + Python. Do not edit Neon Run files. No force-push. One PR, CI green.

Original request, this item:

> Cascaded shadow maps (2–3 cascades, PCF, PCSS on the high tier). Frustum AABB culling and instancing only where they are missing.

Defaults stay the historical one-map picture. `KAGRA_QUALITY=high` or `2` opts in. `0` / `off` / `low` forces the single map.
