#!/usr/bin/env python3
"""Rebuild the embedded font subsets in ``kagra-shared/assets``.

The HUD fonts are compiled into the wasm, so they are cut down to what the games use:

* ``ZenMaruGothic-Bold.ttf`` (SIL OFL): ASCII, Latin-1, all kana, CJK punctuation, full-width
  forms, a few symbols, and every character at or above U+3000 found in ``kagra-shared/src``,
  ``games/*.json`` and ``kagra-shared/tests/fixtures/*.json``. Symbols the games write
  (U+2000..U+2BFF: arrows, shapes, marks) are kept too when the font has them; the ones it
  lacks are printed. A symbol neither font has must be one the engine draws itself
  (``kagra-shared/src/glyph_symbols.rs``) or it shows as a blank: the Rust test
  ``font::tests::every_symbol_in_the_games_can_be_drawn`` names it.
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


def symbols_in(text: str, *, code: bool) -> set[str]:
    """Symbols (U+2000..U+2BFF) a game writes. In Rust, comments are left out."""
    lines = (line.split("//")[0] for line in text.splitlines()) if code else [text]
    return {ch for line in lines for ch in line if 0x2000 <= ord(ch) <= 0x2BFF}


def japanese_chars() -> str:
    used = set()
    files = list((ROOT / "kagra-shared" / "src").rglob("*.rs"))
    files += list((ROOT / "games").glob("*.json"))
    files += list((ROOT / "kagra-shared" / "tests" / "fixtures").glob("*.json"))
    for f in files:
        text = f.read_text(encoding="utf-8", errors="ignore")
        used.update(ch for ch in text if ord(ch) >= 0x3000)
        used.update(symbols_in(text, code=f.suffix == ".rs"))
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
        wanted = japanese_chars()
        (t / "jp.txt").write_text(wanted, encoding="utf-8")
        from fontTools.ttLib import TTFont

        have = TTFont(t / "zen.ttf").getBestCmap()
        lacking = sorted(ch for ch in wanted if 0x2000 <= ord(ch) <= 0x2BFF and ord(ch) not in have)
        if lacking:
            print("symbols Zen Maru Gothic does not have (the engine must draw them): " + " ".join(lacking))
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
