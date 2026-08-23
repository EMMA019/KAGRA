"""同梱ライブデモ。``python -m kagra`` / ``python -m kagra.demo``。

``assets/`` に Mixamo の ``.fbx`` や ``.vrma`` を落とすと、そのまま再生する。
複数あればプレイリスト（SPACE / N で次）。``--dance path`` で 1 本に固定。
無ければ同梱ダンス。曲は ``assets/cute_song_trial.wav``、無ければ内蔵ソング。
VRM が無ければサンプルを 1 回だけダウンロードする。

``--loop`` で曲を繰り返す（OBS でこの窓をキャプチャして配信）。
``--stream`` で字幕 HUD + 仮想カメラ（``kagra[stream]``）+ JSONL チャット受け口。
``--mascot`` で最前面の枠なし窓（デスクトップマスコット）。
``--stage venue.glb`` / ``--backdrop sky.png`` で外部会場。無ければプロシージャル空 + 円盤。

Windows ではレンダラが ``run()`` の中でしか起きないので、
``avatar()`` / ``font()`` は ``on_ready`` で呼ぶ。
"""
from __future__ import annotations

import argparse
import math
import os
import sys
from pathlib import Path

DEFAULT_DANCE = "auto"
DEFAULT_SONG = "cute_song_trial"
_AUTO_DANCE = frozenset({"auto", "all", "*"})


def _stage_meshes():
    """暗い円盤 + 暖色スポット。チェッカーはデバッグに見えるので使わない。"""
    from kagra.look import make_live_floor

    return make_live_floor()


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="python -m kagra",
        description="KAGRA live demo — a VRM avatar that sings and dances.",
    )
    p.add_argument("--vrm", default="Emma", help="VRM name, alias, or path (default: Emma)")
    p.add_argument(
        "--dance",
        default=DEFAULT_DANCE,
        help="auto = every .fbx/.vrma in assets/; or a VRMA/BVH/FBX name/path",
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
    p.add_argument(
        "--stream",
        action="store_true",
        help="HUD + optional virtual camera (pip install 'kagra[stream]')",
    )
    p.add_argument(
        "--chat",
        default="",
        help="JSONL chat inbox ({user,text} per line). default with --stream: kagra-chat.jsonl",
    )
    return p


def _resolve_optional(kind, name: str) -> str | None:
    from kagra.contracts import resolve_asset

    if not name or name.lower() in ("builtin", "-", "none"):
        return None
    found = resolve_asset(kind, name, required=False)
    return str(found) if found else None


def _is_auto_dance(name: str) -> bool:
    return (name or "").strip().lower() in _AUTO_DANCE


def _unique_clip_name(path: Path, used: set[str]) -> str:
    base = path.stem or "motion"
    name = base
    n = 2
    while name in used:
        name = f"{base}_{n}"
        n += 1
    used.add(name)
    return name


def _frames_duration(frames) -> float:
    """``_anim._clips`` のフレーム列から秒数。最低 0.5s。"""
    total = 0.0
    for frame in frames or ():
        if hasattr(frame, "duration"):
            try:
                total += float(frame.duration)
                continue
            except (TypeError, ValueError):
                pass
        if isinstance(frame, (tuple, list)) and len(frame) >= 2:
            try:
                total += float(frame[1])
            except (TypeError, ValueError):
                continue
    return max(0.5, total)


def _pressed_any(*names: str) -> bool:
    import kagra

    for name in names:
        try:
            if kagra.pressed(name):
                return True
        except ValueError:
            continue
    return False


def _discover_dance_paths(name: str) -> list[Path]:
    from kagra.contracts import AssetKind, list_motion_drops

    if _is_auto_dance(name):
        return list(list_motion_drops())
    found = _resolve_optional(AssetKind.ANY, name)
    return [Path(found)] if found else []


def _hud_dance_label(state: dict) -> str:
    playlist = state.get("playlist") or []
    if not playlist:
        return ""
    i = int(state.get("playlist_i") or 0) % len(playlist)
    _name, path = playlist[i]
    label = path.name
    if len(playlist) > 1:
        label = f"{i + 1}/{len(playlist)}  {label}"
    return label


def _apply_drop_clip(av, clip: str) -> None:
    av.play(clip, loop=True, fade=0.3)
    if not av._clip_has_fingers(clip):
        av.relax_hands()
    if not av._grounding:
        av.enable_grounding()


def _play_playlist_index(av, state: dict, index: int) -> None:
    playlist = state.get("playlist") or []
    if not playlist:
        return
    i = index % len(playlist)
    name, path = playlist[i]
    _apply_drop_clip(av, name)
    state["playlist_i"] = i
    state["clip_t"] = 0.0
    state["clip_dur"] = _frames_duration(av._anim._clips.get(name) or [])
    print(f"[kagra] dance={path} ({i + 1}/{len(playlist)})")
    hud = state.get("hud")
    if hud is not None:
        hud.subtitle = _hud_dance_label(state)


def _start_dances(av, dance_name: str, state: dict) -> None:
    paths = _discover_dance_paths(dance_name)
    used: set[str] = set()
    entries: list[tuple[str, Path]] = []
    for path in paths:
        name = _unique_clip_name(path, used)
        try:
            av.load_motion(name, str(path))
        except Exception as e:
            print(f"[kagra] skip {path.name}: {e}", file=sys.stderr)
            continue
        entries.append((name, path))
    if not entries:
        av.dance()
        if _is_auto_dance(dance_name):
            print("[kagra] dance=bundled  (no .fbx / .vrma in assets/)")
            print("[kagra] drop Mixamo .fbx in assets/  (or assets/anim, assets/motion)")
        else:
            print(f"[kagra] dance=bundled (no {dance_name})")
        return
    print(f"[kagra] drop-in motions ({len(entries)}):")
    for i, (_n, path) in enumerate(entries, 1):
        print(f"  {i}. {path.name}")
    if _is_auto_dance(dance_name) or len(entries) > 1:
        print("[kagra] drop more .fbx / .vrma in assets/  (SPACE / N = next)")
    state["playlist"] = entries
    _play_playlist_index(av, state, 0)


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
    if mascot:
        cam.use_orbit(radius=2.4, theta=0.0, phi=0.10, target=(0.0, 0.95, 0.0))
    elif args.no_orbit:
        cam.use_orbit(radius=3.35, theta=0.0, phi=0.14, target=(0.0, 0.84, 0.0))
    else:
        cam.use_showcase()

    max_frames = args.max_frames if args.max_frames > 0 else None
    shot_at = max(1, (args.max_frames // 2)) if max_frames else None
    streaming = bool(args.stream)
    chat_path = args.chat or ("kagra-chat.jsonl" if streaming else "")
    state: dict = {
        "av": None,
        "floor": [],
        "venue": None,
        "sky": None,
        "sky_mesh": None,
        "t": 0.0,
        "hud": None,
        "inbox": None,
        "cam": None,
        "song_label": "builtin",
        "playlist": [],
        "playlist_i": 0,
        "clip_t": 0.0,
        "clip_dur": 0.0,
    }

    def on_ready():
        try:
            kagra.font()
        except RuntimeError as e:
            print(f"[kagra] font skipped: {e}", file=sys.stderr)
        from kagra.look import apply_live_look, load_default_sky

        apply_live_look(mascot=mascot)
        kagra.set_camera3d(cam)
        if not mascot:
            sky = _resolve_optional(AssetKind.TEXTURE, args.backdrop)
            hall = _resolve_optional(AssetKind.GLTF, args.stage)
            if sky:
                state["sky"] = kagra.stage(sky)
                print(f"[kagra] backdrop={sky}")
            else:
                tex, verts, idx = load_default_sky()
                state["sky_mesh"] = (tex, verts, idx)
            if hall:
                state["venue"] = kagra.stage(hall)
                print(f"[kagra] stage={hall}")
            else:
                state["floor"] = _stage_meshes()
        av = kagra.avatar(str(vrm))
        _start_dances(av, args.dance, state)
        av.enable_lookat(eye_height=1.42, smooth_speed=4.0, head_weight=0.22, neck_weight=0.12)
        av.enable_emotion(blend_speed=1.8)
        song = _resolve_optional(AssetKind.AUDIO, args.song)
        duration = av.sing(song, loop=loop) if song else av.sing(loop=loop)
        song_label = "builtin" if song is None else os.path.basename(str(song))
        state["song_label"] = song_label
        print(f"[kagra] song={'builtin' if song is None else song} {duration:.1f}s — ESC to quit")
        if loop:
            print("[kagra] loop on — OBS: Game Capture this window (no YouTube API)")
        if mascot:
            print("[kagra] mascot — always on top, look at mouse")
        if not mascot:
            from kagra.stream import StreamHud

            hud = StreamHud(
                song=f"♪ {song_label}",
                credit="Alicia Solid © Dwango",
            )
            dance_label = _hud_dance_label(state)
            if dance_label:
                hud.subtitle = dance_label
            elif streaming:
                hud.subtitle = "KAGRA live"
            state["hud"] = hud
        if streaming:
            from kagra.stream import ChatInbox, VirtualCam

            if chat_path:
                state["inbox"] = ChatInbox(chat_path)
                print(f"[kagra] chat inbox → {chat_path}  (echo '{{\"user\":\"a\",\"text\":\"hi\"}}' >> …)")
            try:
                state["cam"] = VirtualCam(fps=30).start(width, height)
                print("[kagra] virtual cam on — OBS: Video Capture Device / OBS Virtual Camera")
            except Exception as e:
                print(f"[kagra] virtual cam skipped ({e})")
                print('[kagra]   pip install "kagra[stream]"  then retry --stream')
        state["av"] = av

    def update(dt: float):
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        av = state["av"]
        if av is None:
            return
        playlist = state.get("playlist") or []
        if len(playlist) > 1:
            if _pressed_any("SPACE", "N"):
                _play_playlist_index(av, state, int(state.get("playlist_i") or 0) + 1)
            else:
                state["clip_t"] = float(state.get("clip_t") or 0.0) + dt
                if state["clip_t"] >= float(state.get("clip_dur") or 0.5):
                    _play_playlist_index(av, state, int(state.get("playlist_i") or 0) + 1)
        cam_out = state.get("cam")
        if cam_out is not None:
            cam_out.send()
        inbox = state.get("inbox")
        hud = state.get("hud")
        if inbox is not None and hud is not None:
            for msg in inbox.poll():
                hud.push_chat(msg)
                hud.subtitle = msg.text
        state["t"] += dt
        t = state["t"]
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
        cam.update(kagra.get_engine(), None if (args.no_orbit or mascot) else dt)
        if shot_at is not None and args.screenshot and kagra.tick_count() == shot_at:
            os.makedirs(os.path.dirname(args.screenshot) or ".", exist_ok=True)
            kagra.screenshot(args.screenshot)

    def draw():
        if mascot:
            kagra.cls(0, 0, 0)
        else:
            kagra.cls(8, 6, 18)
        if state["sky"] is not None:
            state["sky"].draw()
        elif state.get("sky_mesh") is not None:
            tex, verts, idx = state["sky_mesh"]
            kagra.draw_mesh_3d(tex, verts, idx)
        if state["venue"] is not None:
            state["venue"].draw()
        else:
            for tex, verts, idx in state["floor"]:
                kagra.draw_mesh_3d(tex, verts, idx)
        av = state["av"]
        if av is not None:
            kagra.draw_vrm(av.vrm_id)
        if not mascot:
            from kagra.look import draw_vignette

            draw_vignette(width, height, 0.40)
        hud = state.get("hud")
        if hud is not None:
            hud.draw(width, height)
        elif not mascot:
            kagra.text("ESC", 16, height - 32, 14, (140, 140, 160))

    kagra.run(update, draw, on_ready=on_ready, max_frames=max_frames, fixed_dt=(1.0 / 60.0 if max_frames else None))
    return 0


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return run_live(args)


if __name__ == "__main__":
    raise SystemExit(main())
