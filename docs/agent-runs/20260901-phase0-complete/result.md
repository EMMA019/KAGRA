# Result — Phase 0 完了（新旧レンダラー差分を埋める）

汎用エンジン化ロードマップ Phase 0 の全 8 項目が完了した。

| # | 項目 | 対応 | コミット |
|---|---|---|---|
| ① | spring→布の適用 + 袖ヘルパー | step_with_updates（回転デルタ）/ wind / sample_skinned_cloth / sleeve.rs（ensure_sleeve_cloth 移植） | 7127af2, b7c91a2, 022d015 |
| ② | 空間音響 | set_listener / play_se(x,y,z)（距離減衰 + ステレオパン、spatial.py 移植） | bd663e7 |
| ③ | TTS 音素タイミング | kagra/tts.py（VOICEVOX mora → walker.expression リップ）+ bunny 配線 | e538fc5 |
| ④ | VRMC_node_constraint | 既に適用済みと確認（apply_constraints が配線済み） | — |
| ⑤ | firstPerson 適用 | part_hidden_in_first_person（ThirdPersonOnly/Auto 非表示）+ eye カメラでパーツフィルタ | 5798bb8 |
| ⑥ | カプセル上端の坂浮き | slope_ground（足元リング最大高で接地） | dc3a686 |
| ⑦ | master マージ状況 | 解決（master == origin/master、README 実態一致） | — |
| ⑧ | VRM Crest Isle 統合 | crest_emma_world.json + collectathon_emma_smoke（タイトル→プレイ→結果を Emma が歩く） | 9019685 |

## verify

- lib 393 + offscreen 12 パス、clippy -D warnings クリーン、wasm32 OK
- pytest 全パス（tts 3 件追加）
- 実ゲーム: 布が風で揺れる（固定 clip でもフレーム間 869+ 画素変化）、
  Emma が Crest collectathon を歩く、TTS リップが口の形を動かす

## 次の山

Phase 2 — アニメーションブレンド（パラメータ駆動の連続ブレンド +
上半身/下半身レイヤー分離）。バニーガーデンの「話しながら手のジェスチャー」、
トルネコの「攻撃モーションが移動と合成される」に直結。
