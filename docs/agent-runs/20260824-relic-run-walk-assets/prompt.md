Two related jobs on EMMA019/KAGRA master (after #71 sticky-walk merge). Public APIs only. Follow AGENTS.md. Open ONE PR.

1) Bug: walking with arms stretched forward (Emma screenshot of Relic Run). While walking, both arms are bent at the elbows and held together in front of the midsection. Investigate avatar locomotion (missing walk clip → bind pose; Mixamo retarget; ActionController arm layers; leftover carry/IK; look-at). Fix walk arm swing + relaxed idle. If walk asset missing, fail loudly or ship a licensed clip. Do not leave except: pass hiding a failed load.

2) Quality: gather FREE licensed assets (CC0 Kenney / Poly Haven / Quaternius) and make Relic Run look like a real 30s game. Document source URL + license. Do not vendor into the pip wheel. Keep under examples/assets/relic_run/. Visual bar: grass/dirt ground, glTF trees/rocks in spawn frustum, glowing relics, HDRI/sky not purple void. Keep gameplay (5 relics, 30s, score, Walk.face, third person).

Verify: pytest tests -m "not golden"; update relic_run tests/smoke if paths change; README sample line stays; agent log under docs/agent-runs/.
