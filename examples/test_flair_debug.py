"""
Flair.fbx の Hips 移動データを詳しく確認するデバッグスクリプト。
assets/ に Flair.fbx を置いて実行してください。
"""
import kagra, os, sys

# FBX を assets にコピーするか確認
fbx_src = "assets/Flair.fbx"
if not os.path.exists(fbx_src):
    print(f"ERROR: {fbx_src} が見つかりません")
    sys.exit(1)

print("=== Flair.fbx Hips 移動データ解析 ===\n")
raw = kagra._engine.load_fbx_anim(fbx_src)
if not raw:
    print("ERROR: FBX にアニメーションが見つかりません")
    sys.exit(1)

clip_name, frame_time, frames = raw[0]
print(f"クリップ名: {clip_name}")
print(f"フレーム数: {len(frames)}  fps: {1/frame_time:.0f}")
print(f"尺: {len(frames)*frame_time:.1f}秒\n")

# 全ボーン名を確認
if frames:
    bone_names = [b[0] for b in frames[0]]
    hips_bones = [n for n in bone_names if 'Hip' in n or 'hip' in n]
    print(f"Hips 系ボーン: {hips_bones}")
    print(f"全ボーン数: {len(bone_names)}\n")

# Hips の XYZ 位置変化を追跡
hips_data = {'x': [], 'y': [], 'z': []}
for frame in frames:
    for (bname, tx, ty, tz, qx, qy, qz, qw, has_trans) in frame:
        if 'Hips' in bname:
            hips_data['x'].append(tx)
            hips_data['y'].append(ty)
            hips_data['z'].append(tz)
            break

if hips_data['y']:
    ys = hips_data['y']
    xs = hips_data['x']
    zs = hips_data['z']
    print(f"=== Hips 位置範囲 ===")
    print(f"X: min={min(xs):.4f}  max={max(xs):.4f}  range={max(xs)-min(xs):.4f}m")
    print(f"Y: min={min(ys):.4f}  max={max(ys):.4f}  range={max(ys)-min(ys):.4f}m")
    print(f"Z: min={min(zs):.4f}  max={max(zs):.4f}  range={max(zs)-min(zs):.4f}m")
    print(f"\nframe[0] 基準: Y={ys[0]:.4f}m")
    
    print(f"\n=== Y が大きく変化するフレーム（基準から0.1m以上） ===")
    base = ys[0]
    shown = 0
    for i, y in enumerate(ys):
        if abs(y - base) > 0.1:
            print(f"  frame[{i:3d}]: Y={y:.4f}m  delta={y-base:+.4f}m  ({(i*frame_time):.1f}秒)")
            shown += 1
            if shown >= 20:
                print(f"  ... (残り {sum(1 for yy in ys if abs(yy-base)>0.1) - shown} フレーム)")
                break
    
    if shown == 0:
        print("  → Y はほぼ変化していません！")
        print("  → Hips の上下動がアニメーションに含まれていない可能性があります")
    
    print(f"\n=== has_trans の確認 ===")
    for (bname, tx, ty, tz, qx, qy, qz, qw, has_trans) in frames[0]:
        if 'Hips' in bname:
            print(f"Hips has_trans = {has_trans}")
            print(f"frame[0]: tx={tx:.4f} ty={ty:.4f} tz={tz:.4f}")
            break
else:
    print("ERROR: Hips ボーンが見つかりません")

