"""ゴールデン画像比較ヘルパ。

基準 PNG は tests/goldens/ に置く。更新は:

    KAGRA_UPDATE_GOLDENS=1 pytest tests -m golden
"""
from __future__ import annotations

import os
import struct
import zlib
from pathlib import Path

GOLDENS_DIR = Path(__file__).resolve().parent / "goldens"
DIFFS_DIR = Path(__file__).resolve().parents[1] / "scratch" / "golden_diffs"


def update_requested() -> bool:
    return os.environ.get("KAGRA_UPDATE_GOLDENS", "").strip() in ("1", "true", "TRUE", "yes")


def _read_png_rgba(path: Path) -> tuple[int, int, bytes]:
    """標準ライブラリだけで RGBA8 PNG を読む（Pillow 不要）。"""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"not a PNG: {path}"
    offset = 8
    width = height = None
    raw = b""
    while offset + 8 <= len(data):
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        ctype = data[offset + 4 : offset + 8]
        chunk = data[offset + 8 : offset + 8 + length]
        offset += 12 + length
        if ctype == b"IHDR":
            width, height, bit_depth, color_type, *_ = struct.unpack(">IIBBBBB", chunk)
            if bit_depth != 8 or color_type not in (2, 6):
                raise ValueError(f"unsupported PNG format in {path}")
        elif ctype == b"IDAT":
            raw += chunk
        elif ctype == b"IEND":
            break
    if width is None or height is None:
        raise ValueError(f"IHDR missing: {path}")
    decompressed = zlib.decompress(raw)
    channels = 4 if color_type == 6 else 3
    stride = 1 + width * channels
    rgba = bytearray(width * height * 4)
    prev = bytearray(width * channels)
    i = 0
    for y in range(height):
        filt = decompressed[y * stride]
        row = bytearray(decompressed[y * stride + 1 : (y + 1) * stride])
        if filt == 1:  # Sub
            for x in range(channels, len(row)):
                row[x] = (row[x] + row[x - channels]) & 255
        elif filt == 2:  # Up
            for x in range(len(row)):
                row[x] = (row[x] + prev[x]) & 255
        elif filt == 3:  # Average
            for x in range(len(row)):
                left = row[x - channels] if x >= channels else 0
                row[x] = (row[x] + ((left + prev[x]) // 2)) & 255
        elif filt == 4:  # Paeth
            for x in range(len(row)):
                a = row[x - channels] if x >= channels else 0
                b = prev[x]
                c = prev[x - channels] if x >= channels else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                row[x] = (row[x] + pr) & 255
        elif filt != 0:
            raise ValueError(f"unsupported filter {filt} in {path}")
        prev = row
        for x in range(width):
            if channels == 4:
                rgba[i : i + 4] = row[x * 4 : x * 4 + 4]
            else:
                rgba[i : i + 3] = row[x * 3 : x * 3 + 3]
                rgba[i + 3] = 255
            i += 4
    return width, height, bytes(rgba)


def _write_png_rgba(path: Path, width: int, height: int, rgba: bytes) -> None:
    """非圧縮フィルタ0の RGBA PNG を書く（差分画像用）。"""
    rows = b""
    row_bytes = width * 4
    for y in range(height):
        rows += b"\x00" + rgba[y * row_bytes : (y + 1) * row_bytes]
    compressed = zlib.compress(rows, 9)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        crc = zlib.crc32(tag + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", crc)

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", compressed)
        + chunk(b"IEND", b"")
    )


def compare_png(
    actual_path: Path,
    golden_name: str,
    *,
    max_mean_abs: float = 2.0,
    max_bad_ratio: float = 0.002,
    bad_threshold: int = 8,
) -> None:
    """actual を基準 PNG と比較する。差が閾値超なら AssertionError。

    Args:
        max_mean_abs: 全画素・全チャンネルの平均絶対誤差の上限
        max_bad_ratio: |diff| > bad_threshold の画素割合の上限
        bad_threshold: 「悪い画素」とみなすチャンネル差分
    """
    golden_path = GOLDENS_DIR / golden_name
    if update_requested() or not golden_path.exists():
        GOLDENS_DIR.mkdir(parents=True, exist_ok=True)
        golden_path.write_bytes(actual_path.read_bytes())
        if update_requested():
            return
        raise AssertionError(
            f"golden missing, wrote baseline: {golden_path}\n"
            "Re-run with KAGRA_UPDATE_GOLDENS=1 to acknowledge, or commit the new file."
        )

    aw, ah, actual = _read_png_rgba(actual_path)
    gw, gh, golden = _read_png_rgba(golden_path)
    assert (aw, ah) == (gw, gh), f"size mismatch: actual={aw}x{ah} golden={gw}x{gh}"

    n = len(actual)
    total_abs = 0
    bad = 0
    diff_img = bytearray(n)
    for i in range(0, n, 4):
        for c in range(3):
            d = abs(actual[i + c] - golden[i + c])
            total_abs += d
            if d > bad_threshold:
                bad += 1
            # 差分を赤く可視化（見やすいように増幅）
            diff_img[i + c] = min(255, d * 8) if c == 0 else 0
        diff_img[i + 3] = 255

    pixels = aw * ah
    mean_abs = total_abs / (pixels * 3)
    bad_ratio = bad / (pixels * 3)

    if mean_abs <= max_mean_abs and bad_ratio <= max_bad_ratio:
        return

    DIFFS_DIR.mkdir(parents=True, exist_ok=True)
    diff_path = DIFFS_DIR / f"diff_{golden_name}"
    _write_png_rgba(diff_path, aw, ah, bytes(diff_img))
    actual_copy = DIFFS_DIR / f"actual_{golden_name}"
    actual_copy.write_bytes(actual_path.read_bytes())
    raise AssertionError(
        f"golden mismatch for {golden_name}: "
        f"mean_abs={mean_abs:.3f} (max {max_mean_abs}), "
        f"bad_ratio={bad_ratio:.5f} (max {max_bad_ratio})\n"
        f"diff saved: {diff_path}"
    )


def png_mean_abs(path_a: Path, path_b: Path) -> float:
    """2 枚の PNG の RGB 平均絶対誤差。サイズが違えば ValueError。"""
    aw, ah, a = _read_png_rgba(path_a)
    bw, bh, b = _read_png_rgba(path_b)
    if (aw, ah) != (bw, bh):
        raise ValueError(f"size mismatch: {aw}x{ah} vs {bw}x{bh}")
    total = 0
    n = len(a)
    for i in range(0, n, 4):
        total += abs(a[i] - b[i]) + abs(a[i + 1] - b[i + 1]) + abs(a[i + 2] - b[i + 2])
    return total / (aw * ah * 3)


def assert_pngs_differ(
    path_a: Path,
    path_b: Path,
    *,
    min_mean_abs: float,
    name: str,
) -> float:
    """画素が十分に違うことを要求する（オン/オフのペアワイズ）。"""
    mean_abs = png_mean_abs(path_a, path_b)
    if mean_abs >= min_mean_abs:
        return mean_abs
    DIFFS_DIR.mkdir(parents=True, exist_ok=True)
    aw, ah, a = _read_png_rgba(path_a)
    _, _, b = _read_png_rgba(path_b)
    diff_img = bytearray(len(a))
    for i in range(0, len(a), 4):
        for c in range(3):
            d = abs(a[i + c] - b[i + c])
            diff_img[i + c] = min(255, d * 8) if c == 0 else 0
        diff_img[i + 3] = 255
    diff_path = DIFFS_DIR / f"diff_{name}.png"
    _write_png_rgba(diff_path, aw, ah, bytes(diff_img))
    raise AssertionError(
        f"{name}: mean_abs={mean_abs:.3f} (need >= {min_mean_abs})\n"
        f"pair too similar; spot shadow / tonemap / metal may not be reaching pixels\n"
        f"diff saved: {diff_path}"
    )


def assert_pngs_similar(
    path_a: Path,
    path_b: Path,
    *,
    max_mean_abs: float,
    name: str,
) -> float:
    """画素が十分に近いことを要求する（這い：同じスナップ格子の 2 視点）。"""
    mean_abs = png_mean_abs(path_a, path_b)
    if mean_abs <= max_mean_abs:
        return mean_abs
    DIFFS_DIR.mkdir(parents=True, exist_ok=True)
    aw, ah, a = _read_png_rgba(path_a)
    _, _, b = _read_png_rgba(path_b)
    diff_img = bytearray(len(a))
    for i in range(0, len(a), 4):
        for c in range(3):
            d = abs(a[i + c] - b[i + c])
            diff_img[i + c] = min(255, d * 8) if c == 0 else 0
        diff_img[i + 3] = 255
    diff_path = DIFFS_DIR / f"diff_{name}.png"
    _write_png_rgba(diff_path, aw, ah, bytes(diff_img))
    raise AssertionError(
        f"{name}: mean_abs={mean_abs:.3f} (need <= {max_mean_abs})\n"
        f"pair too different; near-cascade snap may have crawled\n"
        f"diff saved: {diff_path}"
    )
