#!/usr/bin/env python3
"""Rebuild the embedded font subsets in ``kagra-shared/assets``.

The HUD fonts are compiled into the wasm, so they are cut down to what the games use:

* ``ZenMaruGothic-Bold.ttf`` (SIL OFL): ASCII, Latin-1, all kana, CJK punctuation, full-width
  forms, a few symbols, and every character at or above U+3000 found in ``kagra-shared/src``,
  ``games/*.json`` and ``kagra-shared/tests/fixtures/*.json``.
* ``Cinzel-Bold.ttf`` (SIL OFL): the wght=700 instance of the variable font, Latin only.

Run it after adding Japanese text with a kanji the subset lacks (the Rust test
``font::tests::every_japanese_character_in_the_games_is_in_the_subset`` names it)::

    pip install fonttools
    python tools/subset_fonts.py

Neither font declares a Reserved Font Name, so a subset keeps its name. The OFL texts sit next
to the fonts (``*-OFL.txt``) and the ledger entries are ``font-zenmarugothic`` and
``font-cinzel``.
"""
from __future__ import annotations

import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "kagra-shared" / "assets"
GOOGLE_FONTS = "https://raw.githubusercontent.com/google/fonts/main/ofl"
ZEN_URL = f"{GOOGLE_FONTS}/zenmarugothic/ZenMaruGothic-Bold.ttf"
CINZEL_URL = f"{GOOGLE_FONTS}/cinzel/Cinzel%5Bwght%5D.ttf"
SYMBOLS = "・ー〜…‥「」『』【】→←↑↓★☆●○◆◇■□▲△▼▽♪♥♡×÷±※"


def japanese_chars() -> str:
    used = set()
    files = list((ROOT / "kagra-shared" / "src").rglob("*.rs"))
    files += list((ROOT / "games").glob("*.json"))
    files += list((ROOT / "kagra-shared" / "tests" / "fixtures").glob("*.json"))
    for f in files:
        used.update(ch for ch in f.read_text(encoding="utf-8", errors="ignore") if ord(ch) >= 0x3000)
    ranges = [(0x20, 0x7F), (0xA0, 0x100), (0x3000, 0x3040), (0x3041, 0x3097), (0x30A0, 0x3100), (0xFF01, 0xFF5F)]
    base = {chr(c) for lo, hi in ranges for c in range(lo, hi)} | set(SYMBOLS)
    return "".join(sorted(base | used))


def fetch(url: str, dest: Path) -> None:
    with urllib.request.urlopen(url) as r:
        dest.write_bytes(r.read())


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        fetch(ZEN_URL, t / "zen.ttf")
        fetch(CINZEL_URL, t / "cinzel_var.ttf")
        (t / "jp.txt").write_text(japanese_chars(), encoding="utf-8")
        common = ["--layout-features=*", "--no-hinting"]
        subprocess.run(
            ["pyftsubset", str(t / "zen.ttf"), f"--text-file={t / 'jp.txt'}", "--desubroutinize",
             f"--output-file={ASSETS / 'ZenMaruGothic-Bold.ttf'}", *common],
            check=True,
        )
        subprocess.run(
            [sys.executable, "-m", "fontTools.varLib.instancer", str(t / "cinzel_var.ttf"), "wght=700",
             "-o", str(t / "cinzel.ttf")],
            check=True,
        )
        subprocess.run(
            ["pyftsubset", str(t / "cinzel.ttf"),
             "--unicodes=U+0020-007E,U+00A0-00FF,U+2013-2014,U+2018-201D,U+2026,U+00D7",
             f"--output-file={ASSETS / 'Cinzel-Bold.ttf'}", *common],
            check=True,
        )
    for name in ("ZenMaruGothic-Bold.ttf", "Cinzel-Bold.ttf"):
        print(f"{name}: {(ASSETS / name).stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
