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


def test_package_android_workflow_checks_load_and_zipalign():
    workflow = (ROOT / ".github/workflows/package-android.yml").read_text(encoding="utf-8")
    assert "scripts/check_android_16k.py" in workflow
    script = (ROOT / "scripts" / "check_android_16k.py").read_text(encoding="utf-8")
    assert "llvm-readelf" in script
    assert '"-c", "-P", "16", "-v", "4"' in script
    cmake = (ROOT / "mobile/android/app/src/main/cpp/CMakeLists.txt").read_text(encoding="utf-8")
    assert "-Wl,-z,max-page-size=16384" in cmake
    assert "-Wl,-z,common-page-size=16384" in cmake
    native = (ROOT / "mobile/android/app/src/main/java/dev/kagra/shell/KagraNative.kt").read_text(
        encoding="utf-8"
    )
    assert "loadError" in native
    activity = (ROOT / "mobile/android/app/src/main/java/dev/kagra/shell/MainActivity.kt").read_text(
        encoding="utf-8"
    )
    assert "KagraNative.loadError" in activity
