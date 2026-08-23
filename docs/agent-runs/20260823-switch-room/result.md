# Result — Switch Room

| 成果物 | 内容 |
|---|---|
| `examples/vrm_switch_room.py` | ゲーム本体（公開 API のみ） |
| `examples/switch_room_rules.py` | スイッチ判定 / 箱配置（GPU 不要） |
| `examples/verify_scenarios/switch_room_smoke.json` | ヘッドレス検証 |
| `kagra/world3d.py` | 床 + 箱 + カプセル |
| `Camera3D.follow` | ワールド追従 |
| `upload_mesh_3d` / `draw_mesh_id` | メッシュ保持 |

## Verify

- `pytest tests -m "not golden"` — ルール / World3D / Camera follow / API 索引
- `python -m kagra.verify examples/verify_scenarios/switch_room_smoke.json`
  — この環境では `kagra_core` 未ビルドのため未実行（シナリオは置いた）

往復: 実装 1 本。Heart Catch と同じ「ロジックは pytest、GPU は JSON」の閉じ方。
