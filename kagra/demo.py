"""同梱ライブデモ。``python -m kagra`` / ``python -m kagra.demo``。

``assets/cute_song_trial.wav`` と ``assets/Samba Dancing.fbx`` があれば
それを歌う／踊る。無ければ内蔵ソングと同梱ダンスにフォールバックする。
VRM が無ければサンプルを 1 回だけダウンロードする。

``--loop`` で曲を繰り返す（OBS でこの窓をキャプチャして配信）。
``--mascot`` で最前面の枠なし窓（デスクトップマスコット）。
``--stage venue.glb`` / ``--backdrop sky.png`` で外部会場。無ければチェッカー床。

Windows ではレンダラが ``run()`` の中でしか起きないので、
``avatar()`` / ``font()`` は ``on_ready`` で呼ぶ。
"""
from __future__ import annotations

import argparse
import math
import os
import sys

DEFAULT_DANCE = "Samba Dancing"
DEFAULT_SONG = "cute_song_trial"


def _stage_meshes():
    """床＋スポット。紫の虚空に立たせない。"""
    import math
    from kagra.vrm_stage import make_png

    def floor_px(x, y):
        stripe = (x // 16 + y // 16) % 2
        if stripe == 0:
            return (36, 28, 52, 255)
        return (28, 22, 42, 255)

    def spot_px(x, y):
        d = math.sqrt((x - 32) ** 2 + (y - 32) ** 2) / 32.0
        a = max(0, int((1.0 - d) * 160))
        return (255, 230, 180, a)

    floor_tex = make_png(128, 128, floor_px)
    spot_tex = make_png(64, 64, spot_px)
    meshes = []
    for tex, radius, y in ((floor_tex, 2.4, 0.0), (spot_tex, 0.7, 0.012)):
        segs = 32
        verts, idx = [], []
        for i in range(segs):
            a0 = math.radians(i * 360 / segs)
            a1 = math.radians((i + 1) * 360 / segs)
            base = len(verts)
            verts += [
                [0.0, y, 0.0, 0.0, 1.0, 0.0, 0.5, 0.5],
                [math.cos(a0) * radius, y, math.sin(a0) * radius, 0.0, 1.0, 0.0, 0.5 + math.cos(a0) * 0.5, 0.5 + math.sin(a0) * 0.5],
                [math.cos(a1) * radius, y, math.sin(a1) * radius, 0.0, 1.0, 0.0, 0.5 + math.cos(a1) * 0.5, 0.5 + math.sin(a1) * 0.5],
            ]
            idx += [base, base + 1, base + 2]
        meshes.append((tex, verts, idx))
    return meshes


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="python -m kagra",
        description="KAGRA live demo — a VRM avatar that sings and dances.",
    )
    p.add_argument("--vrm", default="Emma", help="VRM name, alias, or path (default: Emma)")
    p.add_argument(
        "--dance",
        default=DEFAULT_DANCE,
        help="VRMA/BVH/FBX name, alias, or path (default: Samba Dancing)",
    )
    p.add_argument(
        "--song",
        default=DEFAULT_SONG,
        help="WAV name, alias, or path (default: cute_song_trial; 'builtin' = synth)",
    )
    p.add_argument("--offline", action="store_true", help="do not download a sample VRM")
    p.add_argument("--width", type=int, default=None, help="window width (default 1280, mascot 360)")
    p.add_argument("--height", type=int, default=None, help="window height (default 720, mascot 640)")
    p.add_argument("--no-orbit", action="store_true", help="keep the camera still")
    p.add_argument("--loop", action="store_true", help="loop song + dance (OBS / unattended)")
    p.add_argument(
        "--mascot",
        action="store_true",
        help="always-on-top borderless window (desktop mascot)",
    )
    p.add_argument(
        "--stage",
        default="stage",
        help="glTF hall name/alias/path (default: stage → assets/stage.glb). 'none' = checkerboard",
    )
    p.add_argument(
        "--backdrop",
        default="",
        help="PNG/JPEG sky sphere (alias or path). Drawn behind the hall",
    )
    p.add_argument("--hidden", action="store_true", help="hide the window (agent verify)")
    p.add_argument("--max-frames", type=int, default=0, help="quit after N frames (0 = until ESC)")
    p.add_argument("--screenshot", default="", help="write a PNG at mid-run when --max-frames is set")
    return p


def _resolve_optional(kind, name: str) -> str | None:
    from kagra.contracts import resolve_asset

    if not name or name.lower() in ("builtin", "-", "none"):
        return None
    found = resolve_asset(kind, name, required=False)
    return str(found) if found else None


def run_live(args: argparse.Namespace) -> int:
    import kagra
    from kagra.camera3d import Camera3D
    from kagra.contracts import AssetKind, KagraContractError
    from kagra.samples import SAMPLE_LICENSE, ensure_vrm

    try:
        vrm = ensure_vrm(args.vrm, download=not args.offline)
    except KagraContractError as e:
        print(e, file=sys.stderr)
        return 2

    width = args.width or (360 if args.mascot else 1280)
    height = args.height or (640 if args.mascot else 720)
    mascot = bool(args.mascot)
    loop = bool(args.loop or mascot)

    print(f"[kagra] vrm={vrm}")
    print(f"[kagra] {SAMPLE_LICENSE}")

    kagra.init(
        title="KAGRA — VRM Live",
        width=width,
        height=height,
        visible=not args.hidden,
        decorations=not mascot,
        always_on_top=mascot,
        transparent=mascot,
    )
    cam = Camera3D(width, height, fov_deg=32.0)
    # theta=0 → +Z 側。radius を広げて足元〜両手上げまで入るようにする。
    cam.use_orbit(
        radius=2.4 if mascot else 3.7,
        theta=0.0,
        phi=0.10 if mascot else 0.16,
        target=(0.0, 0.95 if mascot else 0.82, 0.0),
    )

    max_frames = args.max_frames if args.max_frames > 0 else None
    shot_at = max(1, (args.max_frames // 2)) if max_frames else None
    state: dict = {"av": None, "floor": [], "venue": None, "sky": None, "t": 0.0}

    def on_ready():
        try:
            kagra.font()
        except RuntimeError as e:
            print(f"[kagra] font skipped: {e}", file=sys.stderr)
        kagra.set_light_dir(0.35, 1.0, 0.55)
        kagra.set_shadow_enabled(not mascot)
        if not mascot:
            sky = _resolve_optional(AssetKind.TEXTURE, args.backdrop)
            hall = _resolve_optional(AssetKind.GLTF, args.stage)
            if sky:
                state["sky"] = kagra.stage(sky)
                print(f"[kagra] backdrop={sky}")
            if hall:
                state["venue"] = kagra.stage(hall)
                print(f"[kagra] stage={hall}")
            else:
                state["floor"] = _stage_meshes()
        av = kagra.avatar(str(vrm))
        dance = _resolve_optional(AssetKind.ANY, args.dance)
        if dance:
            av.dance(dance)
            print(f"[kagra] dance={dance}")
        else:
            av.dance()
            print(f"[kagra] dance=bundled (no {args.dance})")
        av.enable_lookat(eye_height=1.42, smooth_speed=4.0, head_weight=0.22, neck_weight=0.12)
        av.enable_emotion(blend_speed=1.8)
        song = _resolve_optional(AssetKind.AUDIO, args.song)
        duration = av.sing(song, loop=loop) if song else av.sing(loop=loop)
        print(f"[kagra] song={'builtin' if song is None else song} {duration:.1f}s — ESC to quit")
        if loop:
            print("[kagra] loop on — OBS: Game Capture this window (no YouTube API)")
        if mascot:
            print("[kagra] mascot — always on top, look at mouse")
        state["av"] = av

    def update(dt: float):
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        av = state["av"]
        if av is None:
            return
        state["t"] += dt
        t = state["t"]
        if not args.no_orbit and not mascot:
            cam.orbit_by(dt * 0.25, 0)
        if av.lookat:
            if mascot:
                mx, my = kagra.mouse()
                av.look_at_screen(mx, my, width, height)
            else:
                cam._update_orbit()
                cx, cy, cz = cam.position
                av.look_at_3d(
                    cx + math.sin(t * 0.35) * 0.35,
                    cy + math.sin(t * 0.22) * 0.12,
                    cz,
                )
        if av.emotion is not None:
            open_ = av.lipsync.mouth_open if av.lipsync else 0.0
            av.emotion.blend({
                "joy": min(0.5, open_ * 0.7),
                "fun": 0.18 + min(0.35, open_ * 0.25),
            })
        av.update(dt)
        cam.update(kagra.get_engine())
        if shot_at is not None and args.screenshot and kagra.tick_count() == shot_at:
            os.makedirs(os.path.dirname(args.screenshot) or ".", exist_ok=True)
            kagra.screenshot(args.screenshot)

    def draw():
        if mascot:
            kagra.cls(0, 0, 0)
        else:
            kagra.cls(16, 12, 32)
        if state["sky"] is not None:
            state["sky"].draw()
        if state["venue"] is not None:
            state["venue"].draw()
        else:
            for tex, verts, idx in state["floor"]:
                kagra.draw_mesh_3d(tex, verts, idx)
        av = state["av"]
        if av is not None:
            kagra.draw_vrm(av.vrm_id)
        if not mascot:
            kagra.text("KAGRA", 16, 16, 18, (220, 200, 140))
            kagra.text("ESC", 16, height - 32, 14, (140, 140, 160))

    kagra.run(update, draw, on_ready=on_ready, max_frames=max_frames, fixed_dt=(1.0 / 60.0 if max_frames else None))
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return run_live(args)


if __name__ == "__main__":
    raise SystemExit(main())
