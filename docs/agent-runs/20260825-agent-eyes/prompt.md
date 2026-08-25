# Prompt

Emma (KAGRA owner) wants the NEXT slice — NOT a Unity editor. Strategy stays: no Tk/Inspector, no 4-cascade CSM, SSAO, volumetrics, WebXR, desktop-pet headpat. Give Cursor/Claude the same EYES she has when she pastes screenshots.

Start from current master of https://github.com/EMMA019/KAGRA (Crest Isle / Relic Run / sticky-walk already merged or in flight — use latest master).

## Ship in ONE PR (public APIs, AGENTS.md, gen_api_index, pytest -m "not golden")

### 1. Click annotations (extend pick)
There is already pick()/screen→bone. Extend so a preview click saves a JSON note: screen xy, world xyz if available, bone name if any, Prop id if any, timestamp, optional screenshot path. API like `kagra.annotate()` or extend existing pick. Persist to scratch/ or a small JSONL. Document for agents: this is how "ここもう少し" becomes numbers.

### 2. kagra.debug_trace()
Per-frame telemetry for physics feel bugs that LOOK like they need video:
- player foot Y vs terrain height (ground_y), delta
- optionally vx/vz, on_ground flag, camera distance
Emit only frames over a threshold (e.g. |foot-terrain| > 0.05 while supposedly grounded) plus a compact summary ("frames 32-48 floated 0.15").
JSONL for agents. GPU-free unit tests with fake height fn.
This is the slope-float detector — do NOT add Rapier in this PR.

### 3. Camera3D.follow wall clip
Simple ray/segment from player to camera vs World3D boxes/static triangles (whatever collision already exists). If hit, pull distance in so the camera doesn't go through walls at map corners. Test with a boxed room (Switch/Dodge style bounds).

### 4. Prop/terrain toon
In kagra-core shaders.rs, Prop/terrain fs_main should use the same cam.toon stepped lighting as VRM. Character vs environment currently use different light math. Pairwise/golden if the repo already has that pattern; don't invent a huge post stack.

## Out of this PR
Cloth/spring sleeves, locomotion blend trees, spatial audio, multi-avatar perf study, Rapier crate in the wheel.

## Docs
Short note in docs/ROADMAP.ja.md or REVIEW: "agent eyes = annotate + debug_trace; not a visual editor". README API index regen.
Agent log under docs/agent-runs/.

Open a PR with how to verify each of the four.
