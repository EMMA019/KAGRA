from __future__ import annotations

from pathlib import Path
from typing import Callable

from kagra.asset_types import AssetRecord, AssetType


class AssetDatabase:
    """KAGRA 用の中核アセットDB。"""

    def __init__(self):
        self.base_dir = "assets"
        self.image_dir = "img"
        self.audio_dir = "audio"
        self.font_dir = "fonts"
        self.map_dir = "maps"
        self.rig_dir = "rigs"
        self.scene_dir = "scenes"
        self.prefab_dir = "prefabs"
        self.data_dir = "data"

        self._records: dict[str, AssetRecord] = {}
        self._loaders: dict[AssetType, Callable[["AssetDatabase", AssetRecord], object]] = {}

    def register_loader(
        self,
        asset_type: AssetType,
        loader: Callable[["AssetDatabase", AssetRecord], object],
    ) -> None:
        self._loaders[asset_type] = loader

    def register(
        self,
        key: str,
        asset_type: AssetType,
        path: str,
        metadata: dict | None = None,
        dependencies: list[str] | None = None,
        overwrite: bool = True,
    ) -> AssetRecord:
        if (not overwrite) and key in self._records:
            return self._records[key]
        rec = AssetRecord(
            key=key,
            asset_type=asset_type,
            path=path,
            metadata=metadata or {},
            dependencies=dependencies or [],
        )
        self._records[key] = rec
        return rec

    def unregister(self, key: str) -> bool:
        return self._records.pop(key, None) is not None

    def exists(self, key: str) -> bool:
        return key in self._records

    def peek(self, key: str) -> AssetRecord | None:
        return self._records.get(key)

    def require(self, key: str) -> AssetRecord:
        rec = self.peek(key)
        if rec is None:
            raise KeyError(f"Asset not registered: {key}")
        return rec

    def get(self, key: str):
        rec = self.require(key)
        if rec.loaded is None:
            loader = self._loaders.get(rec.asset_type)
            if loader is None:
                raise ValueError(f"No loader for asset type: {rec.asset_type}")
            rec.loaded = loader(self, rec)
            rec.load_count += 1
        return rec.loaded

    def reload(self, key: str):
        rec = self.require(key)
        rec.loaded = None
        rec.dirty = False
        return self.get(key)

    def mark_dirty(self, key: str) -> None:
        self.require(key).dirty = True

    def list_keys(self, asset_type: AssetType | None = None) -> list[str]:
        if asset_type is None:
            return sorted(self._records.keys())
        return sorted(k for k, v in self._records.items() if v.asset_type == asset_type)

    def list_records(self, asset_type: AssetType | None = None) -> list[AssetRecord]:
        if asset_type is None:
            return list(self._records.values())
        return [r for r in self._records.values() if r.asset_type == asset_type]

    def clear_cache(self, asset_type: AssetType | None = None) -> None:
        for rec in self.list_records(asset_type):
            rec.loaded = None
            rec.dirty = False

    def summary(self) -> dict[str, int]:
        out: dict[str, int] = {}
        for rec in self._records.values():
            out[rec.asset_type.value] = out.get(rec.asset_type.value, 0) + 1
        return dict(sorted(out.items()))

    def save_manifest(self, path: str = "assets_manifest.json") -> None:
        """登録済みアセット情報をマニフェストJSONに書き出す。"""
        from kagra.asset_manifest import AssetManifest
        AssetManifest(path).save(self._records)

    def load_manifest(self, path: str = "assets_manifest.json") -> int:
        """マニフェストJSONからアセットを一括登録する。返り値は登録件数。"""
        from kagra.asset_manifest import AssetManifest
        from kagra.asset_types import AssetType
        data = AssetManifest(path).load()
        count = 0
        for key, info in data.items():
            try:
                atype = AssetType(info["type"])
            except (KeyError, ValueError):
                continue
            self.register(
                key, atype,
                info.get("path", ""),
                metadata=info.get("metadata", {}),
                dependencies=info.get("dependencies", []),
                overwrite=False,
            )
            count += 1
        return count

    def scan_dir(
        self,
        root: str | Path,
        asset_type: AssetType,
        key_prefix: str,
        exts: tuple[str, ...],
        recursive: bool = True,
    ) -> int:
        root_path = Path(root)
        if not root_path.exists():
            return 0

        count = 0
        iterator = root_path.rglob("*") if recursive else root_path.glob("*")
        for path in iterator:
            if not path.is_file() or path.suffix.lower() not in exts:
                continue
            rel = path.relative_to(root_path).with_suffix("").as_posix()
            key = f"{key_prefix}/{rel}" if rel else key_prefix
            self.register(key, asset_type, path.as_posix(), overwrite=False)
            count += 1
        return count
