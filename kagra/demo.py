"""同梱ライブデモ。``python -m kagra`` / ``python -m kagra.demo``。

``assets/cute_song_trial.wav`` と ``assets/Samba Dancing.fbx`` があれば
それを歌う／踊る。無ければ内蔵ソングと同梱ダンスにフォールバックする。
VRM が無ければサンプルを 1 回だけダウンロードする。

Windows ではレンダラが ``run()`` の中でしか起きないので、
``avatar()`` / ``font()`` は ``on_ready`` で呼ぶ。
"""
from __future__ import annotations

import argparse
import os
import sys

DEFAULT_DANCE = "Samba Dancing"
DEFAULT_SONG = "cute_song_trial"


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
    p.add_argument("--width", type=int, default=1280)
    p.add_argument("--height", type=int, default=720)
    p.add_argument("--no-orbit", action="store_true", help="keep the camera still")
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

    print(f"[kagra] vrm={vrm}")
    print(f"[kagra] {SAMPLE_LICENSE}")

    kagra.init(
        title="KAGRA — VRM Live",
        width=args.width,
        height=args.height,
        visible=not args.hidden,
    )
    cam = Camera3D(args.width, args.height)
    # theta=0 → +Z 側。このクリップ／モデルはそちらが顔。
    cam.use_orbit(radius=2.8, theta=0.0, phi=0.12, target=(0.0, 0.9, 0.0))

    max_frames = args.max_frames if args.max_frames > 0 else None
    shot_at = max(1, (args.max_frames // 2)) if max_frames else None
    state: dict = {"av": None}

    def on_ready():
        try:
            kagra.font()
        except RuntimeError as e:
            print(f"[kagra] font skipped: {e}", file=sys.stderr)
        av = kagra.avatar(str(vrm))
        dance = _resolve_optional(AssetKind.ANY, args.dance)
        if dance:
            av.dance(dance)
            print(f"[kagra] dance={dance}")
        else:
            av.dance()
            print(f"[kagra] dance=bundled (no {args.dance})")
        song = _resolve_optional(AssetKind.AUDIO, args.song)
        duration = av.sing(song) if song else av.sing()
        print(f"[kagra] song={'builtin' if song is None else song} {duration:.1f}s — ESC to quit")
        state["av"] = av

    def update(dt: float):
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        av = state["av"]
        if av is None:
            return
        av.update(dt)
        if not args.no_orbit:
            cam.orbit_by(dt * 0.25, 0)
        cam.update(kagra.get_engine())
        if shot_at is not None and args.screenshot and kagra.tick_count() == shot_at:
            os.makedirs(os.path.dirname(args.screenshot) or ".", exist_ok=True)
            kagra.screenshot(args.screenshot)

    def draw():
        kagra.cls(16, 12, 32)
        av = state["av"]
        if av is not None:
            kagra.draw_vrm(av.vrm_id)
        kagra.text("KAGRA  python -m kagra", 16, 16, 22, (255, 220, 120))
        kagra.text("ESC quit", 16, args.height - 36, 16, (180, 180, 200))

    kagra.run(update, draw, on_ready=on_ready, max_frames=max_frames, fixed_dt=(1.0 / 60.0 if max_frames else None))
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return run_live(args)


if __name__ == "__main__":
    raise SystemExit(main())
