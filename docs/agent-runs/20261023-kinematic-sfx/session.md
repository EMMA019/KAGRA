# Kinematic character controller and procedural SFX

`PhysicsWorld::move_character` uses Rapier's `KinematicCharacterController` on walker capsules and kinematic props. The caller passes this tick's translation, including gravity. The controller slides, snaps to the ground, and autosteps obstacles up to 0.35 m. Static bodies return None. WorldPlay's sticky walk is unchanged.

`kagra.rigid.PhysicsWorld.move_character` is the Python wrapper. Tests skip when the extension was built without `physics`.

`sfx::synth_wav` writes 16-bit mono WAV presets (`jump`, `hit`/`pickup`, `coin`, `ok`, `hurt`, `explode`). Bundled clips still win in `mixer::sfx_wav`. Wasm exports `synthWav`. No sfxr source is copied. No fundsp crate.
