"""``kagra`` / ``python -m kagra`` エントリ。"""
from __future__ import annotations

import sys

_HELP = """KAGRA — Kernel for Anime/Game Runtime Architecture

Usage:
  python -m kagra              sing & dance demo (downloads a sample VRM once)
  python -m kagra --loop --stream   HUD + virtual camera (needs kagra[stream])
  python -m kagra demo         same
  python -m kagra verify FILE  run an agent verify scenario
  python -m kagra render-world FILE [OUT.png]  shared wgpu 30 offscreen of a World.dump JSON
  python -m kagra play-world [FILE]  shared wgpu 30 desktop window (WASD; official Crest play)
  kagra --help
"""


def main(argv: list[str] | None = None) -> int:
    from kagra.launch import warn_checkout_shadow

    warn_checkout_shadow()
    argv = list(sys.argv[1:] if argv is None else argv)
    if not argv or argv[0] in ("-h", "--help"):
        if argv and argv[0] in ("-h", "--help"):
            print(_HELP)
            return 0
        from kagra.demo import main as demo_main
        return demo_main([])
    cmd, rest = argv[0], argv[1:]
    if cmd == "demo":
        from kagra.demo import main as demo_main
        return demo_main(rest)
    if cmd == "verify":
        from kagra.verify import main as verify_main
        return verify_main(rest)
    if cmd in ("render-world", "render_world"):
        from kagra.render_world import main as render_world_main
        return render_world_main(rest)
    if cmd in ("play-world", "play_world"):
        from kagra.play_world import main as play_world_main
        return play_world_main(rest)
    # 未知のサブコマンドはデモの引数として扱う（--offline 等）
    if cmd.startswith("-"):
        from kagra.demo import main as demo_main
        return demo_main(argv)
    print(_HELP, file=sys.stderr)
    print(f"unknown command: {cmd}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
