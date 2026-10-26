"""ゲームマニフェストと、Windows / Android 共通のステージング。

``games/<example-stem>.json`` を両方のパッケージスクリプトが読む。
必須キーは title / icon / version / entry。``world`` があると Android の
WorldPlay がそれを進める。Windows は ``entry`` の Python 例をそのまま起動する。
"""
from __future__ import annotations

import json
import re
import struct
import zlib
from dataclasses import dataclass, field
from pathlib import Path

from kagra.contracts import AssetKind, resolve_asset
from kagra.meta_spec import MetaError, normalize_meta
from kagra.page_spec import PageError, normalize_page
from kagra.platform_spec import PlatformError, normalize_platform

REQUIRED = ("title", "icon", "version", "entry")
_VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)")
_APP_ID_RE = re.compile(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$")
_ASSET_KEYS = {"gltf", "model", "texture", "normal", "albedo", "mesh", "icon"}
_ASSET_SUFFIXES = (".glb", ".gltf", ".png", ".jpg", ".jpeg", ".webp", ".wav", ".ogg", ".vrm", ".fbx")
_IMPORT_RE = re.compile(r"(?:from|import)\s+kagra\.([A-Za-z0-9_]+)")
# 閉じ引用符がすぐ続くものだけ。モジュール docstring の三重引用をまたがない。
_PATH_RE = re.compile(r"""['"]((?:assets|kagra-shared)/[^'"]+)['"]""")
_ALIAS_RE = re.compile(r"""['"]([A-Za-z][A-Za-z0-9_]{1,31})['"]""")


class ManifestError(ValueError):
    """マニフェストが読めない、または必須項目が無い。"""


@dataclass
class GameManifest:
    title: str
    icon: str
    version: str
    entry: str
    application_id: str
    world: str | None = None
    bundle_assets: list[str] = field(default_factory=list)
    path: Path | None = None
    slug: str = ""
    platform: dict = field(default_factory=dict)
    meta: dict = field(default_factory=dict)
    page: dict = field(default_factory=dict)

    def version_code(self) -> int:
        return version_code(self.version)


def version_code(version: str) -> int:
    """``major.minor.patch`` → ``major * 10000 + minor * 100 + patch``。"""
    match = _VERSION_RE.match(version.strip())
    if not match:
        raise ManifestError(f"version must look like 1.2.3, got {version!r}")
    major, minor, patch = (int(match.group(i)) for i in range(1, 4))
    return major * 10000 + minor * 100 + patch


def slug_from_entry(entry: str) -> str:
    stem = Path(entry).stem
    if stem.endswith("_minimal"):
        stem = stem[: -len("_minimal")]
    slug = re.sub(r"[^a-z0-9_]+", "_", stem.lower()).strip("_")
    if not slug:
        raise ManifestError(f"cannot derive a slug from {entry!r}")
    return slug


def application_id_for(slug: str) -> str:
    ident = f"dev.kagra.{slug}"
    if not _APP_ID_RE.match(ident):
        raise ManifestError(f"application id is not a Java package: {ident}")
    return ident


def find_manifest(repo: Path, game: Path) -> Path:
    """``--game`` に渡した例またはマニフェスト JSON を解決する。"""
    repo = repo.resolve()
    game = game if game.is_absolute() else repo / game
    if game.suffix.lower() == ".json":
        if not game.is_file():
            raise ManifestError(f"manifest not found: {game}")
        return game
    stem = game.stem
    candidates = [
        repo / "games" / f"{stem}.json",
        repo / "games" / f"{stem}.kagra.json",
        game.with_suffix(".kagra.json"),
    ]
    for cand in candidates:
        if cand.is_file():
            return cand
    raise ManifestError(
        "no game manifest for "
        f"{game}. Add games/{stem}.json with title, icon, version, and entry. "
        "See docs/distribution.md."
    )


def load_manifest(path: Path, repo: Path | None = None) -> GameManifest:
    repo = (repo or Path.cwd()).resolve()
    path = path.resolve()
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ManifestError(f"{path}: {exc}") from exc
    if not isinstance(raw, dict):
        raise ManifestError(f"{path}: manifest must be a JSON object")
    missing = [key for key in REQUIRED if not str(raw.get(key, "")).strip()]
    if missing:
        raise ManifestError(f"{path}: missing {', '.join(missing)}")
    entry = _repo_relative(str(raw["entry"]), repo)
    icon = _repo_relative(str(raw["icon"]), repo)
    world_raw = raw.get("world")
    world = _repo_relative(str(world_raw), repo) if world_raw else None
    bundle = raw.get("bundle_assets") or []
    if not isinstance(bundle, list) or not all(isinstance(item, str) for item in bundle):
        raise ManifestError(f"{path}: bundle_assets must be a list of strings")
    version = str(raw["version"]).strip()
    version_code(version)
    slug = slug_from_entry(entry)
    app_id = str(raw.get("application_id") or application_id_for(slug)).strip()
    if not _APP_ID_RE.match(app_id):
        raise ManifestError(f"{path}: application_id is not a Java package: {app_id}")
    try:
        platform = normalize_platform(raw.get("platform"))
    except PlatformError as exc:
        raise ManifestError(f"{path}: {exc}") from exc
    try:
        meta = normalize_meta(raw.get("meta"))
    except MetaError as exc:
        raise ManifestError(f"{path}: {exc}") from exc
    try:
        page = normalize_page(raw.get("page"))
    except PageError as exc:
        raise ManifestError(f"{path}: {exc}") from exc
    if meta.get("rewarded") and meta["rewarded"]["slot"] not in {r["slot"] for r in platform.get("ads", {}).get("rewarded", [])}:
        raise ManifestError(
            f"{path}: meta.rewarded.slot {meta['rewarded']['slot']!r} is not a platform.ads.rewarded slot"
        )
    _must_exist(repo / entry, "entry")
    _must_exist(repo / icon, "icon")
    if world:
        _must_exist(repo / world, "world")
    return GameManifest(
        title=str(raw["title"]).strip(),
        icon=icon,
        version=version,
        entry=entry,
        application_id=app_id,
        world=world,
        bundle_assets=[_repo_relative(item, repo) for item in bundle],
        path=path,
        slug=slug,
        platform=platform,
        meta=meta,
        page=page,
    )


def manifest_for_game(repo: Path, game: str | Path) -> GameManifest:
    repo = repo.resolve()
    return load_manifest(find_manifest(repo, Path(game)), repo)


def _repo_relative(value: str, repo: Path) -> str:
    text = value.strip().replace("\\", "/")
    if not text or text.startswith("/") or ".." in Path(text).parts:
        raise ManifestError(f"path must stay inside the repo: {value!r}")
    path = Path(text)
    if path.is_absolute():
        try:
            text = path.resolve().relative_to(repo.resolve()).as_posix()
        except ValueError as exc:
            raise ManifestError(f"path is outside the repo: {value}") from exc
    return text


def _must_exist(path: Path, label: str) -> None:
    if not path.is_file():
        raise ManifestError(f"{label} file not found: {path}")


def collect_assets(repo: Path, manifest: GameManifest) -> list[Path]:
    """ステージに入れるアセットファイル。ディレクトリ指定は中のファイルへ展開する。"""
    repo = repo.resolve()
    found: set[Path] = set()
    if manifest.bundle_assets == ["assets"]:
        root = repo / "assets"
        if root.is_dir():
            for path in root.rglob("*"):
                if path.is_file():
                    found.add(path.resolve())
        return sorted(found)
    for rel in manifest.bundle_assets:
        found.update(_expand(repo / rel))
    found.update(_scan_world(repo, manifest.world))
    found.update(_scan_python(repo, repo / manifest.entry))
    expanded: set[Path] = set()
    for path in found:
        expanded.update(_sidecars(path))
    return sorted(expanded)


def _expand(path: Path) -> set[Path]:
    if path.is_file():
        return {path.resolve()}
    if path.is_dir():
        return {child.resolve() for child in path.rglob("*") if child.is_file()}
    return set()


def _sidecars(path: Path) -> set[Path]:
    """``.gltf`` は同じフォルダの bin / 画像と、uri が指す実在ファイルを一緒に持っていく。"""
    if path.suffix.lower() != ".gltf" or not path.is_file():
        return {path}
    keep = {path.resolve()}
    parent = path.parent.resolve()
    for child in parent.iterdir():
        if child.is_file() and child.suffix.lower() in {".bin", ".png", ".jpg", ".jpeg", ".webp"}:
            keep.add(child.resolve())
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return keep

    def walk(node: object) -> None:
        if isinstance(node, dict):
            uri = node.get("uri")
            if isinstance(uri, str) and not uri.startswith("data:"):
                target = (parent / uri).resolve()
                try:
                    target.relative_to(parent)
                except ValueError:
                    target = None
                if target is not None and target.is_file():
                    keep.add(target)
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for item in node:
                walk(item)

    walk(data)
    return keep


def _scan_world(repo: Path, world: str | None) -> set[Path]:
    if not world:
        return set()
    path = repo / world
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return set()
    found: set[Path] = set()
    _walk_json(repo, data, found)
    return found


def _walk_json(repo: Path, node: object, found: set[Path]) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            if key in _ASSET_KEYS and isinstance(value, str):
                _consider(repo, value, found)
            else:
                _walk_json(repo, value, found)
    elif isinstance(node, list):
        for item in node:
            _walk_json(repo, item, found)


def _scan_python(repo: Path, entry: Path) -> set[Path]:
    files = [entry]
    try:
        text = entry.read_text(encoding="utf-8")
    except OSError:
        return set()
    for name in _IMPORT_RE.findall(text):
        module = repo / "kagra" / f"{name}.py"
        if module.is_file():
            files.append(module)
    found: set[Path] = set()
    for path in files:
        try:
            body = path.read_text(encoding="utf-8")
        except OSError:
            continue
        for token in _PATH_RE.findall(body):
            _consider(repo, token, found)
        for token in _ALIAS_RE.findall(body):
            _consider(repo, token, found)
    return found


def _consider(repo: Path, token: str, found: set[Path]) -> None:
    token = token.strip()
    if not token or len(token) > 240 or ".." in Path(token).parts:
        return
    direct = repo / token
    if direct.is_file():
        found.add(direct.resolve())
        return
    if direct.is_dir():
        found.update(_expand(direct))
        return
    looks_named = "/" in token or token.lower().endswith(_ASSET_SUFFIXES) or token.isidentifier()
    if not looks_named:
        return
    for kind in (
        AssetKind.GLTF,
        AssetKind.VRM,
        AssetKind.TEXTURE,
        AssetKind.AUDIO,
        AssetKind.FBX,
        AssetKind.ANY,
    ):
        try:
            hit = resolve_asset(kind, token, root=repo, required=False)
        except Exception:
            hit = None
        if hit is not None and hit.is_file():
            found.add(hit.resolve())
            return


def stage_game(repo: Path, manifest: GameManifest, dest: Path) -> list[str]:
    """配布用のファイルツリーを ``dest`` に書く。戻り値は相対パス。

    レイアウトはリポジトリと同じ相対パス。PyInstaller の ``_internal`` や
    Android の ``filesDir/kagra_root`` をここにする。
    """
    repo = repo.resolve()
    dest = dest.resolve()
    dest.mkdir(parents=True, exist_ok=True)
    copied: list[str] = []

    def copy_rel(rel: str) -> None:
        src = repo / rel
        if not src.is_file():
            return
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(src.read_bytes())
        copied.append(rel.replace("\\", "/"))

    copy_rel(manifest.entry)
    copy_rel(manifest.icon)
    if manifest.world:
        copy_rel(manifest.world)
        world_bytes = (repo / manifest.world).read_bytes()
        (dest / "world.json").write_bytes(world_bytes)
        copied.append("world.json")
    marker = repo / "kagra-shared" / "Cargo.toml"
    if marker.is_file():
        copy_rel("kagra-shared/Cargo.toml")
    for asset in collect_assets(repo, manifest):
        try:
            rel = asset.resolve().relative_to(repo).as_posix()
        except ValueError:
            continue
        copy_rel(rel)
    payload = {
        "title": manifest.title,
        "icon": manifest.icon,
        "version": manifest.version,
        "entry": manifest.entry,
        "application_id": manifest.application_id,
        "world": manifest.world,
        "slug": manifest.slug,
    }
    (dest / "kagra_game.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    copied.append("kagra_game.json")
    return sorted(set(copied))


def write_android_properties(manifest: GameManifest, path: Path, *, icon_ref: str) -> None:
    """Gradle が UTF-8 で読む ``key=value``。"""
    lines = [
        f"title={_prop(manifest.title)}",
        f"version_name={_prop(manifest.version)}",
        f"version_code={manifest.version_code()}",
        f"application_id={_prop(manifest.application_id)}",
        f"icon={_prop(icon_ref)}",
        "",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines), encoding="utf-8")


def _prop(value: str) -> str:
    return value.replace("\n", " ").replace("\r", " ").strip()


def solid_icon_png(label: str, size: int = 48) -> bytes:
    """タイトルから色を決めた正方形 PNG。Pillow 不要。"""
    hue = sum(label.encode("utf-8")) % 360
    r, g, b = _hsl(hue, 0.55, 0.42)
    inner_r, inner_g, inner_b = _hsl((hue + 40) % 360, 0.7, 0.72)
    pixels = bytearray(size * size * 4)
    margin = size // 6
    for y in range(size):
        for x in range(size):
            i = (y * size + x) * 4
            inside = margin <= x < size - margin and margin <= y < size - margin
            if inside and (x + y) % 9 < 5:
                pixels[i : i + 4] = bytes((inner_r, inner_g, inner_b, 255))
            else:
                pixels[i : i + 4] = bytes((r, g, b, 255))
    return rgba_png(bytes(pixels), size, size)


def rgba_png(rgba: bytes, width: int, height: int) -> bytes:
    """RGBA8 → PNG（標準ライブラリのみ）。"""

    def chunk(typ: bytes, data: bytes) -> bytes:
        body = typ + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    raw = b"".join(b"\x00" + rgba[y * width * 4 : (y + 1) * width * 4] for y in range(height))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def png_to_ico(png: bytes) -> bytes:
    """Vista 以降が読める、PNG を包んだだけの ICO。"""
    if not png.startswith(b"\x89PNG"):
        raise ManifestError("icon is not a PNG")
    # ICONDIR + 1 entry + PNG payload
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png), 6 + 16)
    return header + entry + png


def _hsl(h: int, s: float, light: float) -> tuple[int, int, int]:
    c = (1 - abs(2 * light - 1)) * s
    x = c * (1 - abs((h / 60) % 2 - 1))
    m = light - c / 2
    if h < 60:
        r, g, b = c, x, 0.0
    elif h < 120:
        r, g, b = x, c, 0.0
    elif h < 180:
        r, g, b = 0.0, c, x
    elif h < 240:
        r, g, b = 0.0, x, c
    elif h < 300:
        r, g, b = x, 0.0, c
    else:
        r, g, b = c, 0.0, x
    return (int((r + m) * 255), int((g + m) * 255), int((b + m) * 255))
