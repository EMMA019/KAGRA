import kagra, os, sys

class DebugScene(kagra.Scene):
    def on_enter(self):
        fbx_src = "assets/Flair.fbx"
        raw = kagra._engine.load_fbx_anim(fbx_src)
        clip_name, frame_time, frames = raw[0]
        print(f"フレーム数: {len(frames)}  fps: {1/frame_time:.0f}")

        hips_ys, hips_xs, hips_zs = [], [], []
        for frame in frames:
            for (bname, tx, ty, tz, qx, qy, qz, qw, has_trans) in frame:
                if 'Hips' in bname:
                    hips_ys.append(ty); hips_xs.append(tx); hips_zs.append(tz)
                    break

        print(f"X: min={min(hips_xs):.4f}  max={max(hips_xs):.4f}  range={max(hips_xs)-min(hips_xs):.4f}m")
        print(f"Y: min={min(hips_ys):.4f}  max={max(hips_ys):.4f}  range={max(hips_ys)-min(hips_ys):.4f}m")
        print(f"Z: min={min(hips_zs):.4f}  max={max(hips_zs):.4f}  range={max(hips_zs)-min(hips_zs):.4f}m")
        print(f"frame[0]: Y={hips_ys[0]:.4f}m")

        base = hips_ys[0]
        significant = [(i, y) for i, y in enumerate(hips_ys) if abs(y - base) > 0.1]
        if significant:
            print(f"\nY が 0.1m 以上変化するフレーム（{len(significant)}件）:")
            for i, y in significant[:10]:
                print(f"  frame[{i:3d}]: Y={y:.4f}m  delta={y-base:+.4f}m")
        else:
            print("\n→ Y はほぼ変化していません（0.1m 未満）")

        for (bname, tx, ty, tz, qx, qy, qz, qw, has_trans) in frames[0]:
            if 'Hips' in bname:
                print(f"\nHips has_trans={has_trans}")
                break

        raise SystemExit

    def update(self, dt): pass
    def draw(self): kagra.cls(0, 0, 0)

kagra.init(320, 240, "debug", 60)
kagra.run(start_scene=DebugScene())
