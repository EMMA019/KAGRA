# kagra/mapgen.py
# 手続き生成マップ
#
# 3種類の生成:
#   MapGen.town(cols, rows)      → 街（道路・建物・広場・木）
#   MapGen.dungeon(cols, rows)   → ダンジョン（部屋＋通路）
#   MapGen.field(cols, rows)     → フィールド（地形ノイズ）
#
# 出力は TileMap が直接受け取れる 2D 配列（list[list[int]]）
#
# タイルIDはKenny Tiny Townを基準にしているが
# TILE_ID_* 定数を変更するだけで任意のタイルセットに対応できる。
#
# 使い方:
#   from kagra.mapgen import MapGen, TownTiles, DungeonTiles
#
#   data = MapGen.town(30, 24, seed=42)
#   tm   = TileMap(tileset, data, ATTRS)
#
#   # または後から手で修正する
#   data[5][8] = TownTiles.SHOP_SIGN  # 看板を置く
#   tm = TileMap(tileset, data, ATTRS)

from __future__ import annotations
import random
import math
from typing import Optional


# ── タイルID定数（Tiny Town 準拠） ───────────────────────────
# 実際の値はタイルセット画像に合わせて変更すること

class TownTiles:
    EMPTY        = -1
    GROUND       = 0    # 地面（草）
    ROAD_H       = 1    # 横道
    ROAD_V       = 2    # 縦道
    ROAD_CROSS   = 3    # 交差点
    ROAD_CORNER_TL = 4
    ROAD_CORNER_TR = 5
    ROAD_CORNER_BL = 6
    ROAD_CORNER_BR = 7
    WALL_H       = 10   # 建物の壁（横）
    WALL_V       = 11   # 建物の壁（縦）
    WALL_CORNER  = 12
    FLOOR        = 13   # 建物内床
    DOOR         = 14   # ドア
    TREE         = 20   # 木
    TREE2        = 21
    WELL         = 22   # 井戸
    SIGN         = 23   # 看板
    WATER        = 30   # 水
    WATER_EDGE_T = 31
    FENCE        = 40   # フェンス
    PATH         = 50   # 石畳


class DungeonTiles:
    EMPTY  = -1
    FLOOR  = 0
    WALL   = 1
    DOOR   = 2
    STAIR_DOWN = 3
    STAIR_UP   = 4
    CHEST  = 5
    TRAP   = 6


class FieldTiles:
    GRASS   = 0
    FOREST  = 1
    WATER   = 2
    MOUNTAIN = 3
    SAND    = 4
    ROAD    = 5


# ── ユーティリティ ────────────────────────────────────────────

def _empty_grid(cols: int, rows: int, fill: int = -1) -> list[list[int]]:
    return [[fill] * cols for _ in range(rows)]


def _in_bounds(grid, col, row) -> bool:
    return 0 <= row < len(grid) and 0 <= col < len(grid[0])


def _fill_rect(grid, x, y, w, h, tile: int):
    for r in range(y, y + h):
        for c in range(x, x + w):
            if _in_bounds(grid, c, r):
                grid[r][c] = tile


def _border_rect(grid, x, y, w, h, wall: int, floor: int):
    """矩形の枠を wall、内部を floor で塗る。"""
    _fill_rect(grid, x, y, w, h, floor)
    for c in range(x, x + w):
        if _in_bounds(grid, c, y):       grid[y][c]       = wall
        if _in_bounds(grid, c, y+h-1):  grid[y+h-1][c]   = wall
    for r in range(y, y + h):
        if _in_bounds(grid, x, r):       grid[r][x]       = wall
        if _in_bounds(grid, x+w-1, r):  grid[r][x+w-1]   = wall


# ── MapGen ────────────────────────────────────────────────────

class MapGen:
    """手続き生成エントリポイント。"""

    # ── 街 ────────────────────────────────────────────────────

    @staticmethod
    def town(
        cols:  int = 30,
        rows:  int = 24,
        seed:  Optional[int] = None,
        style: str = "village",   # "village" | "town" | "castle_town"
    ) -> list[list[int]]:
        """街マップを生成する。

        生成ルール:
        1. 外周を草で埋める
        2. 主要道路を格子状に引く
        3. 道路で区切られたブロックに建物をランダム配置
        4. 広場・木・装飾を追加
        5. 境界付近にフェンス・水辺を追加

        Args:
            style: "village"=小村 / "town"=街 / "castle_town"=城下町
        Returns:
            2D tile_id array
        """
        rng = random.Random(seed)
        T = TownTiles
        g = _empty_grid(cols, rows, T.GROUND)

        # ── 外壁フェンス ─────────────────────────────────────
        for c in range(cols):
            g[0][c] = T.FENCE; g[rows-1][c] = T.FENCE
        for r in range(rows):
            g[r][0] = T.FENCE; g[r][cols-1] = T.FENCE

        # ── 主要道路 ─────────────────────────────────────────
        # 縦横に道路を引く（格子間隔はスタイルで変える）
        road_interval = {"village": 8, "town": 7, "castle_town": 6}[style]

        h_roads = list(range(4, rows - 2, road_interval))
        v_roads = list(range(4, cols - 2, road_interval))

        for r in h_roads:
            for c in range(1, cols - 1):
                g[r][c] = T.ROAD_H
        for vc in v_roads:
            for r in range(1, rows - 1):
                g[r][vc] = T.ROAD_V
        # 交差点
        for r in h_roads:
            for vc in v_roads:
                if _in_bounds(g, vc, r):
                    g[r][vc] = T.ROAD_CROSS

        # ── 入口の道（南）─────────────────────────────────────
        mid_c = cols // 2
        for r in range(rows - 2, rows):
            g[r][mid_c]   = T.ROAD_V
            g[r][mid_c-1] = T.ROAD_V

        # ── ブロックに建物を配置 ──────────────────────────────
        # 道路で囲まれたブロックを列挙
        blocks = _get_blocks(h_roads, v_roads, rows, cols)
        for block in blocks:
            _place_buildings_in_block(g, block, rng, T, style)

        # ── 広場（中心付近）──────────────────────────────────
        plaza_r = rows // 2
        plaza_c = cols // 2
        _place_plaza(g, plaza_c - 3, plaza_r - 3, 6, 6, rng, T)

        # ── 木・装飾 ─────────────────────────────────────────
        _scatter_decoration(g, rng, T, density=0.04)

        return g

    # ── ダンジョン ────────────────────────────────────────────

    @staticmethod
    def dungeon(
        cols:     int = 40,
        rows:     int = 30,
        seed:     Optional[int] = None,
        min_rooms: int = 5,
        max_rooms: int = 12,
        floor:    int = 0,    # 階数（深いほど敵が強い）
    ) -> tuple[list[list[int]], list[tuple[int,int]], tuple[int,int], tuple[int,int]]:
        """ダンジョンを生成する。

        Returns:
            (grid, room_centers, stair_up_pos, stair_down_pos)
            room_centers: 各部屋の中心座標リスト（NPCやアイテム配置に使う）
            stair_up_pos: 上り階段の位置
            stair_down_pos: 下り階段の位置
        """
        rng = random.Random(seed)
        T = DungeonTiles
        g = _empty_grid(cols, rows, T.WALL)

        rooms: list[tuple[int,int,int,int]] = []  # (x, y, w, h)
        num_rooms = rng.randint(min_rooms, max_rooms)

        # ── 部屋を生成 ────────────────────────────────────────
        attempts = num_rooms * 10
        for _ in range(attempts):
            if len(rooms) >= num_rooms:
                break
            rw = rng.randint(4, min(10, cols // 4))
            rh = rng.randint(4, min(8,  rows // 4))
            rx = rng.randint(1, cols - rw - 2)
            ry = rng.randint(1, rows - rh - 2)
            # 重複チェック
            overlap = any(
                rx < ex + ew + 2 and rx + rw + 2 > ex and
                ry < ey + eh + 2 and ry + rh + 2 > ey
                for ex, ey, ew, eh in rooms
            )
            if not overlap:
                _fill_rect(g, rx, ry, rw, rh, T.FLOOR)
                rooms.append((rx, ry, rw, rh))

        # ── 部屋を通路で接続 ─────────────────────────────────
        centers = [(x + w//2, y + h//2) for x,y,w,h in rooms]
        for i in range(len(centers) - 1):
            _carve_corridor(g, centers[i], centers[i+1], T.FLOOR, rng)

        # ── ドア配置 ─────────────────────────────────────────
        for rx, ry, rw, rh in rooms:
            cx, cy = rx + rw//2, ry + rh//2
            # 部屋の端にドアを置く確率
            door_candidates = [
                (rx, cy), (rx+rw-1, cy),   # 左右
                (cx, ry), (cx, ry+rh-1),   # 上下
            ]
            for dc, dr in door_candidates:
                if _in_bounds(g, dc, dr) and rng.random() < 0.5:
                    g[dr][dc] = T.DOOR

        # ── 階段 ─────────────────────────────────────────────
        if len(rooms) >= 2:
            # 最初の部屋に上り、最後の部屋に下り
            sx, sy, sw, sh = rooms[0]
            g[sy + sh//2][sx + sw//2] = T.STAIR_UP

            ex, ey, ew, eh = rooms[-1]
            g[ey + eh//2][ex + ew//2] = T.STAIR_DOWN

            stair_up   = (sx + sw//2, sy + sh//2)
            stair_down = (ex + ew//2, ey + eh//2)
        else:
            stair_up = stair_down = (cols//2, rows//2)

        # ── 宝箱（部屋の隅に低確率）──────────────────────────
        for rx, ry, rw, rh in rooms[1:]:  # 最初の部屋以外
            if rng.random() < 0.4:
                cx = rng.choice([rx+1, rx+rw-2])
                cy = rng.choice([ry+1, ry+rh-2])
                if _in_bounds(g, cx, cy):
                    g[cy][cx] = T.CHEST

        return g, centers, stair_up, stair_down

    # ── フィールド ────────────────────────────────────────────

    @staticmethod
    def field(
        cols:     int = 50,
        rows:     int = 40,
        seed:     Optional[int] = None,
        water_ratio:  float = 0.15,
        forest_ratio: float = 0.25,
        mountain_ratio: float = 0.10,
    ) -> list[list[int]]:
        """フィールドマップをノイズで生成する。

        簡易パーリンノイズ風の実装。
        水辺・森・山・草原をそれぞれの比率で配置する。
        """
        rng = random.Random(seed)
        T   = FieldTiles

        # 簡易ノイズマップ生成
        noise = _simple_noise(cols, rows, rng, octaves=4, persistence=0.5)

        g = _empty_grid(cols, rows, T.GRASS)
        for r in range(rows):
            for c in range(cols):
                v = noise[r][c]
                if v < water_ratio:
                    g[r][c] = T.WATER
                elif v < water_ratio + 0.05:
                    g[r][c] = T.SAND      # 水辺の砂浜
                elif v > 1.0 - mountain_ratio:
                    g[r][c] = T.MOUNTAIN
                elif v > 1.0 - mountain_ratio - forest_ratio:
                    g[r][c] = T.FOREST
                else:
                    g[r][c] = T.GRASS

        # 道（南北に1本）
        road_c = cols // 2
        for r in range(rows):
            if g[r][road_c] not in (T.WATER, T.MOUNTAIN):
                g[r][road_c] = T.ROAD

        return g


# ── 内部ヘルパー ──────────────────────────────────────────────

def _get_blocks(
    h_roads: list[int],
    v_roads: list[int],
    rows: int, cols: int,
) -> list[tuple[int,int,int,int]]:
    """道路で区切られたブロックの座標リストを返す。"""
    blocks = []
    row_edges = [1] + h_roads + [rows - 1]
    col_edges = [1] + v_roads + [cols - 1]
    for i in range(len(row_edges) - 1):
        for j in range(len(col_edges) - 1):
            y0 = row_edges[i] + 1
            y1 = row_edges[i+1]
            x0 = col_edges[j] + 1
            x1 = col_edges[j+1]
            w = x1 - x0 - 1
            h = y1 - y0 - 1
            if w >= 3 and h >= 3:
                blocks.append((x0, y0, w, h))
    return blocks


def _place_buildings_in_block(
    g, block: tuple, rng: random.Random, T, style: str
):
    bx, by, bw, bh = block
    # ブロックを小分けして建物を置く
    attempts = 6
    placed = []
    for _ in range(attempts):
        ww = rng.randint(3, min(5, bw - 1))
        wh = rng.randint(3, min(4, bh - 1))
        wx = rng.randint(bx, bx + bw - ww)
        wy = rng.randint(by, by + bh - wh)
        # 重複チェック
        overlap = any(
            wx < px + pw + 1 and wx + ww + 1 > px and
            wy < py + ph + 1 and wy + wh + 1 > py
            for px, py, pw, ph in placed
        )
        if not overlap and ww >= 3 and wh >= 3:
            _draw_building(g, wx, wy, ww, wh, rng, T)
            placed.append((wx, wy, ww, wh))


def _draw_building(g, x, y, w, h, rng, T):
    """建物1棟を描画する。"""
    _border_rect(g, x, y, w, h, T.WALL_H, T.FLOOR)
    # コーナー
    if _in_bounds(g, x, y):       g[y][x]       = T.WALL_CORNER
    if _in_bounds(g, x+w-1, y):   g[y][x+w-1]   = T.WALL_CORNER
    if _in_bounds(g, x, y+h-1):   g[y+h-1][x]   = T.WALL_CORNER
    if _in_bounds(g, x+w-1, y+h-1): g[y+h-1][x+w-1] = T.WALL_CORNER
    # 縦壁
    for r in range(y+1, y+h-1):
        if _in_bounds(g, x, r):     g[r][x]     = T.WALL_V
        if _in_bounds(g, x+w-1, r): g[r][x+w-1] = T.WALL_V
    # ドア（南側中央）
    door_c = x + w // 2
    door_r = y + h - 1
    if _in_bounds(g, door_c, door_r):
        g[door_r][door_c] = T.DOOR


def _place_plaza(g, x, y, w, h, rng, T):
    """広場を配置する（石畳＋装飾）。"""
    for r in range(y, y + h):
        for c in range(x, x + w):
            if _in_bounds(g, c, r):
                g[r][c] = T.PATH
    # 中心に井戸
    cx, cy = x + w//2, y + h//2
    if _in_bounds(g, cx, cy):
        g[cy][cx] = T.WELL
    # 四隅に木
    for dc, dr in [(-1,-1),(w,-1),(-1,h),(w,h)]:
        tc, tr = x+dc, y+dr
        if _in_bounds(g, tc, tr) and g[tr][tc] == T.GROUND:
            g[tr][tc] = T.TREE


def _scatter_decoration(g, rng, T, density: float = 0.04):
    """地面タイルをランダムに木や草で飾る。"""
    rows = len(g); cols = len(g[0])
    for r in range(1, rows - 1):
        for c in range(1, cols - 1):
            if g[r][c] == T.GROUND and rng.random() < density:
                g[r][c] = rng.choice([T.TREE, T.TREE2])


def _carve_corridor(
    g, a: tuple[int,int], b: tuple[int,int],
    floor_tile: int, rng: random.Random
):
    """2点間をL字通路で接続する。"""
    ax, ay = a; bx, by = b
    # ランダムにL字の折れ方を決める
    if rng.random() < 0.5:
        # 横→縦
        for c in range(min(ax,bx), max(ax,bx)+1):
            if _in_bounds(g, c, ay): g[ay][c] = floor_tile
        for r in range(min(ay,by), max(ay,by)+1):
            if _in_bounds(g, bx, r): g[r][bx] = floor_tile
    else:
        # 縦→横
        for r in range(min(ay,by), max(ay,by)+1):
            if _in_bounds(g, ax, r): g[r][ax] = floor_tile
        for c in range(min(ax,bx), max(ax,bx)+1):
            if _in_bounds(g, c, by): g[by][c] = floor_tile


def _simple_noise(
    cols: int, rows: int, rng: random.Random,
    octaves: int = 4, persistence: float = 0.5,
) -> list[list[float]]:
    """簡易フラクタルノイズ（パーリン風）を生成する。
    値域は 0.0〜1.0 に正規化される。
    """
    result = [[0.0] * cols for _ in range(rows)]
    amp = 1.0
    total_amp = 0.0
    freq = 1

    for _ in range(octaves):
        # このオクターブの格子点をランダムに生成
        gw = max(2, cols // freq + 2)
        gh = max(2, rows // freq + 2)
        grid = [[rng.random() for _ in range(gw)] for _ in range(gh)]

        for r in range(rows):
            for c in range(cols):
                # バイリニア補間
                fx = (c / cols) * (gw - 1)
                fy = (r / rows) * (gh - 1)
                ix, iy = int(fx), int(fy)
                tx, ty = fx - ix, fy - iy
                ix = min(ix, gw - 2); iy = min(iy, gh - 2)
                v = (grid[iy][ix]     * (1-tx) * (1-ty) +
                     grid[iy][ix+1]   * tx      * (1-ty) +
                     grid[iy+1][ix]   * (1-tx)  * ty     +
                     grid[iy+1][ix+1] * tx       * ty)
                result[r][c] += v * amp

        total_amp += amp
        amp *= persistence
        freq *= 2

    # 正規化
    for r in range(rows):
        for c in range(cols):
            result[r][c] /= total_amp

    return result
