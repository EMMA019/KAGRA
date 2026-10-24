"""tools/gen_assets/hop_cover.py: sizes, determinism, and that the committed PNGs match it."""
from __future__ import annotations

import importlib.util
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("hop_cover", ROOT / "tools" / "gen_assets" / "hop_cover.py")
assert _spec is not None and _spec.loader is not None
hc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(hc)


def decode(png: bytes):
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, size = 8, b"", None
    while pos < len(png):
        n, tag = struct.unpack(">I4s", png[pos:pos + 8])
        body = png[pos + 8:pos + 8 + n]
        assert zlib.crc32(tag + body) & 0xFFFFFFFF == struct.unpack(">I", png[pos + 8 + n:pos + 12 + n])[0]
        if tag == b"IHDR":
            size = struct.unpack(">II", body[:8])
            assert body[8:10] == b"\x08\x02"
        elif tag == b"IDAT":
            idat += body
        pos += 12 + n
    return size, zlib.decompress(idat)


def test_small_image_decodes_to_its_size():
    (w, h), raw = decode(hc.png_bytes(64, 48, True))
    assert (w, h) == (64, 48) and len(raw) == h * (1 + 3 * w)


def test_same_input_same_bytes():
    assert hc.png_bytes(48, 48, True) == hc.png_bytes(48, 48, True)


def test_titled_differs_from_notext():
    assert hc.png_bytes(64, 64, True) != hc.png_bytes(64, 64, False)


def test_names_and_sizes_are_consistent():
    for name, (w, h, titled) in hc.IMAGES.items():
        assert name.split("_")[1].startswith(f"{w}x{h}"), name
        assert titled == (not name.endswith("notext")), name
    sizes = {(w, h) for w, h, _ in hc.IMAGES.values()}
    assert {(1920, 1080), (1280, 720), (800, 1200), (800, 800), (512, 512), (512, 384), (200, 120)} <= sizes


def test_committed_pngs_have_the_declared_sizes():
    for name, (w, h, _) in hc.IMAGES.items():
        path = hc.OUT_DIR / f"{name}.png"
        assert path.is_file(), f"{path} missing; run tools/gen_assets/hop_cover.py"
        head = path.read_bytes()[:24]
        assert head[:8] == b"\x89PNG\r\n\x1a\n"
        assert struct.unpack(">II", head[16:24]) == (w, h), name


def test_committed_small_png_matches_the_generator():
    w, h, titled = hc.IMAGES["landscape_200x120_notext"]
    _, committed = decode((hc.OUT_DIR / "landscape_200x120_notext.png").read_bytes())
    _, fresh = decode(hc.png_bytes(w, h, titled))
    assert len(committed) == len(fresh)
    assert max(abs(a - b) for a, b in zip(committed, fresh)) <= 2, "stale; run tools/gen_assets/hop_cover.py"


def test_cli_list_and_unknown(capsys, tmp_path):
    assert hc.main(["--list"]) == 0
    assert "landscape_1920x1080.png" in capsys.readouterr().out
    assert hc.main(["--only", "nope", "--out", str(tmp_path)]) == 2
    assert hc.main(["--only", "landscape_200x120_notext", "--out", str(tmp_path)]) == 0
    assert (tmp_path / "landscape_200x120_notext.png").is_file()


def test_letters_cover_the_title():
    assert set(hc.TITLE.replace(" ", "")) <= set(hc.ca.LETTERS)
