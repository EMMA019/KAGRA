# Prompt

Slope grounding next, after Crest Isle launch crashes. Rapier is NOT in this PR (+5MB wheel still undecided). No cloth, blend trees, spatial audio, multi-avatar, visual editor, CSM/SSAO/WebXR.

Emma: "斜面接地 — first tighten AABB and measure with telemetry. If still not enough, Rapier is a separate decision." She already has `kagra.debug_trace()` on master (PR #76): per-frame foot Y vs terrain height, emit when |delta| > 0.05 while supposedly grounded, plus `debug_trace_summary()` like "frames 32-48 floated 0.15".

Repo: https://github.com/EMMA019/KAGRA latest master (Crest Isle, agent-eyes, #77 _chunk_props). PR #78 (kagra.stage unshadow) may land in parallel — do not redo it. If you touch kagra/__init__.py keep stage callable.

## Goal
Players walking hills (Crest Isle / Relic Run / Overworld) should not float. AABB grounding is coarse. Tighten it. Wire debug_trace so an agent can see float without watching video.

Hypothesis (verify, discard if wrong): character AABB vs heightfield uses a single center sample or a fat box so feet sit above the slope. Possible levers already in-tree: smaller foot AABB, extra height samples at feet, snap-to-plane, existing slope-slide. Do NOT add the Rapier crate.

## Done when
- GPU-free tests: walking a known slope, |foot_y - terrain| stays under a documented threshold while on_ground.
- debug_trace records the float if you temporarily loosen the threshold.
- pytest -m "not golden".
- PR + agent log. Document: still no Rapier; measure first.

Keep it a small physics PR, not a new engine.
