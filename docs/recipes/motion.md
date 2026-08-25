# Dance clips (VRMA / FBX / BVH)

English below. 日本語は後半。

KAGRA does not vendor a motion generator. Drop a Mixamo `.fbx` or a
`.vrma` on `dance()` / `--dance`.

```bash
python -m kagra --vrm me.vrm --dance ymca.fbx
python -m kagra --vrm me.vrm --dance wave.vrma
```

| Source | How |
|---|---|
| [text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) | Generate a `.vrma`. Fingers, expressions, LookAt play as-is. |
| Mixamo `.fbx` | `av.dance("samba.fbx")` for a full-body clip. Locomotion: put Idle/Walk/Run in `assets/mixamo/` (or `KAGRA_MIXAMO_DIR` / `D:\\program\\kagra\\assets\\mixamo\\`) and call `av.bind_locomotion()` then `av.set_locomotion(speed)`. Rest pose and bone-roll are compensated onto VRoid `J_Bip_*`. Do not `resolve_asset(..., "walk")` — that alias hits `synthetic_walk.bvh`. |
| BVH | `av.dance("clip.bvh")`. No fingers → `relax_hands()` curls them. |
| Bundled | `av.dance()` with no args (synthetic BVH in the wheel). |

```python
av.dance("wave.vrma")
```

---

# ダンスクリップ（VRMA / FBX / BVH）

モーション生成はエンジンに入れない。Mixamo の `.fbx` も `.vrma` も
`dance()` / `--dance` に渡す。

```bash
python -m kagra --vrm me.vrm --dance ymca.fbx
python -m kagra --vrm me.vrm --dance wave.vrma
```

[text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) の `.vrma` はそのまま（指・表情・LookAt）。歩きの Mixamo は `bind_locomotion()`（レストとボーンロールを VRoid に載せる。`walk` エイリアスは使わない）。`dance()` は全身。BVH に指が無いときは `relax_hands()` が曲げる。
