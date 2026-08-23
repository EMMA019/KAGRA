# OBS / virtual camera (0.1.3)

YouTube / Twitch APIs are not in the engine. Chat is JSONL. Output is a virtual camera or window capture.

日本語の短い手順は下と同じです。

# OBS に載せる（0.1.3）

YouTube / Twitch API はコアに無い。チャットは JSONL。送出は仮想カメラか窓キャプチャ。

## いちばん短い経路

```bash
pip install "kagra[stream]"
python -m kagra --loop --stream
```

OBS の映像ソース:

1. **Video Capture Device** / **OBS Virtual Camera**（`kagra[stream]` が成功したとき）
2. だめなら **Game Capture** で KAGRA の窓

720p（デフォルト 1280×720）を推奨。GPU readback は毎フレーム。

## チャット（API キーなし）

`--stream` は `kagra-chat.jsonl` を尾から読む。外部スクリプトが YouTube / Twitch を書いてよい。

```bash
echo '{"user":"alice","text":"こんにちは"}' >> kagra-chat.jsonl
```

自分のパスは `--chat inbox.jsonl`。

## マイク口パク

```bash
pip install "kagra[mic]"
```

```python
from kagra.mic import MicLipsync
mic = MicLipsync().start()
# update() の中
mic.apply(av)
```

## まだ無いもの

NDI / RTMP 直送、YouTube Live Chat API、無人セーフティ。窓キャプチャと JSONL で足りるところまでが 0.1.3。
