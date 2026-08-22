"""同梱ライブデモ。``python -m kagra`` / ``python -m kagra.demo``。

外部アセットなしで「VRM が歌って踊る」を 60 秒以内に見せる。
VRM が無ければサンプルを 1 回だけダウンロードする。
"""
from __future__ import annotations

import argparse
import sys


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="python -m kagra",
        description="KAGRA live demo — a VRM avatar that sings and dances.",
    )
    p.add_argument("--vrm", default="Emma", help="VRM name, alias, or path (default: Emma)")
    p.add_argument("--offline", action="store_true", help="do not download a sample VRM")
    p.add_argument("--width", type=int, default=1280)
    p.add_argument("--height", type=int, default=720)
    p.add_argument("--no-orbit", action="store_true", help="keep the camera still")
    return p


def run_live(args: argparse.Namespace) -> int:
    import kagra
    from kagra.camera3d import Camera3D
    from kagra.contracts import KagraContractError
    from kagra.samples import SAMPLE_LICENSE, ensure_vrm

    try:
        vrm = ensure_vrm(args.vrm, download=not args.offline)
    except KagraContractError as e:
        print(e, file=sys.stderr)
        return 2

    print(f"[kagra] vrm={vrm}")
    print(f"[kagra] {SAMPLE_LICENSE}")

    kagra.init(title="KAGRA — VRM Live", width=args.width, height=args.height)
    try:
        kagra.font()
    except RuntimeError as e:
        print(f"[kagra] font skipped: {e}", file=sys.stderr)

    cam = Camera3D(args.width, args.height)
    cam.use_orbit(radius=2.6, phi=0.1, target=(0.0, 0.9, 0.0))

    av = kagra.avatar(str(vrm))
    av.dance()
    duration = av.sing()
    print(f"[kagra] song {duration:.1f}s — ESC to quit")

    def update(dt: float):
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        av.update(dt)
        if not args.no_orbit:
            cam.orbit_by(dt * 0.25, 0)
        cam.update(kagra.get_engine())

    def draw():
        kagra.cls(16, 12, 32)
        kagra.draw_vrm(av.vrm_id)
        kagra.text("KAGRA  python -m kagra", 16, 16, 22, (255, 220, 120))
        kagra.text("ESC quit", 16, args.height - 36, 16, (180, 180, 200))

    kagra.run(update, draw)
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return run_live(args)


if __name__ == "__main__":
    raise SystemExit(main())
