# kagra/tilemap.py
# タイルマップ - CSV/2D配列からマップを構築・描画・衝突判定

from __future__ import annotations
from typing import Optional

# ※ kagra をトップレベルでインポートしない（循環インポート回避）
# draw_texture は draw() 内で遅延インポートする


# タイル属性フラグ
TILE_SOLID   = 0x01   # 衝突あり
TILE_WATER   = 0x02   # 水（歩行不可）
TILE_LADDER  = 0x04   # はしご
TILE_DOOR    = 0x08   # ドア（マップ遷移トリガー）
TILE_DAMAGE  = 0x10   # ダメージ床


def _kagra():
    """遅延インポートヘルパー（entity.py と同じパターン）"""
    import kagra
    return kagra


class TileSet:
    """タイルセット画像からタイルUVを管理する。

    Args:
        texture_id : kagra.load_texture() の戻り値
        tile_w     : 1タイルの幅（px）
        tile_h     : 1タイルの高さ（px）
        spacing    : タイル間の余白（px）
    """
    def __init__(self, texture_id: int, tile_w: int, tile_h: int, spacing: int = 0):
        self.texture_id = texture_id
        self.tile_w     = tile_w
        self.tile_h     = tile_h
        self.spacing    = spacing
        tw, th = _kagra().texture_size(texture_id)
        self.cols = (tw + spacing) // (tile_w + spacing)
        self.rows = (th + spacing) // (tile_h + spacing)

    def get_uv(self, tile_id: int) -> tuple[float, float, float, float]:
        """tile_id から (sx, sy, sw, sh) を返す（0始まり）"""
        col = tile_id % self.cols
        row = tile_id // self.cols
        sx  = col * (self.tile_w + self.spacing)
        sy  = row * (self.tile_h + self.spacing)
        return float(sx), float(sy), float(self.tile_w), float(self.tile_h)


class TileMap:
    """タイルマップ本体。

    Args:
        tileset   : TileSetオブジェクト
        data      : 2D配列 [[tile_id, ...], ...]（行, 列）
        tile_attrs: {tile_id: flagビット} 衝突・属性定義
        tile_w/h  : 描画時のタイルサイズ（省略時はtileset準拠）

    Example::
        ts  = kagra.TileSet(tex, 16, 16)
        MAP = [
            [1,1,1,1,1],
            [1,0,0,0,1],
            [1,0,0,0,1],
            [1,1,1,1,1],
        ]
        ATTRS = {1: kagra.TILE_SOLID}
        tm = kagra.TileMap(ts, MAP, ATTRS)
    """
    def __init__(
        self,
        tileset:    TileSet,
        data:       list[list[int]],
        tile_attrs: dict[int, int] = None,
        tile_w:     int = None,
        tile_h:     int = None,
    ):
        self.tileset    = tileset
        self.data       = data
        self.tile_attrs = tile_attrs or {}
        self.tile_w     = tile_w or tileset.tile_w
        self.tile_h     = tile_h or tileset.tile_h
        self.map_rows   = len(data)
        self.map_cols   = len(data[0]) if data else 0

    @property
    def pixel_width(self) -> int:
        return self.map_cols * self.tile_w

    @property
    def pixel_height(self) -> int:
        return self.map_rows * self.tile_h

    def get_tile(self, col: int, row: int) -> int:
        """タイルIDを返す（範囲外は -1）"""
        if 0 <= row < self.map_rows and 0 <= col < self.map_cols:
            return self.data[row][col]
        return -1

    def set_tile(self, col: int, row: int, tile_id: int) -> bool:
        """タイルIDをセットする。範囲外は False を返す。"""
        if 0 <= row < self.map_rows and 0 <= col < self.map_cols:
            self.data[row][col] = tile_id
            return True
        return False

    def get_tile_at(self, wx: float, wy: float) -> int:
        """ワールド座標からタイルIDを返す"""
        col = int(wx // self.tile_w)
        row = int(wy // self.tile_h)
        return self.get_tile(col, row)

    def get_tile_attrs_at(self, wx: float, wy: float) -> int:
        """ワールド座標からタイル属性フラグを返す（範囲外は 0）。"""
        tid = self.get_tile_at(wx, wy)
        if tid < 0:
            return 0
        return self.tile_attrs.get(tid, 0)

    def world_to_cell(self, wx: float, wy: float) -> tuple[int, int]:
        """ワールド座標 → (col, row) に変換する"""
        return int(wx // self.tile_w), int(wy // self.tile_h)

    def cell_to_world(self, col: int, row: int) -> tuple[float, float]:
        """(col, row) → タイル左上のワールド座標に変換する"""
        return float(col * self.tile_w), float(row * self.tile_h)

    def has_attr(self, tile_id: int, flag: int) -> bool:
        """タイルが指定フラグを持つか"""
        if tile_id < 0:
            return False
        return bool(self.tile_attrs.get(tile_id, 0) & flag)

    def is_solid(self, col: int, row: int) -> bool:
        tid = self.get_tile(col, row)
        return tid >= 0 and self.has_attr(tid, TILE_SOLID)

    def draw(self, camera: "kagra.Camera"):
        """カメラ範囲内のタイルだけ描画（フラスタムカリング）"""
        kg   = _kagra()
        ts   = self.tileset
        tw   = self.tile_w
        th   = self.tile_h

        # 可視タイル範囲を計算
        col0 = max(0, int(camera.x // tw))
        col1 = min(self.map_cols, int((camera.x + camera.screen_w) // tw) + 2)
        row0 = max(0, int(camera.y // th))
        row1 = min(self.map_rows, int((camera.y + camera.screen_h) // th) + 2)

        for row in range(row0, row1):
            for col in range(col0, col1):
                tid = self.data[row][col]
                if tid < 0:
                    continue
                wx = col * tw
                wy = row * th
                sx, sy = camera.to_screen(wx, wy)
                usx, usy, usw, ush = ts.get_uv(tid)
                kg.draw_texture(
                    ts.texture_id,
                    sx, sy, float(tw), float(th),
                    usx, usy, usw, ush,
                )

    # ── AABB衝突判定（タイルマップ版） ──────────────────────────
    def collide_move(
        self,
        wx: float, wy: float, ww: float, wh: float,
        vx: float, vy: float,
    ) -> tuple[float, float, float, float, bool, bool]:
        """タイル衝突を考慮した移動後座標を返す。

        Args:
            wx, wy     : オブジェクトのワールド座標（左上）
            ww, wh     : オブジェクトサイズ
            vx, vy     : 移動量

        Returns:
            (new_wx, new_wy, new_vx, new_vy, hit_x, hit_y)
            hit_x : 横衝突があったか
            hit_y : 縦衝突があったか（着地判定に使う）
        """
        tw = self.tile_w
        th = self.tile_h
        hit_x = False
        hit_y = False

        # ── X軸移動 ─────────────────────────────────────────
        new_wx = wx + vx
        col_l = int(new_wx // tw)
        col_r = int((new_wx + ww - 1e-4) // tw)
        row_t = int(wy // th)
        row_b = int((wy + wh - 1e-4) // th)
        for row in range(row_t, row_b + 1):
            if vx > 0 and self.is_solid(col_r, row):
                new_wx = col_r * tw - ww
                vx = 0; hit_x = True; break
            elif vx < 0 and self.is_solid(col_l, row):
                new_wx = (col_l + 1) * tw
                vx = 0; hit_x = True; break

        # ── Y軸移動 ─────────────────────────────────────────
        new_wy = wy + vy
        col_l  = int(new_wx // tw)
        col_r  = int((new_wx + ww - 1e-4) // tw)
        row_t  = int(new_wy // th)
        row_b  = int((new_wy + wh - 1e-4) // th)
        for col in range(col_l, col_r + 1):
            if vy > 0 and self.is_solid(col, row_b):
                new_wy = row_b * th - wh
                vy = 0; hit_y = True; break
            elif vy < 0 and self.is_solid(col, row_t):
                new_wy = (row_t + 1) * th
                vy = 0; hit_y = True; break

        return new_wx, new_wy, vx, vy, hit_x, hit_y

    # ── CSV入出力 ────────────────────────────────────────────────
    @classmethod
    def from_csv(cls, tileset: TileSet, path: str,
                 tile_attrs: dict = None, **kwargs) -> "TileMap":
        """CSVファイルからTileMapを生成する。

        CSVフォーマット: カンマ区切りの整数、1行1行
        例:
            1,1,1,1,1
            1,0,0,0,1
            1,1,1,1,1
        """
        data = []
        with open(path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                data.append([int(x) for x in line.split(",")])
        return cls(tileset, data, tile_attrs, **kwargs)

    def to_csv(self, path: str) -> None:
        """TileMapをCSVファイルに書き出す。"""
        with open(path, "w", encoding="utf-8") as f:
            for row in self.data:
                f.write(",".join(str(x) for x in row) + "\n")

    @classmethod
    def empty(cls, tileset: TileSet, cols: int, rows: int,
              fill: int = -1, tile_attrs: dict = None, **kwargs) -> "TileMap":
        """空のTileMapを生成する。

        Args:
            cols, rows : マップサイズ（タイル単位）
            fill       : 初期タイルID（-1=空）
        """
        data = [[fill] * cols for _ in range(rows)]
        return cls(tileset, data, tile_attrs, **kwargs)
