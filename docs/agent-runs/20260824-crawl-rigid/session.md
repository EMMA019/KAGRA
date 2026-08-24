# Session — crawl pixels + rigid boxes

- `origin/master` は #63（画素チェック閉じ）込み。ブランチ `cursor/crawl-rigid-6817`。
- 這い: カメラが動くと色バッファ全体が動くので、絶対 golden ではなく pairwise。
  - CI #64 / #65: `outdoor_crawl` vs `_off` が **mean_abs=0.000**
  - 原因は画角だけではなかった。2 段の write が **同じ VP uniform** を
    `Queue::write_buffer` で上書きし、両レイヤが最後の VP になった（bloom と同じ）
  - 修正: レイヤごとに VP バッファ。撮り直し（オービット + 横日）も残す
  - differ 閾値 2.0（平行光ウンブラは mix 0.50）
  - オン/オフで影が画素に出る（`outdoor_crawl` vs `_off`）
  - 同じスナップ格子の 0.2 texel 視点（オービット注視点を平行移動）は似ている
  - 床は skip サイズ（受けだけ）。角の柱で近段 half=12
  - `apply_outdoor_look` は呼ばない（Garden スモークのトーンマップを汚さない）
  - CPU: `outdoor_crawl_golden_eyes_share_near_snap`
  - 画素の閉じは Windows CI。この VM に GPU wheel は無い
- 剛体: Rapier クレートは 5MB wheel のため入れない。公開面は変えない。
  - 落ちる・積むは既にあった
  - 乗る: カプセルは箱を沈めない。寝た箱はキャラでは起こさない
  - テスト: `test_capsule_stands_on_dynamic_box` / `test_player_stands_on_dynamic_stack`
- チェック: 剛体は閉じてよい。這いは CI 待ち。80% とは呼ばない
