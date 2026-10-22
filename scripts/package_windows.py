#!/usr/bin/env python3
"""ジャンル例を Windows 用のフォルダ / zip（.exe 入り）にする。

    python scripts/package_windows.py --game examples/platformer_minimal.py

Windows では PyInstaller の onedir を作り、zip を ``dist/windows/`` に出す。
それ以外の OS は ``--stage-only`` でツリーだけ確認できる。exe 自体は
GitHub Actions の ``windows-latest``（``.github/workflows/package-windows.yml``）
が作る。

事前に ``maturin build`` した wheel を入れておくこと。``import kagra`` が
``kagra.kagra_shared`` を解決できる必要がある。
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import sysconfig
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from kagra.distribute import (  # noqa: E402
    ManifestError,
    manifest_for_game,
    png_to_ico,
    stage_game,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Package a KAGRA example for Windows")
    parser.add_argument("--game", required=True, help="examples/foo_minimal.py or games/foo.json")
    parser.add_argument("--out", default="dist/windows", help="output directory")
    parser.add_argument(
        "--stage-only",
        action="store_true",
        help="write the game tree and skip PyInstaller (non-Windows, or a dry layout)",
    )
    args = parser.parse_args(argv)
    repo = ROOT
    try:
        manifest = manifest_for_game(repo, args.game)
    except ManifestError as exc:
        print(f"manifest: {exc}", file=sys.stderr)
        return 2

    out = Path(args.out)
    if not out.is_absolute():
        out = repo / out
    stage = out / f"{manifest.slug}-stage"
    if stage.exists():
        shutil.rmtree(stage)
    rels = stage_game(repo, manifest, stage)
    readme = _readme(manifest)
    (stage / "README.txt").write_text(readme, encoding="utf-8")
    print(f"staged {len(rels)} files -> {stage}")

    if args.stage_only or sys.platform != "win32":
        if sys.platform != "win32" and not args.stage_only:
            print(
                "PyInstaller exe is built on Windows. Re-run there, or pass --stage-only. "
                "CI: .github/workflows/package-windows.yml",
                file=sys.stderr,
            )
            return 2
        print("stage-only: no .exe")
        return 0

    # このスクリプトはリポジトリを sys.path の先頭に入れる。そこにある kagra/
    # は通常パッケージなので、site-packages の kagra.kagra_shared を隠す。
    # 拡張の有無はリポジトリの外で調べる。
    native_error = _native_module_error()
    if native_error:
        print(
            "native module missing "
            f"({native_error}). Install the wheel first:\n"
            "  maturin build --release --out dist/wheel\n"
            "  pip install dist/wheel/*.whl",
            file=sys.stderr,
        )
        return 2

    exe_dir = _pyinstaller(repo, manifest, stage, out)
    (exe_dir / "README.txt").write_text(readme, encoding="utf-8")
    zip_path = out / f"{manifest.slug}-windows.zip"
    _zip_dir(exe_dir, zip_path)
    exe = exe_dir / f"{manifest.slug}.exe"
    if not exe.is_file():
        print(f"expected exe at {exe}", file=sys.stderr)
        return 1
    print(f"exe  {exe}")
    print(f"zip  {zip_path}")
    return 0


def _pyinstaller(repo: Path, manifest, stage: Path, out: Path) -> Path:
    icon_png = (repo / manifest.icon).read_bytes()
    icon_ico = out / f"{manifest.slug}.ico"
    out.mkdir(parents=True, exist_ok=True)
    icon_ico.write_bytes(png_to_ico(icon_png))
    work = out / "pyi-work" / manifest.slug
    spec = out / "pyi-spec"
    if work.exists():
        shutil.rmtree(work)
    dist = out / "pyi-dist"
    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "--noconfirm",
        "--clean",
        "--onedir",
        "--name",
        manifest.slug,
        "--distpath",
        str(dist),
        "--workpath",
        str(work),
        "--specpath",
        str(spec),
        "--icon",
        str(icon_ico),
        "--add-data",
        f"{stage}{os.pathsep}.",
        "--hidden-import",
        "kagra.kagra_shared",
        "--collect-submodules",
        "kagra",
        "--collect-all",
        "kagra",
    ]
    for site in _site_paths():
        cmd.extend(["--paths", site])
    cmd.append(str(repo / "scripts" / "frozen_launch.py"))
    # 作業ディレクトリがリポジトリだと、解析中の import kagra が
    # ソースツリーを掴んで拡張モジュールを落とす。
    py_cwd = out / "pyi-cwd"
    py_cwd.mkdir(parents=True, exist_ok=True)
    print(" ".join(cmd))
    subprocess.run(cmd, check=True, cwd=py_cwd, env=_env_without_repo())
    return dist / manifest.slug


def _native_module_error() -> str | None:
    proc = subprocess.run(
        [sys.executable, "-c", "import kagra.kagra_shared"],
        cwd=_probe_cwd(),
        env=_env_without_repo(),
        capture_output=True,
        text=True,
    )
    if proc.returncode == 0:
        return None
    return (proc.stderr or proc.stdout or "import failed").strip()


def _probe_cwd() -> Path:
    for key in ("TEMP", "TMP", "TMPDIR"):
        raw = os.environ.get(key, "").strip()
        if raw:
            path = Path(raw)
            if path.is_dir():
                return path
    return Path("/tmp")


def _env_without_repo() -> dict[str, str]:
    env = os.environ.copy()
    kept: list[str] = []
    for item in env.get("PYTHONPATH", "").split(os.pathsep):
        if not item:
            continue
        try:
            if Path(item).resolve() == ROOT:
                continue
        except OSError:
            pass
        kept.append(item)
    if kept:
        env["PYTHONPATH"] = os.pathsep.join(kept)
    else:
        env.pop("PYTHONPATH", None)
    return env


def _site_paths() -> list[str]:
    found: list[str] = []
    for key in ("purelib", "platlib"):
        path = sysconfig.get_path(key)
        if path and path not in found:
            found.append(path)
    return found


def _zip_dir(folder: Path, dest: Path) -> None:
    if dest.exists():
        dest.unlink()
    with zipfile.ZipFile(dest, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for path in folder.rglob("*"):
            if path.is_file():
                zf.write(path, arcname=str(Path(folder.name) / path.relative_to(folder)))


def _readme(manifest) -> str:
    return (
        f"{manifest.title}\n"
        f"version {manifest.version}\n"
        f"entry {manifest.entry}\n"
        "\n"
        "Windows: 同じフォルダの .exe を起動する。描画は Vulkan を使う"
        "（このエンジンは dx12 を切ってある）。ドライバの入った PC で試す。\n"
        "操作は例の冒頭コメントに従う。ESC または窓を閉じると終了。\n"
        "Android 版は同じマニフェストから APK を作る。docs/distribution.md。\n"
    )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        print(f"command failed: {exc}", file=sys.stderr)
        raise SystemExit(exc.returncode or 1) from exc
