# Your own VRM

English below. 日本語は後半。

```bash
pip install kagra
python -m kagra --vrm me.vrm
python -m kagra --vrm me.vrm --song my.wav --dance wave.vrma
```

1. Export a **VRM 0.x or 1.0** from [VRoid Studio](https://vroid.com/studio) (File → Export as VRM).
2. Put `me.vrm` anywhere. An absolute path is fine.
3. Optional: a WAV for `sing()` (any TTS, including [Irodori-TTS](https://github.com/Aratako/Irodori-TTS) — not bundled), a `.vrma` / `.fbx` / `.bvh` for `dance()`.
4. Credit the character if the license asks. Alicia Solid (the first-run sample) is © Dwango.

Python, same thing:

```python
av = kagra.avatar("me.vrm")
av.dance("wave.vrma")
av.sing("my.wav")
```

---

# 自分の VRM

```bash
pip install kagra
python -m kagra --vrm me.vrm
python -m kagra --vrm me.vrm --song my.wav --dance wave.vrma
```

1. [VRoid Studio](https://vroid.com/studio) から **VRM 0.x / 1.0** を書き出す（ファイル → VRM 書き出し）。
2. `me.vrm` はどこでもよい。絶対パスで渡せる。
3. 歌うなら WAV（[Irodori-TTS](https://github.com/Aratako/Irodori-TTS) など。エンジン非同梱）、踊るなら `.vrma` / `.fbx` / `.bvh`。
4. モデル規約どおりクレジットを。初回サンプルの Alicia Solid は © Dwango。
