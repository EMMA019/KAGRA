"""tools/gen_assets/cover_art.py: sizes, determinism, and that the committed PNGs match it."""
from __future__ import annotations

import importlib.util
import struct
import zlib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("cover_art", ROOT / "tools" / "gen_assets" / "cover_art.py")
assert _spec is not None and _spec.loader is not None
ca = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ca)


def decode(png: bytes):
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, size = 8, b"", None
    while pos < len(png):
        n, tag = struct.unpack(">I4s", png[pos:pos + 8])
        body = png[pos + 8:pos + 8 + n]
        assert zlib.crc32(tag + body) & 0xFFFFFFFF == struct.unpack(">I", png[pos + 8 + n:pos + 12 + n])[0]
        if tag == b"IHDR":
            size = struct.unpack(">II", body[:8])
            assert body[8:10] == b"\x08\x02"      # 8-bit RGB
        elif tag == b"IDAT":
            idat += body
        pos += 12 + n
    return size, zlib.decompress(idat)


def test_small_image_decodes_to_its_size():
    png = ca.png_bytes(64, 48, True)
    (w, h), raw = decode(png)
    assert (w, h) == (64, 48) and len(raw) == h * (1 + 3 * w)


def test_same_input_same_bytes():
    assert ca.png_bytes(48, 48, True) == ca.png_bytes(48, 48, True)


def test_notext_differs_from_titled():
    assert ca.png_bytes(64, 64, True) != ca.png_bytes(64, 64, False)


def test_names_and_sizes_are_consistent():
    for name, (w, h, titled) in ca.IMAGES.items():
        assert name.split("_")[1].startswith(f"{w}x{h}"), name
        assert titled == (not name.endswith("notext")), name
    sizes = {(w, h) for w, h, _ in ca.IMAGES.values()}
    assert {(1920, 1080), (1280, 720), (800, 1200), (800, 800), (512, 512), (512, 384), (200, 120)} <= sizes


def test_committed_pngs_have_the_declared_sizes():
    for name, (w, h, _) in ca.IMAGES.items():
        path = ca.OUT_DIR / f"{name}.png"
        assert path.is_file(), f"{path} missing; run tools/gen_assets/cover_art.py"
        head = path.read_bytes()[:24]
        assert head[:8] == b"\x89PNG\r\n\x1a\n"
        assert struct.unpack(">II", head[16:24]) == (w, h), name


@pytest.mark.parametrize("name", ["landscape_200x120_notext"])
def test_committed_small_png_matches_the_generator(name):
    # Compare pixels, not file bytes: zlib builds differ in how they compress, and
    # libm can move a rounded channel by one step.
    w, h, titled = ca.IMAGES[name]
    _, committed = decode((ca.OUT_DIR / f"{name}.png").read_bytes())
    _, fresh = decode(ca.png_bytes(w, h, titled))
    assert len(committed) == len(fresh)
    worst = max(abs(a - b) for a, b in zip(committed, fresh))
    assert worst <= 2, f"{name}.png is stale; run tools/gen_assets/cover_art.py"


def test_cli_list_and_unknown(capsys, tmp_path):
    assert ca.main(["--list"]) == 0
    assert "landscape_1920x1080.png" in capsys.readouterr().out
    assert ca.main(["--only", "nope", "--out", str(tmp_path)]) == 2
    assert ca.main(["--only", "landscape_200x120_notext", "--out", str(tmp_path)]) == 0
    assert (tmp_path / "landscape_200x120_notext.png").is_file()


def test_letters_cover_the_title():
    assert set("NEON CAT".replace(" ", "")) <= set(ca.LETTERS)
