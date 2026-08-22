"""サンプル VRM の解決と初回ダウンロード。

リポジトリは VRM を同梱しない（サイズとライセンス）。代わりに、
初回デモで再配布可能なテストモデルをキャッシュへ取得する。

既定モデル: Alicia Solid (VRM 0.51) from UniVRM tests.
  © Dwango / ニコニ立体ちゃん
  利用規約: https://3d.nicovideo.jp/alicia/rule.html
"""
from __future__ import annotations

import hashlib
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

from kagra.contracts import (
    AssetKind,
    KagraContractError,
    project_root,
    resolve_asset,
)

# UniVRM がテスト用に公開している Alicia Solid。URL はタグでピン留め。
SAMPLE_NAME = "AliciaSolid"
SAMPLE_FILENAME = "AliciaSolid.vrm"
SAMPLE_URL = (
    "https://raw.githubusercontent.com/vrm-c/UniVRM/"
    "v0.128.1/Tests/Models/Alicia_vrm-0.51/AliciaSolid_vrm-0.51.vrm"
)
SAMPLE_SHA256 = "237bb02efadf8c13a114af91dd8e860173081457dee87017e51011c448d05dc2"
SAMPLE_LICENSE = (
    "Alicia Solid (ニコニ立体ちゃん) © Dwango. "
    "Terms: https://3d.nicovideo.jp/alicia/rule.html — "
    "credit the character when you publish screenshots or videos."
)

_ALIAS_NAMES = frozenset({"emma", "player", "alicia", "sample", SAMPLE_NAME.lower()})


def cache_dir() -> Path:
    """OS ごとのキャッシュディレクトリ。"""
    override = os.environ.get("KAGRA_CACHE_DIR")
    if override:
        return Path(override)
    if sys.platform == "win32":
        base = Path(os.environ.get("LOCALAPPDATA") or (Path.home() / "AppData" / "Local"))
        return base / "kagra" / "samples"
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Caches" / "kagra" / "samples"
    xdg = os.environ.get("XDG_CACHE_HOME")
    base = Path(xdg) if xdg else (Path.home() / ".cache")
    return base / "kagra" / "samples"


def cached_sample_vrm() -> Path | None:
    """ダウンロード済みのサンプルがあればそのパス。"""
    p = cache_dir() / SAMPLE_FILENAME
    return p if p.is_file() else None


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _download(url: str, dest: Path, *, timeout: float = 60.0) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "kagra-engine/0.1"})
        with urllib.request.urlopen(req, timeout=timeout) as resp, tmp.open("wb") as out:
            while True:
                chunk = resp.read(1024 * 1024)
                if not chunk:
                    break
                out.write(chunk)
        digest = _sha256(tmp)
        if digest != SAMPLE_SHA256:
            tmp.unlink(missing_ok=True)
            raise KagraContractError(
                code="SAMPLE_CHECKSUM_MISMATCH",
                message="downloaded VRM failed checksum verification",
                hint="Delete the cache and retry, or place your own .vrm at assets/Emma.vrm",
                path=str(dest),
                candidates=[digest, SAMPLE_SHA256],
            )
        tmp.replace(dest)
    except KagraContractError:
        raise
    except (urllib.error.URLError, OSError, TimeoutError) as e:
        tmp.unlink(missing_ok=True)
        raise KagraContractError(
            code="SAMPLE_DOWNLOAD_FAILED",
            message=f"could not download sample VRM: {e}",
            hint=(
                "Need network once, or place any .vrm at assets/Emma.vrm "
                f"(or set KAGRA_VRM). Source: {SAMPLE_URL}"
            ),
            path=url,
        ) from e


def download_sample_vrm(*, force: bool = False) -> Path:
    """サンプル VRM をキャッシュへ取得してパスを返す。"""
    dest = cache_dir() / SAMPLE_FILENAME
    if dest.is_file() and not force:
        if _sha256(dest) == SAMPLE_SHA256:
            return dest
        dest.unlink(missing_ok=True)
    print(f"[kagra] downloading sample VRM ({SAMPLE_NAME})…", file=sys.stderr)
    print(f"[kagra] {SAMPLE_LICENSE}", file=sys.stderr)
    _download(SAMPLE_URL, dest)
    print(f"[kagra] saved {dest}", file=sys.stderr)
    return dest


def ensure_vrm(
    name: str = "Emma",
    *,
    download: bool = True,
    root: Path | None = None,
) -> Path:
    """VRM を解決する。無ければ（download=True なら）サンプルを取得する。

    探索順:
      1. 環境変数 ``KAGRA_VRM``
      2. contracts（assets/Emma.vrm 等のエイリアス）
      3. キャッシュ済みサンプル
      4. 初回ダウンロード（``download=True``）

    Example::
        av = kagra.avatar(str(ensure_vrm()))
    """
    env = os.environ.get("KAGRA_VRM")
    if env:
        p = Path(env)
        if p.is_file():
            return p.resolve()
        raise KagraContractError(
            code="ASSET_NOT_FOUND",
            message=f"KAGRA_VRM does not point to a file: {env}",
            hint="Unset KAGRA_VRM or point it at a .vrm file",
            path=env,
        )

    found = resolve_asset(AssetKind.VRM, name, root=root or project_root(), required=False)
    if found is not None:
        return found

    key = Path(name).stem.lower()
    if key in _ALIAS_NAMES:
        cached = cached_sample_vrm()
        if cached is not None:
            return cached
        if download:
            return download_sample_vrm()

    raise KagraContractError(
        code="ASSET_NOT_FOUND",
        message=f"vrm asset not found: {name}",
        hint=(
            "Place any .vrm at assets/Emma.vrm, or run `python -m kagra.demo` "
            "to download a sample (Alicia Solid) and play. "
            "Your own model: kagra.avatar('/path/to/model.vrm')"
        ),
        path=name,
    )
