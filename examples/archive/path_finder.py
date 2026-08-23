"""
path_finder.py - A*経路探索システム（Python実装）

エンジンに不足している経路探索機能をPython側で実装します。
TODO: エンジン側に経路探索システムを実装し、Pythonから呼び出せるようにする
"""

import heapq
from typing import List, Tuple, Optional


class PathFinder:
    """A*アルゴリズムを使用した経路探索クラス"""
    
    def __init__(self, tilemap_data: List[List[int]], 
                 walkable_tiles: List[int] = [0, 2, 3]):
        """
        初期化
        
        Args:
            tilemap_data: 2次元リストのタイルマップデータ
            walkable_tiles: 通行可能なタイルIDのリスト
        """
        self.tilemap = tilemap_data
        self.walkable_tiles = set(walkable_tiles)
        self.height = len(tilemap_data)
        self.width = len(tilemap_data[0]) if self.height > 0 else 0
        
    def is_walkable(self, x: int, y: int) -> bool:
        """指定位置が通行可能かチェック"""
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.tilemap[y][x] in self.walkable_tiles
        return False
    
    def heuristic(self, a: Tuple[int, int], b: Tuple[int, int]) -> float:
        """マンハッタン距離ヒューリスティック"""
        return abs(a[0] - b[0]) + abs(a[1] - b[1])
    
    def get_neighbors(self, pos: Tuple[int, int]) -> List[Tuple[int, int]]:
        """4方向の隣接セルを取得（斜め移動なし）"""
        x, y = pos
        neighbors = []
        
        # 上下左右
        for dx, dy in [(0, -1), (1, 0), (0, 1), (-1, 0)]:
            nx, ny = x + dx, y + dy
            if self.is_walkable(nx, ny):
                neighbors.append((nx, ny))
                
        return neighbors
    
    def find_path(self, start: Tuple[int, int], goal: Tuple[int, int]) -> Optional[List[Tuple[int, int]]]:
        """
        A*アルゴリズムで経路を探索
        
        Args:
            start: 開始位置 (x, y)
            goal: 目標位置 (x, y)
            
        Returns:
            経路の座標リスト（開始位置を含む）、経路がない場合はNone
        """
        # 開始位置と目標位置が有効かチェック
        if not self.is_walkable(*start) or not self.is_walkable(*goal):
            return None
            
        # 同じ位置の場合は空の経路を返す
        if start == goal:
            return [start]
        
        # オープンリスト（優先度付きキュー）
        open_set = []
        heapq.heappush(open_set, (0, start))
        
        # 各ノードへの最適なコスト
        g_score = {start: 0}
        
        # 各ノードの親ノードを記録
        came_from = {}
        
        # 各ノードの推定コスト
        f_score = {start: self.heuristic(start, goal)}
        
        while open_set:
            current_f, current = heapq.heappop(open_set)
            
            if current == goal:
                # 経路を再構築
                path = []
                while current in came_from:
                    path.append(current)
                    current = came_from[current]
                path.append(start)
                path.reverse()
                return path
            
            for neighbor in self.get_neighbors(current):
                # 各移動のコスト（基本は1）
                tentative_g = g_score[current] + 1
                
                if neighbor not in g_score or tentative_g < g_score[neighbor]:
                    # この経路がより良い
                    came_from[neighbor] = current
                    g_score[neighbor] = tentative_g
                    f = tentative_g + self.heuristic(neighbor, goal)
                    f_score[neighbor] = f
                    
                    # まだオープンリストにない場合のみ追加
                    if neighbor not in [i[1] for i in open_set]:
                        heapq.heappush(open_set, (f, neighbor))
        
        # 経路が見つからない
        return None
    
    def find_path_pixel(self, start_pixel: Tuple[float, float], 
                       goal_pixel: Tuple[float, float],
                       tile_size: int = 48) -> Optional[List[Tuple[float, float]]]:
        """
        ピクセル座標から経路を探索
        
        Args:
            start_pixel: 開始ピクセル座標 (x, y)
            goal_pixel: 目標ピクセル座標 (x, y)
            tile_size: タイルのサイズ（ピクセル）
            
        Returns:
            ピクセル座標の経路リスト
        """
        # ピクセル座標をタイル座標に変換
        start_tile = (int(start_pixel[0] // tile_size), 
                     int(start_pixel[1] // tile_size))
        goal_tile = (int(goal_pixel[0] // tile_size), 
                    int(goal_pixel[1] // tile_size))
        
        # タイル座標で経路探索
        tile_path = self.find_path(start_tile, goal_tile)
        
        if not tile_path:
            return None
        
        # タイル座標をピクセル座標に変換（タイルの中心）
        pixel_path = []
        for tile_x, tile_y in tile_path:
            pixel_x = tile_x * tile_size + tile_size // 2
            pixel_y = tile_y * tile_size + tile_size // 2
            pixel_path.append((pixel_x, pixel_y))
            
        return pixel_path


def create_simple_map(width: int = 20, height: int = 15) -> List[List[int]]:
    """
    シンプルなテスト用マップを作成
    
    タイルID:
        0: グラス（タワー設置可能）
        1: パス（道）
        2: スポーン地点
        3: ベース
        
    Returns:
        2次元リストのマップデータ
    """
    # 全てグラスで初期化
    map_data = [[0 for _ in range(width)] for _ in range(height)]
    
    # スポーン地点（左中央）
    spawn_x, spawn_y = 2, height // 2
    map_data[spawn_y][spawn_x] = 2
    
    # ベース（右中央）
    base_x, base_y = width - 3, height // 2
    map_data[base_y][base_x] = 3
    
    # 水平なパスを作成
    for x in range(spawn_x + 1, base_x):
        map_data[spawn_y][x] = 1
    
    return map_data


if __name__ == "__main__":
    # テストコード
    print("PathFinder テスト実行...")
    
    # テストマップ作成
    test_map = create_simple_map(20, 15)
    
    # PathFinderインスタンス作成
    finder = PathFinder(test_map, walkable_tiles=[0, 1, 2, 3])
    
    # 経路探索テスト
    start = (2, 7)  # スポーン地点
    goal = (17, 7)  # ベース
    
    path = finder.find_path(start, goal)
    
    if path:
        print(f"経路が見つかりました: {len(path)}ステップ")
        for i, (x, y) in enumerate(path):
            print(f"  {i}: ({x}, {y})")
    else:
        print("経路が見つかりませんでした")