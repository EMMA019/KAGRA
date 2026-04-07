# kagra/scriptable.py
# Scriptable Object システム
#
# ── 使い方 ──────────────────────────────────────────────────
#
# 1) データファイルを作る（data/enemies/goblin.json）:
#   {
#     "_type": "enemy",
#     "name": "ゴブリン",
#     "hp": 30,
#     "speed": 80,
#     "damage": 5,
#     "sprite": "enemies/goblin",
#     "drop_rate": 0.3
#   }
#
# 2) 読み込む:
#   goblin = kagra.load_data("enemies/goblin")
#   goblin.hp        # → 30
#   goblin.name      # → "ゴブリン"
#   goblin["sprite"] # → "enemies/goblin"
#   goblin.get("drop_rate", 0.0)  # → 0.3
#
# 3) Entity に展開する:
#   entity = kagra.spawn_from(goblin, world)
#   # SpawnRule で定義した変換が自動適用される
#
# 4) SpawnRule を登録する:
#   @kagra.spawn_rule("enemy")
#   def build_enemy(data: DataObject, entity: Entity):
#       entity.add(EnemyScript(hp=data.hp, speed=data.speed))
#       entity.add(SpriteRenderer(kagra.load_texture(data.sprite), 32, 32))
#
# ─────────────────────────────────────────────────────────────

from __future__ import annotations

import json
import os
from typing import Any


# ════════════════════════════════════════════════════════
#  DataObject
# ════════════════════════════════════════════════════════

class DataObject:
    """JSON データの読み取り専用コンテナ。

    ドット記法とブラケット記法の両方でアクセスできる。
    存在しないキーにアクセスした場合は AttributeError / KeyError を出す。
    get() でデフォルト値付きアクセスが可能。

    Example::
        goblin = DataObject.from_file("data/enemies/goblin.json", key="enemies/goblin")
        goblin.hp           # 30
        goblin["speed"]     # 80
        goblin.get("boss", False)  # False（キーがなければデフォルト）
        goblin._type        # "enemy"
    """

    def __init__(self, data: dict, key: str = "", source_path: str = ""):
        # _data を直接 __dict__ に格納して循環を避ける
        object.__setattr__(self, "_data", data)
        object.__setattr__(self, "_key", key)
        object.__setattr__(self, "_source_path", source_path)

    # ── アクセス ─────────────────────────────────────────

    def __getattr__(self, name: str) -> Any:
        data = object.__getattribute__(self, "_data")
        if name in data:
            return data[name]
        raise AttributeError(
            f"DataObject '{object.__getattribute__(self, '_key')}' "
            f"has no field '{name}'"
        )

    def __getitem__(self, key: str) -> Any:
        return object.__getattribute__(self, "_data")[key]

    def __contains__(self, key: str) -> bool:
        return key in object.__getattribute__(self, "_data")

    def get(self, key: str, default: Any = None) -> Any:
        return object.__getattribute__(self, "_data").get(key, default)

    def __setattr__(self, name, value):
        raise AttributeError("DataObject は読み取り専用です。")

    # ── メタ情報 ─────────────────────────────────────────

    @property
    def key(self) -> str:
        return object.__getattribute__(self, "_key")

    @property
    def source_path(self) -> str:
        return object.__getattribute__(self, "_source_path")

    @property
    def type(self) -> str:
        """_type フィールドを返す。未設定なら空文字列。"""
        return object.__getattribute__(self, "_data").get("_type", "")

    def to_dict(self) -> dict:
        """生の辞書を返す（コピー）。"""
        return dict(object.__getattribute__(self, "_data"))

    def keys(self) -> list[str]:
        return list(object.__getattribute__(self, "_data").keys())

    def __repr__(self) -> str:
        key = object.__getattribute__(self, "_key")
        t   = object.__getattribute__(self, "_data").get("_type", "?")
        return f"<DataObject key={key!r} type={t!r}>"

    # ── ファクトリ ────────────────────────────────────────

    @classmethod
    def from_file(cls, path: str, key: str = "") -> "DataObject":
        """JSONファイルから DataObject を生成する。"""
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        if not isinstance(data, dict):
            raise ValueError(f"DataObject のJSONはオブジェクト（dict）である必要があります: {path}")
        return cls(data, key=key or path, source_path=path)

    @classmethod
    def from_dict(cls, data: dict, key: str = "") -> "DataObject":
        """辞書から DataObject を生成する。"""
        return cls(dict(data), key=key)

    def derive(self, overrides: dict, new_key: str = "") -> "DataObject":
        """このオブジェクトをベースに一部フィールドを上書きした新しい DataObject を返す。

        Example::
            elite_goblin = goblin.derive({"hp": 100, "damage": 20}, key="enemies/goblin_elite")
        """
        merged = {**object.__getattribute__(self, "_data"), **overrides}
        key = object.__getattribute__(self, "_key")
        return DataObject(merged, key=new_key or f"{key}(derived)")


# ════════════════════════════════════════════════════════
#  DataRegistry
# ════════════════════════════════════════════════════════

class DataRegistry:
    """DataObject のキャッシュ・検索を管理するレジストリ。

    Example::
        registry = DataRegistry(base_dir="data")
        goblin = registry.load("enemies/goblin")   # data/enemies/goblin.json
        all_enemies = registry.all_of_type("enemy")
    """

    def __init__(self, base_dir: str = "data"):
        self.base_dir = base_dir
        self._cache: dict[str, DataObject] = {}

    def _resolve_path(self, key: str) -> str:
        """キー → JSONファイルパスを解決する。"""
        # 拡張子がなければ .json を追加
        if not key.endswith(".json"):
            key_with_ext = key + ".json"
        else:
            key_with_ext = key
        return os.path.join(self.base_dir, key_with_ext)

    def load(self, key: str, force_reload: bool = False) -> DataObject:
        """キーで DataObject を取得する（キャッシュあり）。

        Args:
            key          : "enemies/goblin" のようなスラッシュ区切りのキー
            force_reload : True なら必ずファイルから再読み込み

        Returns:
            DataObject
        """
        if not force_reload and key in self._cache:
            return self._cache[key]

        path = self._resolve_path(key)
        if not os.path.exists(path):
            raise FileNotFoundError(
                f"DataObject ファイルが見つかりません: {path}\n"
                f"  key='{key}', base_dir='{self.base_dir}'"
            )
        obj = DataObject.from_file(path, key=key)
        self._cache[key] = obj
        return obj

    def load_dict(self, key: str, data: dict) -> DataObject:
        """辞書から DataObject を登録する（テスト・動的生成用）。"""
        obj = DataObject.from_dict(data, key=key)
        self._cache[key] = obj
        return obj

    def unload(self, key: str) -> bool:
        """キャッシュから削除する。"""
        return self._cache.pop(key, None) is not None

    def clear(self):
        """全キャッシュをクリアする。"""
        self._cache.clear()

    def preload_dir(self, subdir: str = "", recursive: bool = True) -> list[str]:
        """ディレクトリ内の全JSONファイルを先読みする。

        Args:
            subdir    : base_dir からの相対サブディレクトリ（空文字で base_dir 全体）
            recursive : True でサブディレクトリも再帰的に読む

        Returns:
            読み込んだキーのリスト
        """
        root = os.path.join(self.base_dir, subdir) if subdir else self.base_dir
        loaded = []
        if not os.path.isdir(root):
            return loaded

        pattern = "**/*.json" if recursive else "*.json"
        import glob
        for path in glob.glob(os.path.join(root, pattern), recursive=recursive):
            # base_dir からの相対パスをキーにする
            rel = os.path.relpath(path, self.base_dir)
            key = rel.replace("\\", "/").removesuffix(".json")
            try:
                self.load(key)
                loaded.append(key)
            except Exception as e:
                print(f"[DataRegistry] preload 失敗 {key}: {e}")
        return loaded

    def get_cached(self, key: str) -> "DataObject | None":
        """キャッシュ済みなら返す。なければ None。"""
        return self._cache.get(key)

    def all_keys(self) -> list[str]:
        """キャッシュ済みの全キーを返す。"""
        return list(self._cache.keys())

    def all_of_type(self, type_name: str) -> list[DataObject]:
        """_type フィールドが一致する全 DataObject を返す。"""
        return [obj for obj in self._cache.values() if obj.type == type_name]

    def find(self, **conditions) -> list[DataObject]:
        """フィールド条件でフィルタする。

        Example::
            bosses = registry.find(_type="enemy", is_boss=True)
        """
        results = []
        for obj in self._cache.values():
            data = obj.to_dict()
            if all(data.get(k) == v for k, v in conditions.items()):
                results.append(obj)
        return results


# ════════════════════════════════════════════════════════
#  SpawnRule / spawn_from
# ════════════════════════════════════════════════════════

_spawn_rules: dict[str, list] = {}


def register_spawn_rule(type_name: str, rule_fn):
    """DataObject._type に対応するスポーンルールを登録する。

    rule_fn のシグネチャ: (data: DataObject, entity: Entity) -> None

    Example::
        def build_goblin(data, entity):
            entity.add(EnemyScript(hp=data.hp))
            entity.add(SpriteRenderer(tex_id, 32, 32))

        register_spawn_rule("enemy", build_goblin)
    """
    _spawn_rules.setdefault(type_name, []).append(rule_fn)


def spawn_rule(type_name: str):
    """スポーンルールをデコレータとして登録する。

    Example::
        @kagra.spawn_rule("enemy")
        def build_enemy(data, entity):
            entity.add(EnemyScript(hp=data.hp, speed=data.speed))
    """
    def decorator(fn):
        register_spawn_rule(type_name, fn)
        return fn
    return decorator


def spawn_from(data: DataObject, world, name: str = "") -> "Entity":
    """DataObject から Entity を生成してワールドに追加する。

    登録済みのスポーンルールを自動適用する。

    Args:
        data  : DataObject（load_data() で取得したもの）
        world : kagra.World インスタンス
        name  : Entity 名（省略時は data.get("name", data.key)）

    Returns:
        生成された Entity

    Example::
        goblin_def = kagra.load_data("enemies/goblin")
        entity = kagra.spawn_from(goblin_def, self.world)
    """
    from kagra.entity import Entity

    entity_name = name or data.get("name", data.key)
    tag          = data.get("tag", data.type)
    entity       = world.create(name=entity_name, tag=tag)

    # Transform 初期値
    entity.transform.x = data.get("x", 0.0)
    entity.transform.y = data.get("y", 0.0)

    # スポーンルール適用
    type_name = data.type
    rules = _spawn_rules.get(type_name, [])
    for rule_fn in rules:
        try:
            rule_fn(data, entity)
        except Exception as e:
            print(f"[spawn_from] SpawnRule '{type_name}' error: {e}")

    return entity


# ════════════════════════════════════════════════════════
#  グローバルレジストリ
# ════════════════════════════════════════════════════════

_global_registry = DataRegistry(base_dir="data")


def get_data_registry() -> DataRegistry:
    """グローバル DataRegistry を返す。"""
    return _global_registry


def set_data_dir(path: str):
    """グローバルレジストリの base_dir を変更する。

    Example::
        kagra.set_data_dir("assets/data")
    """
    global _global_registry
    _global_registry = DataRegistry(base_dir=path)
