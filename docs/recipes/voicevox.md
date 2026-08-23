# VOICEVOX で喋らせる

KAGRA は VOICEVOX を同梱しない。無料のエンジンを別途起動し、HTTP でつなぐ。

1. [VOICEVOX](https://voicevox.hiroshiba.jp/) を入れて起動する（デフォルト `http://localhost:50021`）
2. `pip install kagra`
3. レンダラ準備後に喋らせる

```python
import kagra
from kagra.camera3d import Camera3D

kagra.init()
cam = Camera3D(); cam.use_orbit(radius=2.6, target=(0, 0.9, 0))
av = None

def ready():
    global av
    av = kagra.avatar(str(kagra.ensure_vrm()))
    av.enable_lipsync()
    av.speak_voicevox("こんにちは。今日も歌います。")

def update(dt):
    av.update(dt)
    cam.update(kagra.get_engine())

def draw():
    kagra.cls(16, 12, 32)
    kagra.draw_vrm(av.vrm_id)

kagra.run(update, draw, on_ready=ready)
```

`audio_query` の mora 長が口に乗る。エンジンが落ちているときは `VoicevoxError`。
COEIROINK は `url="http://localhost:50031"`。歌の WAV はこれまで通り `av.sing("voice.wav")`。
