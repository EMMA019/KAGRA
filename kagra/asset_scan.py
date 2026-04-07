
from pathlib import Path
from .asset_types import AssetType

EXT_MAP = {
    ".png": AssetType.IMAGE,
    ".jpg": AssetType.IMAGE,
    ".jpeg": AssetType.IMAGE,
    ".wav": AssetType.AUDIO,
    ".ogg": AssetType.AUDIO,
    ".ttf": AssetType.FONT,
    ".json": AssetType.JSON,
    ".csv": AssetType.MAP,
}

def scan_assets(db, root="assets"):
    root = Path(root)
    for p in root.rglob("*"):
        if not p.is_file():
            continue
        ext = p.suffix.lower()
        if ext not in EXT_MAP:
            continue

        asset_type = EXT_MAP[ext]
        key = str(p.relative_to(root)).replace("\\", "/")
        key = key.rsplit(".", 1)[0]

        if not db.exists(key):
            db.register(key, asset_type, str(p))
