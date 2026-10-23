# Shared juice

`kagra_shared::juice` is not a Neon type. `ease` delegates to `simple-easing` 1.1.1. `Juice::tick` returns the sim dt and keeps shake, squash, and scalar tweens moving during hit-stop. Names match `kagra.juice` (`out_cubic`, `hitstop`, `shake`, `squash`). The Python module stays extension-free.

Neon Run's crash sets a 0.12 s hit-stop and a shake, and calls `flow.finish` only after that clock ends. Near-miss shake is the existing scalar. The chase eye adds `camera_offset`.
