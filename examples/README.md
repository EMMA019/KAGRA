# examples/

本線 — Python ゲームマスター + `WorldDoc` / `WorldPlay`（shared wgpu 30）。
索引は [`docs/AGENT.md`](../docs/AGENT.md) とルート README。

```bash
python -m kagra.play_world                       # collectathon
python examples/action_minimal.py                 # 3D アリーナ
python examples/card_minimal.py                   # カード 1v1
python examples/versus_local.py                   # ローカル対戦（FightSession）
python examples/versus_online.py                  # 入力同期（docs/netplay.md）
python examples/fishcook_minimal.py               # 釣って料理
python examples/fps_minimal.py                    # FPS
python examples/novel_minimal.py                  # ノベル
python examples/platformer_minimal.py             # Box Hop（配布の既定）
python examples/puzzle_minimal.py
python examples/puzzle_pad_minimal.py
python examples/race_minimal.py
python examples/rhythm_minimal.py
python examples/rpg_minimal.py
python -m kagra.play_world kagra-shared/tests/fixtures/shop_buy_world.json
python examples/sports_minimal.py
python examples/stealth_minimal.py
python examples/survival_minimal.py
python examples/td_minimal.py                     # タワーディフェンス
# フィクスチャだけ: action_side sprite_card sim_meter fish_cast
# rhythm_beat cook_stove survival_meter box_hop
#   python -m kagra.play_world kagra-shared/tests/fixtures/<id>_world.json

python examples/python_game_minimal.py            # 接着 API（岸で J → cast）
python examples/bunny_garden_minimal.py           # 会話 / 好感度
python examples/torneko_minimal.py --seed 12345
python examples/bar_sim_minimal.py                # Bar 経営シム
python examples/flight_minimal.py                 # 島の上を飛ぶ
python examples/backyard_brawl_minimal.py         # 裏庭 3v3 旗取り
python examples/chess_minimal.py                  # スタジオ 3D チェス
python examples/physics_minimal.py                # Rapier の振り子と箱
```

音は `kagra/audio.py`、セーブ先とタイトルフローは `kagra/flow.py`。
Windows / Android の包み方は [`docs/distribution.md`](../docs/distribution.md)。

```bash
python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json
python -m kagra.verify examples/verify_scenarios/fps_smoke.json
python -m kagra.verify examples/verify_scenarios/card_smoke.json
python -m kagra.verify examples/verify_scenarios/versus_local_smoke.json
python -m kagra.verify examples/verify_scenarios/physics_smoke.json
```

旧 VRM / RendererV2 デモは [`old/examples/`](../old/examples/) にあります。
ライブデモの smoke（`demo_live_smoke`）もそちらです。本線の
`verify_scenarios/` は実在するスクリプトだけを指します。
`import kagra` は kagra_core を必要としません。
