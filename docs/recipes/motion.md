# Dance clips (VRMA / FBX / BVH)

English below. 日本語は後半。

KAGRA does not vendor a motion generator. Drop `.fbx` / `.vrma` in `assets/`
(or `assets/anim`, `assets/motion`). `python -m kagra` plays every file it
finds. `--dance path` pins one clip. SPACE / N skips.

```bash
python -m kagra
python -m kagra --vrm me.vrm --dance wave.vrma
```

| Source | How |
|---|---|
| [text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) | Generate a `.vrma`. Fingers, expressions, LookAt play as-is. |
| Mixamo `.fbx` | `av.dance("samba.fbx")`. Finger bones retarget to VRoid `J_Bip_*`. |
| BVH | `av.dance("clip.bvh")`. No fingers → `relax_hands()` curls them. |
| Bundled | `av.dance()` with no args (synthetic BVH in the wheel). |

```python
av.dance("wave.vrma")
```

---

# ダンスクリップ（VRMA / FBX / BVH）

モーション生成はエンジンに入れない。`.fbx` / `.vrma` を `assets/` に落とすと
`python -m kagra` が全部再生する。`--dance path` で 1 本固定。SPACE / N で次。

```bash
python -m kagra
python -m kagra --vrm me.vrm --dance wave.vrma
```

[text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) の `.vrma` はそのまま（指・表情・LookAt）。Mixamo の FBX は VRoid 指にリターゲット。BVH に指が無いときは `relax_hands()` が曲げる。
