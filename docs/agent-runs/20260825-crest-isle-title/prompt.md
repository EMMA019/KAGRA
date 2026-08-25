Give Crest Isle a real title screen before SPACE. Repo: https://github.com/EMMA019/KAGRA. Demo: python examples/vrm_open_world.py. Start from current master (has #82 and #83).

Emma's screenshot (attached) is the view BEFORE pressing SPACE. Mode is already `title` (not SMOKE). She sees Alicia standing on a glitched split world: left dark green "sky" + tan strip, right blocky yellow/white, grey untextured floor, hard vertical seam. Title text is missing or lost in that mess. She asked to switch this to a proper title screen. Do not change the brown/green meadow tiling in play (she said leave that).

## Cause (verify)
`CrestIsle.draw` always draws sky + world + props + VRM, then if mode=="title" calls `_banner` which is `kagra.fill(0,0,SW,SH,(6,12,20), 118)` — alpha 118 overlay. The live 3D (camera at start looking at half-streamed / UV-split terrain) shows through. Result screen uses the same overlay.

## Fix
When `mode == "title"` (and similarly `result` is OK to keep a readable overlay, but title must not show the glitched world):
- Do NOT draw world / props / VRM / water behind the title. Optional: puresky/stage or a solid dark/cls only.
- Opaque title: "Crest Isle", the existing Japanese subtitle (草原・海・山を走れ / SPACE でスタート), Best score if any, Alicia credit line. SPACE/ENTER still starts (`update` already does this).
- SMOKE still starts in `play` and must not wait on the title.
- Keep play HUD as-is. No Unity editor, no Rapier, no new public API unless tiny. Don't retint terrain. Don't redo cam clamp / IBL / rehold.

GPU-free: assert title draw path does not call world.draw / Prop.draw_all / draw_vrm (source test like test_open_world.py). pytest -m "not golden". Open a PR to master. Log under docs/agent-runs/ if that's the convention.
