"""Python → Rust Engine の API 整合性テスト。

``kagra/*.py`` が ``_engine.foo(...)`` / ``engine.foo(...)`` で呼ぶメソッドが、
``kagra-core/src/engine/mod.rs`` の ``#[pymethods]`` に存在するかを静的に検査する。
GPU 不要。FBX / set_fog のような「片方にしか無い」事故を防ぐ。
"""
from __future__ import annotations

import ast
import re
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
ENGINE_RS = ROOT / "kagra-core" / "src" / "engine" / "mod.rs"
KAGRA_PY = ROOT / "kagra"


def _rust_pymethods() -> set[str]:
    text = ENGINE_RS.read_text(encoding="utf-8")
    # pymethods impl ブロック内の pub fn を拾う（簡易）
    # ``impl Engine {`` の後の ``pub fn name`` を列挙
    names = set(re.findall(r"(?m)^\s*pub fn ([A-Za-z_][A-Za-z0-9_]*)\s*\(", text))
    # コンストラクタ等は除外しない（呼び出し側も new を使う）
    return names


def _python_engine_calls() -> set[str]:
    """AST で ``_engine.X`` / ``engine.X`` / ``get_engine().X`` を収集。"""
    found: set[str] = set()
    for path in KAGRA_PY.rglob("*.py"):
        if "__pycache__" in path.parts:
            continue
        src = path.read_text(encoding="utf-8")
        try:
            tree = ast.parse(src, filename=str(path))
        except SyntaxError:
            # フォールバック: 単語境界付き（my_tts_engine を誤検知しない）
            for m in re.finditer(
                r"(?<![A-Za-z0-9_])_engine\.([A-Za-z_][A-Za-z0-9_]*)\s*\(", src
            ):
                found.add(m.group(1))
            continue

        class Visitor(ast.NodeVisitor):
            def visit_Call(self, node: ast.Call):
                func = node.func
                if isinstance(func, ast.Attribute) and isinstance(func.value, ast.Name):
                    # グローバル _engine のみ（bare engine はローカル変数が多いので除外）
                    if func.value.id == "_engine":
                        found.add(func.attr)
                if isinstance(func, ast.Attribute) and isinstance(func.value, ast.Call):
                    # get_engine().foo()
                    if (
                        isinstance(func.value.func, ast.Name)
                        and func.value.func.id == "get_engine"
                    ):
                        found.add(func.attr)
                if isinstance(func, ast.Attribute) and isinstance(func.value, ast.Attribute):
                    # kagra._engine.foo / self._engine.foo
                    if func.value.attr == "_engine":
                        found.add(func.attr)
                self.generic_visit(node)

        Visitor().visit(tree)

        # フォールバック（単語境界付き）
        for m in re.finditer(
            r"(?<![A-Za-z0-9_])(?:_engine|get_engine\(\))\.([A-Za-z_][A-Za-z0-9_]*)\s*\(",
            src,
        ):
            found.add(m.group(1))

    return found


# 既知の「Rust に無くてよい」例外（プロパティ）
_ALLOW_MISSING = {
    "fps",  # Engine の getter プロパティ
}


def test_engine_methods_exist_in_rust():
    rust = _rust_pymethods()
    py = _python_engine_calls()
    missing = sorted(py - rust - _ALLOW_MISSING)
    assert not missing, (
        "Python が呼ぶ Engine メソッドが Rust にありません（FBX/set_fog 事故パターン）:\n  - "
        + "\n  - ".join(missing)
        + "\n\nRust 側にある例: "
        + ", ".join(sorted(list(rust))[:12])
        + " ..."
    )


def test_critical_bindings_present():
    """エージェント検証・VRM 系の必須バインディング。"""
    rust = _rust_pymethods()
    required = [
        "load_vrm",
        "draw_vrm",
        "load_fbx_anim",
        "set_fog",
        "set_light_dir",
        "set_rim",
        "set_toon_params",
        "set_shadow_enabled",
        "set_point_light",
        "set_hdri",
        "set_mesh_pbr",
        "request_screenshot",
        "set_grab_frames",
        "grab_frame",
        "inject_key_down",
        "get_vrm_look_at",
        "load_gltf",
        "draw_gltf",
        "unload_gltf",
        "upload_mesh_3d",
        "draw_mesh_id",
        "unload_mesh_3d",
        "vrm_spring_info",
        "step_vrm_spring",
        "set_vrm_pose",
    ]
    missing = [n for n in required if n not in rust]
    assert not missing, f"必須バインディング欠落: {missing}"


def test_set_fog_python_wrapper_exists():
    """set_fog の Python ラッパが公開され、Rust 側へ転送している。

    拡張のビルドを要求しないよう AST で検査する（pure-python CI で回る）。
    """
    src = (KAGRA_PY / "__init__.py").read_text(encoding="utf-8")
    tree = ast.parse(src)
    fn = next(
        (
            n
            for n in tree.body
            if isinstance(n, ast.FunctionDef) and n.name == "set_fog"
        ),
        None,
    )
    assert fn is not None, "kagra.set_fog が未公開"
    assert "_engine.set_fog" in ast.unparse(fn), "set_fog が Rust 側へ転送していない"


def test_gltf_python_wrappers_exist():
    src = (KAGRA_PY / "__init__.py").read_text(encoding="utf-8")
    tree = ast.parse(src)
    names = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    for name, rust in (
        ("load_gltf", "_engine.load_gltf"),
        ("draw_gltf", "_engine.draw_gltf"),
        ("unload_gltf", "_engine.unload_gltf"),
        ("upload_mesh_3d", "_engine.upload_mesh_3d"),
        ("draw_mesh_id", "_engine.draw_mesh_id"),
        ("unload_mesh_3d", "_engine.unload_mesh_3d"),
        ("stage", "Stage.load"),
        ("set_grab_frames", "_engine.set_grab_frames"),
        ("grab_frame", "_engine.grab_frame"),
        ("set_point_light", "_engine.set_point_light"),
        ("set_hdri", "_engine.set_hdri"),
        ("set_mesh_pbr", "_engine.set_mesh_pbr"),
        ("set_rim", "_engine.set_rim"),
    ):
        assert name in names, f"kagra.{name} が未公開"
        assert rust in ast.unparse(names[name]), f"{name} が {rust} を呼んでいない"
