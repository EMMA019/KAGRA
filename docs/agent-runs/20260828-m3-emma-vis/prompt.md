# Prompt

Work on Emma's Windows PC. Do NOT clone. Repo D:\program\kagra (EMMA019/KAGRA).

origin/master is now 4c9c00d (LOD/instancing just landed). Emma tried:
  python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
She got a teal/tan CAPSULE floating in a solid blue void with a diamond PCF blob. Window title said Crest Isle. Eight gray HUD squares. That is NOT assets/Emma.vrm.

Dump on master (verify):
- heightfield null, props [], water_y null → void
- player.model capsule AND gltf assets/Emma.vrm → capsule wins
- window.exe launched from target/debug/examples so relative assets/Emma.vrm may miss repo-root D:\program\kagra\assets\Emma.vrm (16MB, gitignored, ON DISK)

Close THIS slice:
1. Visible ground (small heightfield or floor plane). Not a blue void.
2. When walker.gltf is set, compile_scene MUST draw the CPU-skinned VRM, not MESH_CAPSULE. model:capsule must not win.
3. Resolve assets/Emma.vrm (and resolve_alias emma) from repo root even if cwd is target/debug/examples. Same for play_world subprocess.
4. If VRM parse/load fails: dump-visible name/error, do not silent capsule. Tests may skip if file missing (CI) and still pass with tpose_humanoid.vrm fallback that is clearly a skinned mesh not a cylinder.
5. Window title from dump, not always Crest Isle. Coin HUD pips off when coins=0.
6. Keep Mixamo/MToon/albedo/spring/morph/look-at that already apply to VRM walkers.

No Bevy. No second renderer. world_play.rs tiny. Relic Run UV stay. Clippy Rust 1.98 -D warnings (as_chunks).

Tests: cargo fmt; clippy -D warnings (also --features render); cargo test -p kagra-shared --locked --offline --lib; pytest tests -m "not golden"; wasm32 wasm,render builds.
Push origin master when green, no force. Log docs/agent-runs/20260828-m3-emma-vis/

Try: python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
Emma should see a VRoid mesh on a ground, not a capsule in the sky.
