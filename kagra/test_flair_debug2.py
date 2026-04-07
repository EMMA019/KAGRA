import kagra

class DebugScene(kagra.Scene):
    def on_enter(self):
        raw = kagra._engine.load_fbx_anim("assets/Flair.fbx")
        clip_name, frame_time, frames = raw[0]
        print(f"フレーム数: {len(frames)}, ボーン数: {len(frames[0])}")

        # 全ボーンの translation 変化量を確認
        print("\n=== 位置が変化するボーン (range > 0.001m) ===")
        bone_names = [b[0] for b in frames[0]]
        for bi, bname in enumerate(bone_names):
            xs = [frames[fi][bi][1] for fi in range(len(frames))]
            ys = [frames[fi][bi][2] for fi in range(len(frames))]
            zs = [frames[fi][bi][3] for fi in range(len(frames))]
            xr = max(xs)-min(xs)
            yr = max(ys)-min(ys)
            zr = max(zs)-min(zs)
            if xr > 0.001 or yr > 0.001 or zr > 0.001:
                print(f"  {bname}: X_range={xr:.4f} Y_range={yr:.4f} Z_range={zr:.4f}")

        # Root ボーンの詳細
        print("\n=== Root ボーンの詳細 ===")
        for bi, bname in enumerate(bone_names):
            if bname in ('Root', 'root', 'Armature') or bi < 3:
                b0 = frames[0][bi]
                b30 = frames[min(30, len(frames)-1)][bi]
                print(f"  [{bi}] {bname}")
                print(f"    frame[0]:  tx={b0[1]:.4f} ty={b0[2]:.4f} tz={b0[3]:.4f}")
                print(f"    frame[30]: tx={b30[1]:.4f} ty={b30[2]:.4f} tz={b30[3]:.4f}")
                print(f"    has_trans={b0[8]}")

        raise SystemExit

    def update(self, dt): pass
    def draw(self): kagra.cls(0,0,0)

kagra.init(320, 240, "debug2", 60)
kagra.run(start_scene=DebugScene())
