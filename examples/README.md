# examples/

本線 — Python ゲームマスター + `WorldDoc` / `WorldPlay`（shared wgpu 30）。

```bash
python -m kagra.play_world                # Crest collectathon
python examples/python_game_minimal.py    # 接着 API（岸で J → cast）
python examples/bunny_garden_minimal.py   # 会話 / 好感度
python examples/torneko_minimal.py --seed 12345
python examples/bar_sim_minimal.py        # Bar 経営シム
python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json
```

旧 VRM / RendererV2 デモは [`old/examples/`](../old/examples/) にあります。
`import kagra` は kagra_core を必要としません。
