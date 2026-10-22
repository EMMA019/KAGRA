KAGRA (Rust kagra-shared + Python, GenreSession with dump/from_json, browser/Android/Windows builds). Read AGENTS.md first.

Goal: while playing on a phone (browser preview or Android) or Windows, Emma can say "直してここ" in-game and an AI agent can reproduce it exactly.

1. A shared debug overlay (debug builds/preview only): a hidden gesture (3-finger long-press on touch, F12 on keyboard) pauses and opens a small panel: capture screenshot, GenreSession dump() + recent input log (last ~10 s) + build/version/device info, and a free-text note (Japanese input must work).
2. Sending without embedding any token in the app: produce a GitHub "new issue" URL (github.com/EMMA019/KAGRA/issues/new with template + prefilled title/body including note, versions, and the state as a compact JSON or a short code). Screenshot: save to gallery/downloads and/or copy to clipboard with on-screen instruction to paste it into the issue. If the state is too large for a URL, use a free paste-able compressed+base64 block. Add an issue template label "playtest".
3. Repro tooling: scripts/repro_issue.py <issue number or pasted block> rebuilds the session from the dump, replays inputs headlessly, and saves a screenshot, so agents can verify before/after. Document in AGENTS.md: agents fixing playtest issues must repro first, then attach before/after screenshots.

No paid services, no secrets. Other agents work in parallel (web preview, Android 16KB fix): keep shared edits additive and merge latest master before finishing (no force-push). Done when full CI + Package workflows are green; include lavapipe screenshots of the overlay. Open a PR with a short Japanese summary. Do not merge.
