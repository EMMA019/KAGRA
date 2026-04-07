
import json
from kagra.scene_io import serialize_entity
from kagra.scene_loader import load_entity

class Prefab:

    def __init__(self, data):
        self.data = data

    @staticmethod
    def from_entity(entity):
        return Prefab(serialize_entity(entity))

    def instantiate(self, component_registry):
        return load_entity(self.data, component_registry)

    def save(self, path):
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.data, f, indent=2)

    @staticmethod
    def load(path):
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        return Prefab(data)
