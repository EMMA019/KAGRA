KAGRA (Rust kagra-shared + Python). Read AGENTS.md, genre_session.rs, games/*.json and docs/distribution.md on master first.

Goal (Phase 3): make genre examples feel like complete games, for Windows and Android.
1. Audio: a cross-platform audio module (e.g. kira or rodio on desktop, working on Android) exposed to Python as kagra.audio: BGM loop with fade, SFX one-shots, volume groups, mute. Use only CC0 sounds (or tiny generated ones) under assets/audio with a license note. Hook SFX into 2-3 existing genres (platformer jump, fps fire, card play).
2. Game flow: a reusable scene/flow layer (title -> play -> pause -> result -> retry/title) plus settings (volume) and save data (high score/progress) in Rust, JSON in a platform-appropriate path (Windows AppData, Android internal storage). Touch + keyboard. HUD text ASCII or Japanese with no missing glyphs.
3. Apply it to the default packaged game (games/*.json entry) so the Windows zip/APK ships with title screen and save.
Headless: audio must no-op safely without a device (CI, lavapipe).
Other agents may work in parallel; keep shared edits additive and merge latest master before finishing (no force-push).
Done when cargo test -p kagra-shared --lib --locked, clippy -D warnings, pytest tests -m "not golden" (known failure test_mainline_import_is_not_old_engine OK) and verify scenarios (demo_live known) pass, and Package Windows/Android workflows still pass. Save lavapipe screenshots of title and pause screens as artifacts. Open a PR against master with a short Japanese summary. Do not merge.
