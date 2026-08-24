# Result — Pretty room

- `room()` / `apply_room_look` / `set_spot_light` / `set_exposure`
- 拡散 IBL: 8² irradiance キューブ。スペキュラは鋭い HDRI キューブ
- Demo: `examples/vrm_pretty_room.py`
- Verify scenario: `examples/verify_scenarios/pretty_room_smoke.json`（この VM では GPU 未ビルド）
- Prop Garden スモークは未変更
- まだ無い: スポット影、フル PMREM LOD、複数ライト、CSM
