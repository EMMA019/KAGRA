# Prompt

ロードマップのフェーズ1「接着 API 4本」を KAGRA 共有ランタイム（kagra-shared
WorldDoc / WorldPlay）に載せてください。ジャンル専用コードはエンジンに逆流
させません。4本は:

1. **interact**（調べる/話す/使う）— 水辺で J → 竿を振る、NPC に話しかける
2. **event**（出来事→複数システム）— FishCaught → 得点/UI/セーブ
3. **timer**（待つ）— Cast → 5〜20 秒 → Bite、料理の待ち
4. **state→animation** — idle / cast / reel / hit の状態→アニメ切替

方針:
- dump JSON スキーマ + 純ロジックで表現する（WorldDoc に載せる）
- 「棚の on / after を本線の WorldDoc に載せる」が近い。enemy.chase みたいに
  invent しない。API_INDEX に無い名前は作らない
- 既存の play_world / Crest / emma walker を壊さない
- cargo test をパスさせ、docs/schemas/world.json と docs/agent-runs/ のログを
  更新する
