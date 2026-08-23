"""チェックアウト直下の kagra/ シャドウ検出。GPU 不要。"""
from __future__ import annotations

from pathlib import Path

from tests.conftest import load_kagra_submodule

launch = load_kagra_submodule("launch")


def test_no_hint_outside_checkout(tmp_path: Path):
    assert launch.checkout_kagra_dir(tmp_path) is None
    assert launch.shadow_hint(cwd=tmp_path) is None


def test_hint_when_local_package_has_no_core(tmp_path: Path):
    pkg = tmp_path / "kagra"
    pkg.mkdir()
    (pkg / "__init__.py").write_text("# stub\n", encoding="utf-8")
    msg = launch.shadow_hint(cwd=tmp_path, loaded_from=pkg, has_core=False)
    assert msg is not None
    assert "maturin develop" in msg
    assert "python -m kagra" in msg


def test_no_hint_when_extension_present(tmp_path: Path):
    pkg = tmp_path / "kagra"
    pkg.mkdir()
    (pkg / "__init__.py").write_text("", encoding="utf-8")
    (pkg / "kagra_core.so").write_bytes(b"x")
    assert launch.extension_present(pkg) is True
    assert launch.shadow_hint(cwd=tmp_path, loaded_from=pkg) is None


def test_import_error_includes_shadow(tmp_path: Path):
    pkg = tmp_path / "kagra"
    pkg.mkdir()
    (pkg / "__init__.py").write_text("", encoding="utf-8")
    text = launch.format_core_import_error(
        RuntimeError("boom"),
        cwd=tmp_path,
        loaded_from=pkg,
    )
    assert "kagra_core が見つかりません" in text
    assert "maturin develop" in text
    assert "boom" in text
