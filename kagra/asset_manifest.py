
import json
from pathlib import Path

class AssetManifest:
    def __init__(self, path="assets_manifest.json"):
        self.path = Path(path)
        self.data = {}

    def load(self):
        if self.path.exists():
            with open(self.path, "r", encoding="utf-8") as f:
                self.data = json.load(f)
        return self.data

    def save(self, records):
        out = {}
        for k, r in records.items():
            out[k] = {
                "type": r.asset_type.value if hasattr(r.asset_type, "value") else str(r.asset_type),
                "path": r.path,
                "metadata": r.metadata,
                "dependencies": r.dependencies
            }
        with open(self.path, "w", encoding="utf-8") as f:
            json.dump(out, f, indent=2, ensure_ascii=False)
