"""アセット／API 契約（エージェントが推測しないための公式解決）。

Example::
    from kagra.contracts import resolve_asset, AssetKind, KagraContractError

    path = resolve_asset(AssetKind.VRM, "Emma")
    # → assets/Emma.vrm など候補を順に探す

エラーは code + hint 付き。エージェントが機械的に直せる形を優先する。
"""
from __future__ import annotations

import json
import os
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[1]


class AssetKind(str, Enum):
    VRM = "vrm"
    FBX = "fbx"
    BVH = "bvh"
    VRMA = "vrma"
    GLTF = "gltf"
    TEXTURE = "texture"
    FONT = "font"
    AUDIO = "audio"
    ANY = "any"


@dataclass
class KagraContractError(Exception):
    """契約違反。エージェント向けに構造化。"""

    code: str
    message: str
    hint: str = ""
    path: str | None = None
    candidates: list[str] = field(default_factory=list)

    def __str__(self) -> str:
        parts = [f"[{self.code}] {self.message}"]
        if self.path:
            parts.append(f"path={self.path}")
        if self.candidates:
            parts.append("tried=" + ", ".join(self.candidates[:8]))
        if self.hint:
            parts.append(f"hint={self.hint}")
        return " | ".join(parts)

    def to_dict(self) -> dict:
        return {
            "code": self.code,
            "message": self.message,
            "hint": self.hint,
            "path": self.path,
            "candidates": self.candidates,
        }


# 公式候補ルート（先頭ほど優先）
_SEARCH_ROOTS = (
    "assets",
    "assets/model",
    "assets/models",
    "assets/anim",
    "assets/motion",
    "assets/stage",
    "assets/env",
    "assets/scenes",
    "tests/fixtures",
)

_EXTENSIONS: dict[AssetKind, tuple[str, ...]] = {
    AssetKind.VRM: (".vrm",),
    AssetKind.FBX: (".fbx",),
    AssetKind.BVH: (".bvh",),
    AssetKind.VRMA: (".vrma",),
    AssetKind.GLTF: (".glb", ".gltf"),
    AssetKind.TEXTURE: (".png", ".jpg", ".jpeg", ".webp"),
    AssetKind.FONT: (".ttf", ".ttc", ".otf"),
    AssetKind.AUDIO: (".wav", ".ogg", ".mp3"),
    AssetKind.ANY: (),
}

# よく使う論理名 → 相対パス候補
_ALIASES: dict[str, list[str]] = {
    "emma": ["assets/Emma.vrm", "assets/model/Emma.vrm", "assets/model/player.vrm"],
    "player": ["assets/model/player.vrm", "assets/Emma.vrm", "assets/player.vrm"],
    "walk": [
        "assets/walk.fbx",
        "assets/anim/walk.fbx",
        "tests/fixtures/synthetic_walk.bvh",
    ],
    "walk_fbx": [
        "assets/walk.fbx",
        "assets/anim/walk.fbx",
    ],
    "samba": ["assets/Samba Dancing.fbx"],
    "samba dancing": ["assets/Samba Dancing.fbx"],
    "ymca": ["assets/Ymca Dance.fbx"],
    "ymca dance": ["assets/Ymca Dance.fbx"],
    "catwalk": ["assets/Catwalk Walk.fbx"],
    "catwalk walk": ["assets/Catwalk Walk.fbx"],
    "tough": ["assets/Female Tough Walk.fbx"],
    "female tough walk": ["assets/Female Tough Walk.fbx"],
    "standing": ["assets/Female Standing Pose.fbx"],
    "female standing pose": ["assets/Female Standing Pose.fbx"],
    "pitch": ["assets/pitch.fbx"],
    "dance": [
        "kagra/data/synthetic_dance.bvh",
        "tests/fixtures/synthetic_dance.bvh",
        "assets/dance.bvh",
        "assets/anim/dance.bvh",
        "assets/dance.vrma",
        "assets/anim/dance.vrma",
    ],
    "coolheadbangwalk": ["assets/coolHeadbangWalk.vrma"],
    "cute_song_trial": ["assets/cute_song_trial.wav"],
    "stage": [
        "assets/stage.glb",
        "assets/stage.gltf",
        "assets/env/stage.glb",
        "assets/scenes/stage.glb",
        "assets/stage/stage.glb",
    ],
    "cube": [
        "kagra/data/unit_cube.glb",
        "tests/fixtures/unit_cube.glb",
    ],
}


def project_root(start: Path | None = None) -> Path:
    """kagra パッケージまたは cwd からリポジトリルートを推定。"""
    if start is None:
        start = Path.cwd()
    cur = start.resolve()
    for p in [cur, *cur.parents]:
        if (p / "kagra-core" / "Cargo.toml").exists() or (p / "pyproject.toml").exists():
            if (p / "kagra").is_dir():
                return p
    return ROOT


def _exts_for(kind: AssetKind) -> tuple[str, ...]:
    return _EXTENSIONS.get(kind, ())


def candidate_paths(
    kind: AssetKind,
    name: str,
    *,
    root: Path | None = None,
) -> list[Path]:
    """論理名または相対パスから探索候補を列挙（存在チェックなし）。"""
    root = root or project_root()
    name = name.strip().replace("\\", "/")
    out: list[Path] = []

    # 絶対／相対パスそのまま
    raw = Path(name)
    if raw.is_absolute():
        out.append(raw)
    else:
        out.append(root / name)

    # pip インストール後も同梱 BVH が解決できるようにする
    pkg_data = Path(__file__).resolve().parent / "data"
    if kind in (AssetKind.BVH, AssetKind.VRMA, AssetKind.GLTF, AssetKind.ANY):
        stem = Path(name).stem
        if kind in (AssetKind.BVH, AssetKind.ANY):
            out.append(pkg_data / f"{stem}.bvh")
            if stem.lower() == "dance":
                out.append(pkg_data / "synthetic_dance.bvh")
        if kind in (AssetKind.VRMA, AssetKind.ANY):
            out.append(pkg_data / f"{stem}.vrma")
        if kind in (AssetKind.GLTF, AssetKind.ANY):
            out.append(pkg_data / f"{stem}.glb")
            out.append(pkg_data / f"{stem}.gltf")
            if stem.lower() == "cube":
                out.append(pkg_data / "unit_cube.glb")

    key = Path(name).stem.lower()
    for alias in _ALIASES.get(key, []):
        out.append(root / alias)

    exts = _exts_for(kind)
    if kind is AssetKind.ANY:
        # dance("wave") が wave.vrma / .bvh / .fbx のどれでも当たるようにする
        exts = (".vrma", ".bvh", ".fbx", ".glb", ".gltf")
    stems = {Path(name).stem, name}
    if not Path(name).suffix:
        stems.add(name)
    for base in _SEARCH_ROOTS:
        for stem in stems:
            if exts:
                for ext in exts:
                    out.append(root / base / f"{stem}{ext}")
                    out.append(root / base / f"{stem.capitalize()}{ext}")
            else:
                out.append(root / base / stem)

    # 重複除去（順序維持）
    seen: set[str] = set()
    uniq: list[Path] = []
    for p in out:
        s = str(p.resolve()) if p.exists() else str(p)
        key_s = os.path.normcase(s)
        if key_s in seen:
            continue
        seen.add(key_s)
        uniq.append(p)
    return uniq


def resolve_asset(
    kind: AssetKind,
    name: str,
    *,
    root: Path | None = None,
    required: bool = True,
) -> Path | None:
    """アセットを解決。見つからなければ KagraContractError（required=True）。"""
    root = root or project_root()
    cands = candidate_paths(kind, name, root=root)
    exts = _exts_for(kind)
    for p in cands:
        if not p.is_file():
            continue
        if exts and p.suffix.lower() not in exts:
            continue
        return p.resolve()
    tried = [str(p) for p in cands[:12]]
    if not required:
        return None
    raise KagraContractError(
        code="ASSET_NOT_FOUND",
        message=f"{kind.value} asset not found: {name}",
        hint=(
            f"Place a {kind.value} file under assets/ (or tests/fixtures/), "
            f"or pass an absolute path. Aliases: {', '.join(sorted(_ALIASES))}. "
            + (
                "Or run `python -m kagra.demo` to download a sample VRM and play."
                if kind is AssetKind.VRM
                else ""
            )
        ),
        path=name,
        candidates=tried,
    )


def require_files(paths: Iterable[str | Path], *, root: Path | None = None) -> list[Path]:
    root = root or project_root()
    found: list[Path] = []
    missing: list[str] = []
    for raw in paths:
        p = Path(raw)
        if not p.is_absolute():
            p = root / p
        if p.is_file():
            found.append(p.resolve())
        else:
            missing.append(str(p))
    if missing:
        raise KagraContractError(
            code="REQUIRED_FILES_MISSING",
            message="required files missing",
            hint="Create the files or update the scenario/asset list",
            candidates=missing,
        )
    return found


def describe_environment(root: Path | None = None) -> dict:
    """エージェント向け環境スナップショット。"""
    root = root or project_root()
    assets = root / "assets"
    vrms = sorted(str(p.relative_to(root)) for p in assets.rglob("*.vrm")) if assets.exists() else []
    fbxs = sorted(str(p.relative_to(root)) for p in assets.rglob("*.fbx")) if assets.exists() else []
    gltfs = (
        sorted(
            str(p.relative_to(root))
            for p in list(assets.rglob("*.glb")) + list(assets.rglob("*.gltf"))
        )
        if assets.exists()
        else []
    )
    fixtures = root / "tests" / "fixtures"
    bvh = (
        sorted(str(p.relative_to(root)) for p in fixtures.rglob("*.bvh"))
        if fixtures.exists()
        else []
    )
    vrma = (
        sorted(str(p.relative_to(root)) for p in fixtures.rglob("*.vrma"))
        if fixtures.exists()
        else []
    )
    return {
        "root": str(root),
        "has_assets_dir": assets.is_dir(),
        "vrm_files": vrms[:20],
        "fbx_files": fbxs[:20],
        "gltf_files": gltfs[:20],
        "bvh_fixtures": bvh[:20],
        "vrma_fixtures": vrma[:20],
        "aliases": {k: v for k, v in _ALIASES.items()},
        "api_index": str(root / "docs" / "API_INDEX.md"),
    }


def dump_environment_json(root: Path | None = None) -> str:
    return json.dumps(describe_environment(root), ensure_ascii=False, indent=2)
