# kagra/assets.py
# アセット管理 - パス解決・キャッシュ・遅延ロード
#
# 使い方:
#   kagra.assets.image("char/front")   → assets/img/char/front.png
#   kagra.assets.font("meiryo")        → 自動でシステムフォントを探す
#   kagra.assets.tileset("dungeon", 16, 16)
#
# 設定変更:
#   kagra.assets.base_dir = "my_assets"   # デフォルト: "assets"
#   kagra.assets.image_dir = "sprites"    # デフォルト: "img"

from __future__ import annotations
import os
import sys
from typing import Optional
from kagra.fonts import find_system_font
from kagra.tilemap import TileSet


# ── システムフォント候補 ──────────────────────────────────────
_SYSTEM_FONTS: dict[str, list[str]] = {
    "meiryo": [
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/meiryob.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    ],
    "gothic": [
        "C:/Windows/Fonts/msgothic.ttc",
        "C:/Windows/Fonts/yugothic.ttf",
        "/System/Library/Fonts/ヒラギノ丸ゴ ProN W4.ttc",
    ],
    "mincho": [
        "C:/Windows/Fonts/msmincho.ttc",
        "C:/Windows/Fonts/yumin.ttf",
        "/System/Library/Fonts/ヒラギノ明朝 ProN W3.ttc",
    ],
}


class AssetManager:
    """KAGRAアセットマネージャー。

    テクスチャ・フォント・タイルセットをキャッシュ付きでロードする。
    パスの解決は自動で行い、ゲームコードから文字列パスを排除する。

    Example::
        # assets/img/player/front.png をロード（2回目以降はキャッシュ）
        tex = kagra.assets.image("player/front")

        # assets/img/tiles/dungeon.png を 16×16 タイルセットとしてロード
        ts = kagra.assets.tileset("tiles/dungeon", 16, 16)

        # システムフォント自動検索
        font = kagra.assets.font("meiryo")
    """

    def __init__(self):
        # ベースディレクトリ（プロジェクトルートからの相対パス）
        self.base_dir:  str = "assets"
        self.image_dir: str = "img"
        self.audio_dir: str = "audio"
        self.font_dir:  str = "fonts"

        # キャッシュ
        self._tex_cache:      dict[str, int]     = {}
        self._font_cache:     dict[str, int]     = {}
        self._tileset_cache:  dict[str, TileSet] = {}

        # 画像拡張子の優先順
        self._image_exts = [".png", ".jpg", ".jpeg", ".bmp", ".webp"]

    # ── パス解決 ─────────────────────────────────────────────

    def _resolve_image(self, name: str) -> str:
        """名前から画像ファイルパスを解決する。"""
        # すでに拡張子がある場合はそのまま
        if os.path.splitext(name)[1]:
            return name if os.path.isabs(name) else os.path.join(self.base_dir, name)

        # 拡張子なし: img_dir 以下を探す
        base = os.path.join(self.base_dir, self.image_dir, name)
        for ext in self._image_exts:
            path = base + ext
            if os.path.exists(path):
                return path

        # 見つからなくても最初の候補を返す（エラーメッセージをわかりやすくするため）
        return base + self._image_exts[0]

    def _resolve_font(self, name: str) -> str:
        """名前からフォントパスを解決する。システムフォントも検索する。"""
        # 絶対パスまたは拡張子ありはそのまま
        if os.path.isabs(name) or os.path.splitext(name)[1]:
            return name

        # システムフォント辞書
        if name.lower() in _SYSTEM_FONTS:
            for path in _SYSTEM_FONTS[name.lower()]:
                if os.path.exists(path):
                    return path

        # assets/fonts/ 以下を探す
        base = os.path.join(self.base_dir, self.font_dir, name)
        for ext in [".ttf", ".ttc", ".otf"]:
            path = base + ext
            if os.path.exists(path):
                return path

        found = find_system_font(name)
        if found:
            return found

        return base + ".ttf"  # 失敗時のダミー

    # ── ロード API ────────────────────────────────────────────

    def image(self, name: str) -> int:
        """テクスチャをロードしてIDを返す（キャッシュあり）。

        Args:
            name: ファイル名（拡張子省略可）。例: "player/front", "tiles/grass"
        Returns:
            texture_id (int)
        """
        if name in self._tex_cache:
            return self._tex_cache[name]

        path = self._resolve_image(name)
        import kagra
        tex_id = kagra.load_texture(path)
        self._tex_cache[name] = tex_id
        return tex_id

    def font(self, name: str = "meiryo") -> int:
        """フォントをロードしてIDを返す（キャッシュあり）。

        Args:
            name: フォント名またはパス。"meiryo" / "gothic" / "mincho" は自動検索。
        Returns:
            font_id (int)
        """
        if name in self._font_cache:
            return self._font_cache[name]

        path = self._resolve_font(name)
        import kagra
        font_id = kagra.load_font(path)
        self._font_cache[name] = font_id
        return font_id

    def tileset(self, name: str, tile_w: int, tile_h: int, spacing: int = 0) -> "TileSet":
        """タイルセットをロードしてTileSetを返す（キャッシュあり）。

        同じ name + サイズの組み合わせはキャッシュされる。

        Args:
            name:    画像名（image()と同じ解決ルール）
            tile_w:  タイル幅 (px)
            tile_h:  タイル高さ (px)
            spacing: タイル間の余白 (px)
        Returns:
            TileSet
        """
        key = f"{name}:{tile_w}:{tile_h}:{spacing}"
        if key in self._tileset_cache:
            return self._tileset_cache[key]

        tex_id = self.image(name)
        ts = TileSet(tex_id, tile_w, tile_h, spacing)
        self._tileset_cache[key] = ts
        return ts

    def audio(self, name: str) -> str:
        """オーディオファイルのパスを返す（ロードはkagra.audio.play_*に任せる）。"""
        if os.path.splitext(name)[1]:
            return os.path.join(self.base_dir, self.audio_dir, name) if not os.path.isabs(name) else name
        base = os.path.join(self.base_dir, self.audio_dir, name)
        for ext in [".ogg", ".wav", ".mp3", ".flac"]:
            path = base + ext
            if os.path.exists(path):
                return path
        return base + ".ogg"

    # ── ユーティリティ ────────────────────────────────────────

    def preload(self, *names: str) -> dict[str, int]:
        """複数のテクスチャを一括ロードしてdict{name: id}を返す。

        Example::
            textures = kagra.assets.preload(
                "player/front", "player/back",
                "player/left",  "player/right",
            )
        """
        return {name: self.image(name) for name in names}

    def clear_cache(self):
        """キャッシュをクリアする（シーン切り替え等で不要なら呼ばない）。"""
        self._tex_cache.clear()
        self._font_cache.clear()
        self._tileset_cache.clear()

    def debug_info(self) -> str:
        return (
            f"AssetManager: "
            f"{len(self._tex_cache)} textures, "
            f"{len(self._font_cache)} fonts, "
            f"{len(self._tileset_cache)} tilesets"
        )
