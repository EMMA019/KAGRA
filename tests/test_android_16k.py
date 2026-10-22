"""16 KB page alignment for the Android APK. No NDK and no device."""
from __future__ import annotations

import importlib.util
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _checker():
    path = ROOT / "scripts" / "check_android_16k.py"
    spec = importlib.util.spec_from_file_location("check_android_16k", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_load_align_parses_readelf_flags():
    text = """
  Type           Offset   VirtAddr           PhysAddr           FileSiz  MemSiz   Flg Align
  PHDR           0x000040 0x0000000000000040 0x0000000000000040 0x000188 0x000188 R   0x8
  LOAD           0x000000 0x0000000000000000 0x0000000000000000 0x001000 0x001000 R E 0x4000
  LOAD           0x002000 0x0000000000002000 0x0000000000002000 0x001000 0x001000 RW  0x10000
  GNU_RELRO      0x002000 0x0000000000002000 0x0000000000002000 0x001000 0x001000 R   0x1
"""
    mod = _checker()
    assert mod.load_aligns_from_readelf(text) == [0x4000, 0x10000]
    assert mod.alignment_errors("libkagra_shared.so", [0x4000, 0x10000]) == []


def test_4kb_load_segment_is_rejected():
    mod = _checker()
    aligns = mod.load_aligns_from_readelf("  LOAD 0x0 0x0 0x0 0x10 0x10 R E 0x1000\n")
    errors = mod.alignment_errors("lib/arm64-v8a/libkagra_jni.so", aligns)
    assert len(errors) == 1
    assert "16384" in errors[0]
    assert "4096" in errors[0]


def test_missing_load_segment_is_rejected():
    mod = _checker()
    errors = mod.alignment_errors("libc++_shared.so", [])
    assert errors
    assert "no LOAD" in errors[0]


def test_ndk_pin_is_r28_or_newer_and_shared():
    setup = (ROOT / "scripts" / "setup_android_sdk.sh").read_text(encoding="utf-8")
    match = re.search(r'^NDK_VERSION="(\d+)\.(\d+)\.(\d+)"', setup, re.M)
    assert match, "NDK_VERSION pin missing"
    assert int(match.group(1)) >= 28
    version = ".".join(match.groups())
    gradle = (ROOT / "mobile/android/app/build.gradle.kts").read_text(encoding="utf-8")
    assert version in gradle
    assert "useLegacyPackaging = false" in gradle
    assert "c++_shared" in gradle
    assert version in (ROOT / "docs" / "distribution.md").read_text(encoding="utf-8")
    for rel in (
        ".github/workflows/ci.yml",
        ".github/workflows/package-android.yml",
    ):
        text = (ROOT / rel).read_text(encoding="utf-8")
        assert "26.1.10909125" not in text
        assert "KAGRA_ANDROID_NDK_VERSION" in text


def test_cargo_android_targets_pass_page_size_flags():
    text = (ROOT / ".cargo" / "config.toml").read_text(encoding="utf-8")
    for target in ("aarch64-linux-android", "x86_64-linux-android"):
        assert f"[target.{target}]" in text
    assert text.count("link-arg=-Wl,-z,max-page-size=16384") >= 2
    assert text.count("link-arg=-Wl,-z,common-page-size=16384") >= 2
    assert text.count("link-arg=-Wl,-soname,libkagra_shared.so") >= 2


def test_package_android_workflow_checks_load_and_zipalign():
    workflow = (ROOT / ".github/workflows/package-android.yml").read_text(encoding="utf-8")
    assert "scripts/check_android_16k.py" in workflow
    assert "DT_NEEDED" in workflow
    script = (ROOT / "scripts" / "check_android_16k.py").read_text(encoding="utf-8")
    assert "llvm-readelf" in script
    assert '"-d"' in script
    assert '"-c", "-P", "16", "-v", "4"' in script
    cmake = (ROOT / "mobile/android/app/src/main/cpp/CMakeLists.txt").read_text(encoding="utf-8")
    assert "-Wl,-z,max-page-size=16384" in cmake
    assert "-Wl,-z,common-page-size=16384" in cmake
    assert 'IMPORTED_SONAME "libkagra_shared.so"' in cmake
    assert "IMPORTED_NO_SONAME FALSE" in cmake
    assert "-Wl,-l:libkagra_shared.so" in cmake
    assert "target_link_libraries(kagra_jni kagra_shared" not in cmake
    build_rs = (ROOT / "kagra-shared" / "build.rs").read_text(encoding="utf-8")
    assert 'target.contains("android")' in build_rs
    assert "cargo:rustc-cdylib-link-arg=-Wl,-soname,libkagra_shared.so" in build_rs
    native = (ROOT / "mobile/android/app/src/main/java/dev/kagra/shell/KagraNative.kt").read_text(
        encoding="utf-8"
    )
    assert "loadError" in native
    activity = (ROOT / "mobile/android/app/src/main/java/dev/kagra/shell/MainActivity.kt").read_text(
        encoding="utf-8"
    )
    assert "KagraNative.loadError" in activity


_READELF_D = """
Dynamic section at offset 0x2a8 contains 6 entries:
  Tag                Type                 Name/Value
  0x0000000000000001 (NEEDED)             Shared library: [liblog.so]
  0x0000000000000001 (NEEDED)             Shared library: [libc++_shared.so]
  0x0000000000000001 (NEEDED)             Shared library: [libc.so]
  0x000000000000000e (SONAME)             Library soname: [libkagra_shared.so]
"""

_READELF_D_PATH = """
  0x0000000000000001 (NEEDED)             Shared library: [liblog.so]
  0x0000000000000001 (NEEDED)             Shared library: [/home/runner/work/KAGRA/KAGRA/mobile/android/app/src/main/cpp/../jniLibs/arm64-v8a/libkagra_shared.so]
  0x000000000000000e (SONAME)             Library soname: [libkagra_jni.so]
"""


def test_needed_without_path_is_accepted():
    mod = _checker()
    assert mod.needed_entries(_READELF_D) == ["liblog.so", "libc++_shared.so", "libc.so"]
    assert mod.dynamic_errors("libkagra_shared.so", _READELF_D, filename="libkagra_shared.so") == []


def test_needed_absolute_path_is_rejected():
    mod = _checker()
    errors = mod.dynamic_errors(
        "lib/arm64-v8a/libkagra_jni.so",
        _READELF_D_PATH,
        filename="libkagra_jni.so",
    )
    assert len(errors) == 1
    assert "/home/runner/work/KAGRA/KAGRA/" in errors[0]
    assert "DT_NEEDED" in errors[0]


def test_shared_library_without_soname_is_rejected():
    mod = _checker()
    text = "  0x0000000000000001 (NEEDED)             Shared library: [libc.so]\n"
    errors = mod.dynamic_errors("libkagra_shared.so", text, filename="libkagra_shared.so")
    assert len(errors) == 1
    assert "SONAME" in errors[0]


def _elf64(needed: list[str], soname: str | None) -> bytes:
    """Tiny identity-mapped ELF64 whose dynamic strings are exactly `needed`."""
    import struct

    strings = b"\x00"
    offsets: list[int] = []
    for name in needed:
        offsets.append(len(strings))
        strings += name.encode() + b"\x00"
    soname_off = None
    if soname is not None:
        soname_off = len(strings)
        strings += soname.encode() + b"\x00"
    # ELF header + 2 program headers, then dynamic, then strings.
    phoff = 64
    dyn_off = phoff + 56 * 2
    # DT_NEEDED * n + optional DT_SONAME + DT_STRTAB + DT_NULL
    dyn_count = len(needed) + (1 if soname_off is not None else 0) + 2
    str_off = dyn_off + dyn_count * 16
    blob = bytearray(str_off + len(strings))
    blob[0:4] = b"\x7fELF"
    blob[4] = 2  # ELFCLASS64
    blob[5] = 1  # ELFDATA2LSB
    blob[6] = 1
    struct.pack_into("<HHIQQQIHHHHHH", blob, 16, 3, 183, 1, 0, phoff, 0, 0, 64, 56, 2, 0, 0, 0)
    filesz = len(blob)

    def phdr(index: int, p_type: int, flags: int, offset: int, filesz_ph: int, align: int) -> None:
        struct.pack_into(
            "<IIQQQQQQ",
            blob,
            phoff + index * 56,
            p_type,
            flags,
            offset,
            offset,
            offset,
            filesz_ph,
            filesz_ph,
            align,
        )

    phdr(0, 1, 5, 0, filesz, 0x4000)  # PT_LOAD
    phdr(1, 2, 6, dyn_off, dyn_count * 16, 8)  # PT_DYNAMIC
    cursor = dyn_off
    for off in offsets:
        struct.pack_into("<qQ", blob, cursor, 1, off)  # DT_NEEDED
        cursor += 16
    if soname_off is not None:
        struct.pack_into("<qQ", blob, cursor, 14, soname_off)  # DT_SONAME
        cursor += 16
    struct.pack_into("<qQ", blob, cursor, 5, str_off)  # DT_STRTAB
    cursor += 16
    struct.pack_into("<qQ", blob, cursor, 0, 0)  # DT_NULL
    blob[str_off:] = strings
    return bytes(blob)


def test_elf_needed_path_is_rejected_without_readelf():
    mod = _checker()
    bad = "/home/runner/work/KAGRA/KAGRA/mobile/android/app/src/main/cpp/../jniLibs/arm64-v8a/libkagra_shared.so"
    soname, needed = mod.elf_dynamic(_elf64([bad, "liblog.so"], "libkagra_jni.so"))
    assert soname == "libkagra_jni.so"
    assert bad in needed
    errors = mod._dynamic_string_errors("lib/arm64-v8a/libkagra_jni.so", "libkagra_jni.so", soname, needed)
    assert any(bad in error and "DT_NEEDED" in error for error in errors)


def test_elf_bare_needed_and_shared_soname_are_accepted():
    mod = _checker()
    soname, needed = mod.elf_dynamic(
        _elf64(["libkagra_shared.so", "liblog.so", "libc.so"], "libkagra_shared.so")
    )
    assert needed[0] == "libkagra_shared.so"
    assert mod._dynamic_string_errors("libkagra_shared.so", "libkagra_shared.so", soname, needed) == []


def test_elf_shared_without_soname_is_rejected():
    mod = _checker()
    soname, needed = mod.elf_dynamic(_elf64(["libc.so"], None))
    errors = mod._dynamic_string_errors("libkagra_shared.so", "libkagra_shared.so", soname, needed)
    assert len(errors) == 1
    assert "SONAME" in errors[0]


def test_debug_keystore_is_pinned():
    keystore = ROOT / "mobile/android/app/debug.keystore"
    assert keystore.is_file()
    assert keystore.stat().st_size > 0
    gradle = (ROOT / "mobile/android/app/build.gradle.kts").read_text(encoding="utf-8")
    assert "androiddebugkey" in gradle
    assert 'storePassword = "android"' in gradle
    assert 'keyPassword = "android"' in gradle
    assert 'storeFile = file("debug.keystore")' in gradle
