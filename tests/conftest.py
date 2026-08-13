"""kagra_core なしで kagra サブモジュールをロードするためのヘルパ。

`import kagra` は Rust 拡張を必須にするため、純粋ロジックの単体テストでは
パッケージの __init__.py を踏まずにサブモジュールを直接読み込む。
"""
from __future__ import annotations

import importlib
import importlib.util
import sys
import types
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
KAGRA_DIR = ROOT / "kagra"


def load_kagra_submodule(name: str):
    """`kagra.<name>` を __init__.py 経由なしでロードする。

    Rust 拡張が入っている環境では本物の `kagra` を優先する。スタブを
    `sys.modules` に残すと、後続テストの `import kagra` を壊すため。
    """
    pkg = "kagra"
    if pkg not in sys.modules:
        try:
            importlib.import_module(pkg)
        except Exception:
            stub = types.ModuleType(pkg)
            stub.__path__ = [str(KAGRA_DIR)]
            sys.modules[pkg] = stub

    full = f"kagra.{name}"
    if full in sys.modules and getattr(sys.modules[full], "__file__", None):
        return sys.modules[full]

    path = KAGRA_DIR / f"{name}.py"
    if not path.exists():
        raise FileNotFoundError(path)

    spec = importlib.util.spec_from_file_location(full, path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    sys.modules[full] = mod
    spec.loader.exec_module(mod)
    return mod


@pytest.fixture(scope="session")
def color_utils():
    return load_kagra_submodule("color_utils")


@pytest.fixture(scope="session")
def event_bus_mod():
    return load_kagra_submodule("event_bus")


@pytest.fixture(scope="session")
def entity_mod():
    return load_kagra_submodule("entity")


@pytest.fixture(scope="session")
def physics_mod(entity_mod):
    # physics は `from kagra.entity import Component` するため entity を先に登録
    return load_kagra_submodule("physics")
