# Session — indoor umbra + parent 4 + normal pairwise

- 0.614 は影がゼロではなく、真上ランプ + mix(0.50) + 小さい接触影で画面差が足りない。
- スポット所有時のウンブラを 0.16 に落とす（平行光の 0.50 は維持。埋めは太陽）。
- golden は横ランプ、床は半辺 13（辺 26 > skip 24）でキャスタにしない、カメラは影の落ちる床を見る。
- Pretty Room のランプも少しオフセット。Garden `KAGRA_SMOKE` は `apply_room_look` を呼ばない。
- 親子 4 段は `PARENT_MAX_LEVELS`。剛体と局所ライト複数はこのスライスに入れない。
- 法線は `normal_bump` / `normal_flat` ペア。這いの画素は入れない（カメラが動くと全体が動く）。
  這いは近段スナップが 0.2 texel の視点移動で同じ、を単体で足した。
- GPU はこの VM に無い。閉じるのは CI golden。
