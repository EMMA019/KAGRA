#!/usr/bin/env python3
"""Rebuild a 「直してここ」 moment and replay it headlessly.

Accepts a GitHub issue number, a file, or ``-`` for stdin. The body may be
a ``KAGRA-PLAYTEST-1`` block or the report JSON itself.

Exit codes:
  0  replayed, or ``--dump-only`` wrote the report
  1  the text did not contain a report
  2  the dump is not a WorldDoc (no ``version``), or the shared module is missing
  3  the world loaded but no GPU adapter could draw it

``--dump-only`` writes ``report.json`` and, when present, ``dump.json``, and
does not import the native module. Walk and confirm events in
``(dump_t, captured_t]`` are replayed. Gaps are not filled.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


def _playtest():
    import kagra.playtest as playtest

    return playtest


def _read_source(source: str) -> str:
    if source == "-":
        return sys.stdin.read()
    path = Path(source)
    if path.is_file():
        return path.read_text(encoding="utf-8")
    if source.isdigit():
        proc = subprocess.run(
            [
                "gh",
                "issue",
                "view",
                source,
                "--repo",
                "EMMA019/KAGRA",
                "--json",
                "title,body",
            ],
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            raise SystemExit(proc.stderr.strip() or f"gh issue view {source} failed")
        data = json.loads(proc.stdout)
        return f"{data.get('title', '')}\n{data.get('body', '')}"
    raise SystemExit(f"not a file or issue number: {source}")


def _write_report(out: Path, report: dict) -> None:
    out.mkdir(parents=True, exist_ok=True)
    (out / "report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    dump = report.get("dump")
    if isinstance(dump, dict):
        (out / "dump.json").write_text(
            json.dumps(dump, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )


def _load_worldplay():
    try:
        import kagra_shared as ks
    except ImportError:
        import kagra.kagra_shared as ks
    return ks.WorldPlay


def _replay(play, report: dict) -> int:
    playtest = _playtest()
    steps = 0
    for ev in playtest.iter_replay(report):
        if ev.get("kind") == "confirm":
            play.confirm()
            steps += 1
            continue
        play.set_input(
            float(ev.get("lx") or 0.0),
            float(ev.get("lz") or 0.0),
            bool(ev.get("jump")),
            bool(ev.get("attack")),
            bool(ev.get("dodge")),
        )
        play.tick(float(ev.get("dt") or 0.0))
        steps += 1
    return steps


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Replay a KAGRA playtest report")
    parser.add_argument("source", help="issue number, block file, or - for stdin")
    parser.add_argument(
        "--out",
        default="scratch/playtest-repro",
        help="directory for report.json, dump.json, and replay.png",
    )
    parser.add_argument(
        "--dump-only",
        action="store_true",
        help="write the decoded report and skip the native replay",
    )
    args = parser.parse_args(argv)
    playtest = _playtest()
    try:
        report = playtest.decode_block(_read_source(args.source))
    except ValueError as exc:
        print(f"playtest: {exc}", file=sys.stderr)
        return 1

    out = Path(args.out)
    _write_report(out, report)
    dump = report.get("dump")
    if not isinstance(dump, dict) or "version" not in dump:
        print("playtest: dump has no WorldDoc version", file=sys.stderr)
        return 2
    if args.dump_only:
        print(f"wrote {out / 'report.json'}")
        return 0

    try:
        world_play = _load_worldplay()
    except ImportError as exc:
        print(f"playtest: {exc}", file=sys.stderr)
        return 2

    play = world_play.from_json(json.dumps(dump, ensure_ascii=False))
    play.confirm()
    steps = _replay(play, report)
    device = report.get("device") if isinstance(report.get("device"), dict) else {}
    width = int(device.get("width") or 1280)
    height = int(device.get("height") or 720)
    width = max(1, width)
    height = max(1, height)
    try:
        from kagra.gameloop import last_draw_was_fallback, render_rgba, rgba_to_png
    except ImportError as exc:
        print(f"playtest: {exc}", file=sys.stderr)
        return 2
    before = last_draw_was_fallback()
    try:
        rgba = render_rgba(json.loads(play.dump()), width, height)
    except Exception as exc:
        print(f"playtest: render failed: {exc}", file=sys.stderr)
        return 3
    if last_draw_was_fallback() and not before:
        print("playtest: no GPU adapter", file=sys.stderr)
        return 3
    shot = out / "replay.png"
    shot.write_bytes(rgba_to_png(rgba, width, height))
    print(f"replayed {steps} events -> {shot}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
