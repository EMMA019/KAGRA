#!/usr/bin/env python3
"""Fail unless Android native libraries are 16 KB page aligned.

For each ``.so`` in an APK (or a loose ``.so`` / directory):

* ``llvm-readelf -l`` — every LOAD ``p_align`` must be >= 16384
* ``zipalign -c -P 16 -v 4`` — the APK stores those libraries uncompressed
  on a 16 KB zip boundary (skipped with ``--elf-only``)

NDK r28+ and build-tools 35+ supply the tools. ``ANDROID_NDK_HOME`` and
``ANDROID_HOME`` select them; otherwise the pin in
``scripts/setup_android_sdk.sh`` is used.
"""
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PAGE_ALIGN = 16384
_NDK_PIN = re.compile(r'^NDK_VERSION="(\d+\.\d+\.\d+)"', re.M)


def pinned_ndk_version() -> str:
    text = (ROOT / "scripts" / "setup_android_sdk.sh").read_text(encoding="utf-8")
    match = _NDK_PIN.search(text)
    if not match:
        raise SystemExit("NDK_VERSION pin missing from scripts/setup_android_sdk.sh")
    return match.group(1)


def load_aligns_from_readelf(text: str) -> list[int]:
    """Parse LOAD p_align values from ``llvm-readelf -l`` output."""
    aligns: list[int] = []
    for raw in text.splitlines():
        parts = raw.split()
        if not parts or parts[0] != "LOAD":
            continue
        token = parts[-1]
        try:
            aligns.append(int(token, 0))
        except ValueError as exc:
            raise ValueError(f"LOAD alignment is not an integer: {raw.strip()}") from exc
    return aligns


def alignment_errors(label: str, aligns: list[int]) -> list[str]:
    if not aligns:
        return [f"{label}: no LOAD segments in llvm-readelf -l output"]
    errors = []
    for align in aligns:
        if align < PAGE_ALIGN:
            errors.append(
                f"{label}: LOAD p_align {align} ({align:#x}) < {PAGE_ALIGN}"
            )
    return errors


def _sdk_root() -> Path | None:
    for key in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        raw = os.environ.get(key, "").strip()
        if raw and Path(raw).is_dir():
            return Path(raw)
    return None


def find_ndk() -> Path:
    for key in ("ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"):
        raw = os.environ.get(key, "").strip()
        if raw and Path(raw).is_dir():
            return Path(raw)
    sdk = _sdk_root()
    if sdk is None:
        raise SystemExit(
            "Android NDK not found. Set ANDROID_NDK_HOME or ANDROID_HOME."
        )
    ndk_root = sdk / "ndk"
    pin = ndk_root / pinned_ndk_version()
    if pin.is_dir():
        return pin
    if ndk_root.is_dir():
        versions = sorted(path for path in ndk_root.iterdir() if path.is_dir())
        if versions:
            return versions[-1]
    bundle = sdk / "ndk-bundle"
    if bundle.is_dir():
        return bundle
    raise SystemExit(f"Android NDK not found under {ndk_root}")


def find_readelf(ndk: Path) -> Path:
    prebuilt = ndk / "toolchains" / "llvm" / "prebuilt"
    found = sorted(prebuilt.glob("*/bin/llvm-readelf*"))
    found = [path for path in found if path.is_file() and not path.name.endswith(".real")]
    if not found:
        raise SystemExit(f"llvm-readelf not found under {prebuilt}")
    # Prefer the host that matches this machine when several prebuilts exist.
    host_needles = []
    if sys.platform.startswith("linux"):
        host_needles.append("linux-x86_64")
    elif sys.platform == "darwin":
        host_needles.extend(("darwin-arm64", "darwin-x86_64"))
    elif os.name == "nt":
        host_needles.append("windows-x86_64")
    for needle in host_needles:
        for path in found:
            if needle in path.parts:
                return path
    return found[0]


def _version_key(name: str) -> tuple[int, ...]:
    parts: list[int] = []
    for piece in name.split("."):
        if piece.isdigit():
            parts.append(int(piece))
        else:
            break
    return tuple(parts)


def find_zipalign() -> Path:
    sdk = _sdk_root()
    if sdk is None:
        found = shutil.which("zipalign")
        if found:
            return Path(found)
        raise SystemExit("zipalign not found. Set ANDROID_HOME (build-tools 35+).")
    tools = sdk / "build-tools"
    candidates: list[tuple[tuple[int, ...], Path]] = []
    if tools.is_dir():
        exe_name = "zipalign.exe" if os.name == "nt" else "zipalign"
        for directory in tools.iterdir():
            exe = directory / exe_name
            if exe.is_file():
                candidates.append((_version_key(directory.name), exe))
    candidates.sort(key=lambda item: item[0], reverse=True)
    for key, exe in candidates:
        if key and key[0] >= 35:
            return exe
    found = shutil.which("zipalign")
    if found:
        return Path(found)
    if candidates:
        raise SystemExit(
            "zipalign -P 16 needs build-tools 35 or newer; "
            f"found {candidates[0][1].parent.name}"
        )
    raise SystemExit(f"zipalign not found under {tools}")


def _shared_objects(root: Path) -> list[Path]:
    if root.is_file() and root.suffix == ".so":
        return [root]
    if root.is_dir():
        return sorted(path for path in root.rglob("*.so") if path.is_file())
    return []


def _extract_apk_libs(apk: Path, dest: Path) -> list[Path]:
    extracted: list[Path] = []
    dest_root = dest.resolve()
    with zipfile.ZipFile(apk) as archive:
        for info in archive.infolist():
            name = info.filename
            if not name.startswith("lib/") or not name.endswith(".so"):
                continue
            if name.startswith("/") or ".." in Path(name).parts:
                raise SystemExit(f"{apk}: unsafe zip entry {name}")
            target = (dest / name).resolve()
            if dest_root != target and dest_root not in target.parents:
                raise SystemExit(f"{apk}: unsafe zip entry {name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(info) as src, target.open("wb") as out:
                shutil.copyfileobj(src, out)
            extracted.append(target)
    return sorted(extracted)


def check_elf(path: Path, readelf: Path, label: str | None = None) -> list[str]:
    label = label or str(path)
    cmd = [str(readelf), "-l", "--wide", str(path)]
    print("+", " ".join(cmd), flush=True)
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout).strip()
        return [f"{label}: llvm-readelf -l failed ({proc.returncode}): {detail}"]
    try:
        aligns = load_aligns_from_readelf(proc.stdout)
    except ValueError as exc:
        return [f"{label}: {exc}"]
    errors = alignment_errors(label, aligns)
    if not errors:
        shown = ", ".join(f"{align:#x}" for align in aligns)
        print(f"{label}: LOAD p_align {shown}")
    return errors


def check_zipalign(apk: Path, zipalign: Path) -> list[str]:
    cmd = [str(zipalign), "-c", "-P", "16", "-v", "4", str(apk)]
    print("+", " ".join(cmd), flush=True)
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
    if proc.stdout:
        sys.stdout.write(proc.stdout)
        if not proc.stdout.endswith("\n"):
            sys.stdout.write("\n")
    if proc.stderr:
        sys.stderr.write(proc.stderr)
        if not proc.stderr.endswith("\n"):
            sys.stderr.write("\n")
    if proc.returncode != 0 or "FAILED" in proc.stdout:
        return [f"{apk}: zipalign -c -P 16 -v 4 failed ({proc.returncode})"]
    return []


def check_paths(paths: list[Path], *, elf_only: bool) -> list[str]:
    readelf = find_readelf(find_ndk())
    zipalign = None if elf_only else find_zipalign()
    errors: list[str] = []
    saw_so = False
    for path in paths:
        if not path.exists():
            errors.append(f"{path}: not found")
            continue
        if path.suffix == ".apk":
            if zipalign is not None:
                errors.extend(check_zipalign(path, zipalign))
            with tempfile.TemporaryDirectory(prefix="kagra-16k-") as raw_tmp:
                tmp = Path(raw_tmp)
                libs = _extract_apk_libs(path, tmp)
                if not libs:
                    errors.append(f"{path}: APK contains no lib/*/*.so")
                for lib in libs:
                    saw_so = True
                    rel = lib.relative_to(tmp).as_posix()
                    errors.extend(check_elf(lib, readelf, label=f"{path}!/{rel}"))
            continue
        libs = _shared_objects(path)
        if not libs:
            errors.append(f"{path}: no .so files")
            continue
        for lib in libs:
            saw_so = True
            errors.extend(check_elf(lib, readelf))
    if not saw_so and not errors:
        errors.append("no .so files to check")
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check Android 16 KB page alignment")
    parser.add_argument("paths", nargs="+", help="APK, .so, or directory")
    parser.add_argument(
        "--elf-only",
        action="store_true",
        help="check LOAD p_align only; do not run zipalign",
    )
    args = parser.parse_args(argv)
    errors = check_paths([Path(path) for path in args.paths], elf_only=args.elf_only)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(f"16 KB alignment OK ({PAGE_ALIGN})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
