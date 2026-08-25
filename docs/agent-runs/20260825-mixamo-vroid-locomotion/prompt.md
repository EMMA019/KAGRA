# prompt

Make Mixamo FBX locomotion actually work on VRoid VRM characters without folded-forward arms. Emma's characters will all be VRoid (`J_Bip_*`). Do not commit Mixamo FBX binaries. Open a PR against master; Emma merges herself.

Goal / done when:
- Mixamo clips retarget onto VRoid humanoid with rest-pose AND bone-roll compensation so arms hang/swing instead of folding forward like a carry pose.
- Works for both T-pose VRoid (Emma.vrm) and A-pose VRoid (Alicia). Same pipeline: Mixamo rest is T-pose; VRoid rest varies.
- Crest Isle / Relic Run can use local Mixamo Idle/Walk/Run if present, else keep built-in clips. Do not resolve the `walk` alias to `tests/fixtures/synthetic_walk.bvh` for play.
- Plug into existing `avatar.set_locomotion` blend (idle/walk/run) rather than `dance()` replacing the whole body. Keep upper-body overlay (clap/banzai) working.
- GPU-free tests prove: T-pose Mixamo on T-pose VRoid does not leave upper-arm ~90° forward of rest; A-pose VRoid also does not fold. Add a fixture small enough for git.
- Existing tests pass. No Rapier, no visual editor, no CSM/SSAO/WebXR, no terrain retune, no large binaries.
