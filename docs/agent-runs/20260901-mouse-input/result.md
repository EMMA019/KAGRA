# Result — 入力拡張（マウス）スライス

## 成果物

- `kagra/gameloop.py` — `mouse_pos()` / `mouse_down(button)` /
  `mouse_clicked(button)`（1=左 / 2=中 / 3=右）。tkinter 窓に Motion /
  Button / ButtonRelease をバインド。クリック瞬間はフレーム開始でクリア。
- `tests/test_gameloop.py` — マウスハンドラのテスト追加（5 件）。

## verify

- pytest tests/test_gameloop.py パス。
- ヘッドレス（--headless）ではマウスは (0,0)・無押下のまま（影響なし）。

## 使い方

```python
from kagra.gameloop import mouse_clicked, mouse_pos

x, y = mouse_pos()
if mouse_clicked(1):        # 左クリックされた瞬間
    # 選択肢の矩形と照合して UI を進める（ゲームロジック）
```

## 次の山

⑤ 1 本目ジャンル（バニーガーデン系）を Python 1 本で実演。
共通コア（文字・UI・音・マウス）が揃ったので、VRM キャラ + 会話 +
好感度 + 日程 + セーブを `Scene.update` だけで組める。
