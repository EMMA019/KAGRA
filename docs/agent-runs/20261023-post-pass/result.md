# Result

Post-process pass graph on `cursor/post-pass-2c16`. Not a genre. No Neon Run files.

- PR: opened against `master` after this log.
- GPU: SSAO contact darkens, DOF blurs the far box more than the focus plane, motion is quiet until the camera moves, TAA keeps a flat center, clear color stays the reference.
- `KAGRA_QUALITY=0` keeps the phone picture on bloom + FXAA.
- Skipped in this PR: cascaded shadows, profiler, hot reload, KTX2, meshopt. Those are later PRs.
- three.js MIT is credited in `NOTICE` and `post.wgsl`. No three.js source is copied.
