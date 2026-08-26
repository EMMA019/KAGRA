# Prompt

Add a game-ready Character Controller to KAGRA so Crest Isle (and other genres) stop feeling floaty/cheap on slopes, steps, accel, and collision. Emma now accepts the pip wheel growing a bit past +5MB IF it earns this. Rapier is allowed only if you verify it is the smallest way to get a solid capsule controller; if AABB snap-to-plane can be made good enough, prefer that and do NOT add Rapier.

Repo: https://github.com/EMMA019/KAGRA. Start from current master. Desktop: `python examples/vrm_open_world.py`.

Another PR is IN FLIGHT for black trees / camera peel / zoom / Kenney density (`cursor` agent leftover-black-trees). Keep THIS PR merge-friendly: put the controller in engine modules (`kagra` / `kagra-core`). Touch `examples/vrm_open_world.py` only as a thin swap so Crest Isle uses the controller for walk/slope/jump. Do not rewrite stream tiles, props, fog, camera zoom, or Mixamo bind.

Done when:
- Capsule/AABB character: walk, accelerate/decelerate, stand on slopes without floating or sliding forever, small steps, optional jump + land, collide with static props. Sticky-walk Windows input (quiet gap 3) stays.
- High-level API an agent can call (search API_INDEX; do not invent blindly), e.g. move/wish/jump on Walk or a new controller type. Document in API index.
- Crest Isle uses it. GPU-free tests for slope stand, step-up, accel. Existing tests pass.
- Open a PR, do not merge. Report size impact if Rapier is added.

Out of scope: NavMesh, SSAO, CSM, volumetric, Unity editor, networking, quests, inventory. Keep Mixamo retarget, spatial audio, blob AO.

Investigate current Walk / foot AABB / snap-to-plane first. Hypothesis (non-binding): slope float is still the old tight AABB + one-sided height sample, not missing Rapier. Verify.
