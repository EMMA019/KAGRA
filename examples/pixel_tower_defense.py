"""
pixel_tower_defense.py - Pixel Tower Defense ゲーム

KAGRAエンジンを使用した2Dタワーディフェンスゲーム。
エンジンに不足している機能はPython側で実装し、エンジン移行推奨コメントを記載します。
"""

import math
import random
import kagra
from kagra.tilemap import TileSet, TileMap, TILE_SOLID
from kagra.ui import Panel, Label, Button, ProgressBar, VBox, HBox
from kagra.effects import EffectManager
from kagra.entity import Entity, World, Transform, SpriteRenderer, Script
from kagra.physics import TopDownPhysicsSystem

# モジュールインポート
from path_finder import PathFinder, create_simple_map
from wave_manager import WaveManager

# =============================================================================
# 定数定義
# =============================================================================

# 画面サイズ
SW, SH = 1280, 720
FPS = 60

# タイルサイズ
TILE_SIZE = 48
MAP_WIDTH = 20
MAP_HEIGHT = 15

# タイルID
TILE_GRASS = 0      # グラス（タワー設置可能）
TILE_PATH = 1       # パス（道）
TILE_SPAWN = 2      # スポーン地点
TILE_BASE = 3       # ベース（本拠地）

# 色定義
COLOR_GRASS = (100, 180, 100)      # 緑
COLOR_PATH = (150, 150, 150)       # グレー
COLOR_SPAWN = (100, 100, 255)      # 青
COLOR_BASE = (255, 100, 100)       # 赤
COLOR_TOWER_RANGE = (255, 255, 0, 50)  # 黄色（半透明）

# ゲームバランス
INITIAL_COINS = 100
BASE_HP = 100

# =============================================================================
# タワー関連クラス
# =============================================================================

class Tower:
    """タワークラス（Python側実装）"""
    
    # TODO: エンジン側にタワーコンポーネントシステムを実装
    
    def __init__(self, tower_type: str, x: float, y: float):
        self.type = tower_type
        self.x = x
        self.y = y
        self.level = 1
        self.attack_timer = 0.0
        self.target = None
        
        # タワータイプごとの基本ステータス
        self.stats = self._get_base_stats()
        
    def _get_base_stats(self):
        """タワータイプごとの基本ステータスを取得"""
        stats = {
            "arrow": {
                "cost": 20,
                "damage": 10,
                "attack_speed": 1.0,  # 回/秒
                "range": 150,
                "projectile_speed": 300,
                "color": (200, 150, 100),  # 茶色
                "upgrade_cost": 30,
            },
            "magic": {
                "cost": 40,
                "damage": 15,
                "attack_speed": 0.8,
                "range": 120,
                "projectile_speed": 200,
                "splash_radius": 20,
                "color": (150, 100, 255),  # 紫色
                "upgrade_cost": 50,
            },
            "cannon": {
                "cost": 60,
                "damage": 25,
                "attack_speed": 0.5,
                "range": 180,
                "projectile_speed": 150,
                "slow_effect": 0.3,  # 30%減速
                "color": (255, 100, 100),  # 赤色
                "upgrade_cost": 70,
            }
        }
        return stats.get(self.type, stats["arrow"]).copy()
    
    def can_attack(self, dt: float) -> bool:
        """攻撃可能かチェック"""
        self.attack_timer += dt
        attack_interval = 1.0 / self.stats["attack_speed"]
        
        if self.attack_timer >= attack_interval:
            self.attack_timer = 0.0
            return True
        return False
    
    def upgrade(self) -> bool:
        """タワーをアップグレード"""
        if self.level >= 3:
            return False
        
        self.level += 1
        
        # レベルアップによるステータス向上
        if self.level == 2:
            self.stats["damage"] *= 1.5
            self.stats["range"] *= 1.1
        elif self.level == 3:
            self.stats["damage"] *= 2.0  # レベル2からの倍増
            if "splash_radius" in self.stats:
                self.stats["splash_radius"] *= 1.5
            if "slow_effect" in self.stats:
                self.stats["slow_effect"] = 0.5  # 50%減速
        
        return True
    
    def get_upgrade_cost(self) -> int:
        """アップグレードコストを取得"""
        if self.level == 1:
            return self.stats["upgrade_cost"]
        elif self.level == 2:
            return int(self.stats["upgrade_cost"] * 1.5)
        return 0  # 最大レベル
    
    def is_in_range(self, target_x: float, target_y: float) -> bool:
        """ターゲットが射程内かチェック"""
        dx = target_x - self.x
        dy = target_y - self.y
        distance = math.sqrt(dx*dx + dy*dy)
        return distance <= self.stats["range"]


class Projectile:
    """弾クラス（Python側実装）"""
    
    # TODO: エンジン側に弾道物理システムを統合
    
    def __init__(self, x: float, y: float, target_x: float, target_y: float, 
                 speed: float, damage: int, tower_type: str):
        self.x = x
        self.y = y
        self.target_x = target_x
        self.target_y = target_y
        self.speed = speed
        self.damage = damage
        self.tower_type = tower_type
        self.active = True
        
        # 発射方向を計算
        dx = target_x - x
        dy = target_y - y
        distance = math.sqrt(dx*dx + dy*dy)
        
        if distance > 0:
            self.vx = (dx / distance) * speed
            self.vy = (dy / distance) * speed
        else:
            self.vx = 0
            self.vy = 0
    
    def update(self, dt: float) -> bool:
        """弾の更新。ターゲットに命中したらTrueを返す"""
        if not self.active:
            return False
        
        self.x += self.vx * dt
        self.y += self.vy * dt
        
        # ターゲットまでの距離を計算
        dx = self.target_x - self.x
        dy = self.target_y - self.y
        distance = math.sqrt(dx*dx + dy*dy)
        
        # 命中判定（近づきすぎた場合）
        if distance < 10:
            self.active = False
            return True
        
        return False


# =============================================================================
# ゲームシーン
# =============================================================================

class TowerDefenseGame(kagra.Scene):
    """メインゲームシーン"""
    
    def on_enter(self):
        """シーン初期化"""
        print("Pixel Tower Defense 開始")
        
        # フォント読み込み
        self.font = kagra.assets.font("C:/Windows/Fonts/meiryo.ttc")
        
        # ゲーム状態
        self.coins = INITIAL_COINS
        self.base_hp = BASE_HP
        self.game_over = False
        self.game_win = False
        
        # マップ作成
        self.create_map()
        
        # 経路探索システム初期化
        self.path_finder = PathFinder(self.tilemap_data, walkable_tiles=[TILE_PATH, TILE_SPAWN, TILE_BASE])
        
        # ウェーブ管理システム初期化
        self.wave_manager = WaveManager()
        self.wave_manager.start_wave(0)
        
        # ゲームオブジェクト
        self.towers = []  # タワーリスト
        self.enemies = []  # 敵リスト
        self.projectiles = []  # 弾リスト
        self.selected_tower = None  # 選択中のタワー
        
        # エフェクト管理
        self.effects = EffectManager()
        
        # UI初期化
        self.setup_ui()
        
        # エンティティワールド
        self.world = World()
        
        print(f"ゲーム初期化完了: {MAP_WIDTH}x{MAP_HEIGHT}マップ")
    
    def create_map(self):
        """マップを作成"""
        # マップデータ生成
        self.tilemap_data = create_simple_map(MAP_WIDTH, MAP_HEIGHT)
        
        # タイル属性（パスとベースは通行可能、他は通行不可）
        tile_attrs = {
            TILE_PATH: 0,  # 通行可能
            TILE_SPAWN: 0, # 通行可能
            TILE_BASE: 0,  # 通行可能
        }
        
        # スポーン位置とベース位置を検出
        self.spawn_positions = []
        self.base_position = None
        
        for y in range(MAP_HEIGHT):
            for x in range(MAP_WIDTH):
                tile = self.tilemap_data[y][x]
                if tile == TILE_SPAWN:
                    self.spawn_positions.append((x * TILE_SIZE + TILE_SIZE//2, 
                                                y * TILE_SIZE + TILE_SIZE//2))
                elif tile == TILE_BASE:
                    self.base_position = (x * TILE_SIZE + TILE_SIZE//2, 
                                         y * TILE_SIZE + TILE_SIZE//2)
        
        # タイルセット（単色のタイルを生成）
        self.tileset = self.create_color_tileset()
        
        # タイルマップ作成
        self.tilemap = TileMap(self.tileset, self.tilemap_data, tile_attrs, 
                              TILE_SIZE, TILE_SIZE)
        
        print(f"マップ作成完了: スポーン位置={len(self.spawn_positions)}, ベース位置={self.base_position}")
    
    def create_color_tileset(self):
        """色分けしたタイルセットを作成"""
        # 単色のタイルテクスチャを生成（簡易実装）
        import struct
        import zlib
        import tempfile
        import os
        
        def create_color_texture(r, g, b):
            """単色のPNGテクスチャを生成"""
            w, h = TILE_SIZE, TILE_SIZE
            rows = b""
            for _ in range(h):
                row = b"\x00"
                for _ in range(w):
                    row += bytes([r, g, b, 255])
                rows += row
            
            raw = zlib.compress(rows)
            def chunk(t, d):
                c = zlib.crc32(t + d) & 0xFFFFFFFF
                return struct.pack(">I", len(d)) + t + d + struct.pack(">I", c)
            
            png = (b"\x89PNG\r\n\x1a\n"
                   + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
                   + chunk(b"IDAT", raw) + chunk(b"IEND", b""))
            
            # 一時ファイルに保存
            import hashlib
            hash_str = hashlib.md5(f"{r}{g}{b}".encode()).hexdigest()[:8]
            temp_path = os.path.join(tempfile.gettempdir(), f"tile_{hash_str}.png")
            with open(temp_path, "wb") as f:
                f.write(png)
            
            return kagra.load(temp_path)
        
        # 4種類のタイルテクスチャを作成
        tex_grass = create_color_texture(*COLOR_GRASS)
        tex_path = create_color_texture(*COLOR_PATH)
        tex_spawn = create_color_texture(*COLOR_SPAWN)
        tex_base = create_color_texture(*COLOR_BASE)
        
        # タイルセット作成（1つのテクスチャにまとめる必要があるが、簡易的に個別に）
        # ここではグラス用のタイルセットを作成（実際のゲームではスプライトシートを使うべき）
        return TileSet(tex_grass, TILE_SIZE, TILE_SIZE)
    
    def setup_ui(self):
        """UIを設定"""
        # 右側UIパネル
        self.ui_panel = Panel(x=MAP_WIDTH * TILE_SIZE, y=0, 
                             w=SW - MAP_WIDTH * TILE_SIZE, h=SH,
                             color=(30, 30, 50, 200))
        
        # リソース表示
        self.coin_label = Label(x=MAP_WIDTH * TILE_SIZE + 20, y=20, 
                               text=f"コイン: {self.coins}", 
                               font=self.font, size=24, color=(255, 220, 100))
        
        self.wave_label = Label(x=MAP_WIDTH * TILE_SIZE + 20, y=60, 
                               text="ウェーブ: 1/5", 
                               font=self.font, size=20, color=(200, 200, 255))
        
        self.base_hp_bar = ProgressBar(x=MAP_WIDTH * TILE_SIZE + 20, y=100,
                                      w=200, h=20, max_val=BASE_HP)
        self.base_hp_bar.value = self.base_hp
        
        # タワーショップ
        shop_y = 150
        self.shop_label = Label(x=MAP_WIDTH * TILE_SIZE + 20, y=shop_y,
                               text="タワーショップ", font=self.font, size=22, 
                               color=(255, 255, 255))
        
        # タワーボタン
        self.tower_buttons = []
        tower_types = [
            ("矢塔", "arrow", 20, (200, 150, 100)),
            ("魔法塔", "magic", 40, (150, 100, 255)),
            ("砲塔", "cannon", 60, (255, 100, 100)),
        ]
        
        for i, (name, tower_type, cost, color) in enumerate(tower_types):
            btn_y = shop_y + 40 + i * 70
            
            # ボタン
            btn = Button(x=MAP_WIDTH * TILE_SIZE + 20, y=btn_y,
                        w=200, h=50, label=f"{name} ({cost}コイン)",
                        font=self.font, size=18,
                        color=color, hover_color=(color[0]+30, color[1]+30, color[2]+30),
                        on_click=lambda t=tower_type, c=cost: self.buy_tower(t, c))
            self.tower_buttons.append(btn)
            
            # 説明ラベル
            desc_labels = {
                "arrow": "基本のタワー。バランスが良い。",
                "magic": "範囲攻撃が可能。",
                "cannon": "高ダメージ、敵を減速。",
            }
            Label(x=MAP_WIDTH * TILE_SIZE + 20, y=btn_y + 55,
                  text=desc_labels[tower_type], font=self.font, size=14,
                  color=(180, 180, 180)).draw()  # 描画だけして保持しない
        
        # ゲームコントロール
        control_y = SH - 120
        self.pause_btn = Button(x=MAP_WIDTH * TILE_SIZE + 20, y=control_y,
                               w=90, h=40, label="一時停止",
                               font=self.font, size=16,
                               on_click=self.toggle_pause)
        
        self.speed_btn = Button(x=MAP_WIDTH * TILE_SIZE + 120, y=control_y,
                               w=90, h=40, label="速度x1",
                               font=self.font, size=16,
                               on_click=self.toggle_speed)
        
        # 選択中のタワー情報
        self.selected_info = Label(x=MAP_WIDTH * TILE_SIZE + 20, y=SH - 70,
                                  text="タワーを選択してください", 
                                  font=self.font, size=16, color=(200, 200, 200))
    
    def buy_tower(self, tower_type: str, cost: int):
        """タワーを購入"""
        if self.coins >= cost and not self.game_over:
            self.coins -= cost
            self.selected_tower = tower_type
            print(f"{tower_type}タワーを選択しました（クリックで設置）")
    
    def place_tower(self, x: float, y: float):
        """タワーを設置"""
        if not self.selected_tower:
            return False
        
        # タイル座標に変換
        tile_x = int(x // TILE_SIZE)
        tile_y = int(y // TILE_SIZE)
        
        # 設置可能かチェック（グラス上のみ）
        if (0 <= tile_x < MAP_WIDTH and 0 <= tile_y < MAP_HEIGHT and
            self.tilemap_data[tile_y][tile_x] == TILE_GRASS):
            
            # 他のタワーと重なっていないかチェック
            for tower in self.towers:
                tx = int(tower.x // TILE_SIZE)
                ty = int(tower.y // TILE_SIZE)
                if tx == tile_x and ty == tile_y:
                    return False
            
            # タワーを作成
            tower_x = tile_x * TILE_SIZE + TILE_SIZE // 2
            tower_y = tile_y * TILE_SIZE + TILE_SIZE // 2
            tower = Tower(self.selected_tower, tower_x, tower_y)
            self.towers.append(tower)
            
            # 選択を解除
            self.selected_tower = None
            
            print(f"タワーを設置: ({tile_x}, {tile_y})")
            return True
        
        return False
    
    def toggle_pause(self):
        """一時停止切り替え"""
        # TODO: 実装
        print("一時停止機能は実装中です")
    
    def toggle_speed(self):
        """ゲーム速度切り替え"""
        # TODO: 実装
        print("速度切り替え機能は実装中です")
    
    def update(self, dt: float):
        """ゲーム更新"""
        if self.game_over or self.game_win:
            return
        
        # ウェーブ管理
        wave_result = self.wave_manager.update(dt)
        if wave_result:
            self.spawn_enemy(wave_result)
        
        # 敵の更新
        self.update_enemies(dt)
        
        # タワーの更新
        self.update_towers(dt)
        
        # 弾の更新
        self.update_projectiles(dt)
        
        # エフェクト更新
        self.effects.update(dt)
        
        # UI更新
        self.update_ui()
        
        # マウスクリックでタワー設置
        if kagra.mouse_click(1) and self.selected_tower:
            mx, my = kagra.mouse()
            self.place_tower(mx, my)
        
        # ESCキーでゲーム終了
        if kagra.pressed("ESCAPE"):
            raise SystemExit
    
    def update_enemies(self, dt: float):
        """敵の更新"""
        for enemy in self.enemies[:]:  # コピーでループ（削除可能にするため）
            if not enemy.get("active", True):
                continue
            
            # 経路に沿って移動
            if "path" in enemy and enemy["path_index"] < len(enemy["path"]):
                target_x, target_y = enemy["path"][enemy["path_index"]]
                
                # ターゲットへの方向を計算
                dx = target_x - enemy["x"]
                dy = target_y - enemy["y"]
                distance = math.sqrt(dx*dx + dy*dy)
                
                if distance < 5:  # ターゲットに到達
                    enemy["path_index"] += 1
                else:
                    # 移動
                    speed = enemy["stats"]["speed"] * enemy.get("slow_factor", 1.0)
                    enemy["x"] += (dx / distance) * speed * dt
                    enemy["y"] += (dy / distance) * speed * dt
            
            # ベースに到達したかチェック
            if self.base_position:
                dx = enemy["x"] - self.base_position[0]
                dy = enemy["y"] - self.base_position[1]
                distance = math.sqrt(dx*dx + dy*dy)
                
                if distance < TILE_SIZE // 2:
                    # ベースにダメージ
                    self.base_hp -= enemy["stats"]["damage"]
                    self.enemies.remove(enemy)
                    
                    # ベースダメージエフェクト
                    self.effects.flash(r=255, g=0, b=0, duration=0.3)
                    
                    if self.base_hp <= 0:
                        self.game_over = True
                        print("ゲームオーバー: ベースが破壊されました")
                    
                    continue
            
            # HPが0以下になった敵を削除
            if enemy["hp"] <= 0:
                # コイン獲得
                self.coins += enemy["stats"]["reward"]
                
                # 死亡エフェクト
                self.effects.damage(enemy["x"], enemy["y"], enemy["stats"]["reward"])
                
                self.enemies.remove(enemy)
    
    def update_towers(self, dt: float):
        """タワーの更新と攻撃"""
        for tower in self.towers:
            # ターゲットを探す
            if not tower.target or not tower.target.get("active", True):
                tower.target = self.find_target_for_tower(tower)
            
            # 攻撃可能かチェック
            if tower.target and tower.can_attack(dt):
                # 射程内かチェック
                if tower.is_in_range(tower.target["x"], tower.target["y"]):
                    # 弾を発射
                    self.fire_projectile(tower, tower.target)
    
    def find_target_for_tower(self, tower: Tower):
        """タワーのターゲットを探す"""
        # TODO: エンジン側にAIコンポーネントシステムを追加推奨
        
        best_target = None
        best_priority = -1
        
        for enemy in self.enemies:
            if not enemy.get("active", True):
                continue
            
            # 射程内かチェック
            if tower.is_in_range(enemy["x"], enemy["y"]):
                # 優先度計算（例: HPが低い敵を優先）
                priority = 1.0 / (enemy["hp"] + 1)  # HPが低いほど優先度が高い
                
                if priority > best_priority:
                    best_priority = priority
                    best_target = enemy
        
        return best_target
    
    def fire_projectile(self, tower: Tower, target: dict):
        """弾を発射"""
        # TODO: エンジン側に弾道物理システムを統合
        
        projectile = Projectile(
            tower.x, tower.y, target["x"], target["y"],
            tower.stats["projectile_speed"], tower.stats["damage"], tower.type
        )
        self.projectiles.append(projectile)
    
    def update_projectiles(self, dt: float):
        """弾の更新"""
        for projectile in self.projectiles[:]:  # コピーでループ
            if not projectile.active:
                self.projectiles.remove(projectile)
                continue
            
            # 弾の移動
            hit = projectile.update(dt)
            
            if hit:
                # 命中処理
                for enemy in self.enemies:
                    if not enemy.get("active", True):
                        continue
                    
                    # 命中判定（簡易版）
                    dx = enemy["x"] - projectile.x
                    dy = enemy["y"] - projectile.y
                    distance = math.sqrt(dx*dx + dy*dy)
                    
                    if distance < 20:  # 命中半径
                        # ダメージ計算
                        damage = projectile.damage
                        
                        # タワータイプに応じた効果
                        if projectile.tower_type == "magic":
                            # 魔法塔: 範囲ダメージ
                            for other_enemy in self.enemies:
                                if other_enemy is enemy:
                                    continue
                                
                                odx = other_enemy["x"] - projectile.x
                                ody = other_enemy["y"] - projectile.y
                                odistance = math.sqrt(odx*odx + ody*ody)
                                
                                if odistance < 20:  # スプラッシュ半径
                                    other_enemy["hp"] -= damage * 0.5  # 範囲ダメージは50%
                        
                        elif projectile.tower_type == "cannon":
                            # 砲塔: 減速効果
                            enemy["slow_factor"] = 0.7  # 30%減速
                        
                        # ダメージ適用
                        enemy["hp"] -= damage
                        
                        # ダメージエフェクト
                        self.effects.damage(enemy["x"], enemy["y"], damage)
                        
                        projectile.active = False
                        break
        
        # 非アクティブな弾を削除
        self.projectiles = [p for p in self.projectiles if p.active]
    
    def spawn_enemy(self, enemy_data: dict):
        """敵をスポーン"""
        if not self.spawn_positions:
            return
        
        # スポーン位置を選択
        spawn_x, spawn_y = random.choice(self.spawn_positions)
        
        # 経路を計算
        path = self.path_finder.find_path_pixel(
            (spawn_x, spawn_y), self.base_position, TILE_SIZE
        )
        
        if not path:
            print("警告: 敵の経路が見つかりません")
            return
        
        # 敵を作成
        enemy = {
            "type": enemy_data["type"],
            "stats": enemy_data["stats"],
            "x": spawn_x,
            "y": spawn_y,
            "hp": enemy_data["stats"]["hp"],
            "path": path,
            "path_index": 0,
            "active": True,
            "slow_factor": 1.0,  # 減速効果係数（1.0 = 通常）
        }
        
        self.enemies.append(enemy)
        print(f"敵スポーン: {enemy_data['type']} (HP: {enemy['hp']})")
    
    def update_ui(self):
        """UI情報を更新"""
        # コイン表示
        self.coin_label.text = f"コイン: {self.coins}"
        
        # ウェーブ情報
        wave_info = self.wave_manager.get_wave_info()
        wave_text = f"ウェーブ: {wave_info['wave_number']}/{wave_info['total_waves']}"
        if wave_info['enemies_remaining'] > 0:
            wave_text += f" (残り: {wave_info['enemies_remaining']})"
        self.wave_label.text = wave_text
        
        # ベースHP
        self.base_hp_bar.value = self.base_hp
        
        # 選択中のタワー情報
        if self.selected_tower:
            self.selected_info.text = f"{self.selected_tower}タワーを選択中（クリックで設置）"
            self.selected_info.color = (255, 255, 100)
        else:
            self.selected_info.text = "タワーを選択またはクリックで選択解除"
            self.selected_info.color = (200, 200, 200)
        
        # ボタンの有効/無効切り替え
        for btn in self.tower_buttons:
            # 簡易的に常に有効
            pass
    
    def draw(self):
        """描画"""
        # 背景
        kagra.cls(20, 25, 35)
        
        # タイルマップ描画
        self.draw_tilemap()
        
        # タワー描画
        self.draw_towers()
        
        # 敵描画
        self.draw_enemies()
        
        # 弾描画
        self.draw_projectiles()
        
        # 選択中のタワー範囲表示
        if self.selected_tower:
            mx, my = kagra.mouse()
            self.draw_tower_range(mx, my)
        
        # エフェクト描画
        self.effects.draw()
        
        # UI描画
        self.draw_ui()
        
        # ゲームオーバー/クリア表示
        self.draw_game_status()
    
    def draw_tilemap(self):
        """タイルマップを描画（色分け）"""
        # 簡易的な色分け描画
        for y in range(MAP_HEIGHT):
            for x in range(MAP_WIDTH):
                tile = self.tilemap_data[y][x]
                
                # タイルタイプに応じた色
                if tile == TILE_GRASS:
                    color = COLOR_GRASS
                elif tile == TILE_PATH:
                    color = COLOR_PATH
                elif tile == TILE_SPAWN:
                    color = COLOR_SPAWN
                elif tile == TILE_BASE:
                    color = COLOR_BASE
                else:
                    color = COLOR_GRASS
                
                # 矩形描画
                kagra.fill(x * TILE_SIZE, y * TILE_SIZE, 
                          TILE_SIZE, TILE_SIZE, color)
                
                # 枠線
                kagra.fill(x * TILE_SIZE, y * TILE_SIZE, 
                          TILE_SIZE, 1, (50, 50, 50))  # 上枠
                kagra.fill(x * TILE_SIZE, y * TILE_SIZE, 
                          1, TILE_SIZE, (50, 50, 50))  # 左枠
    
    def draw_towers(self):
        """タワーを描画"""
        for tower in self.towers:
            # タワー本体
            size = TILE_SIZE // 2
            kagra.fill(tower.x - size//2, tower.y - size//2, 
                      size, size, tower.stats["color"])
            
            # レベル表示
            if tower.level > 1:
                level_text = str(tower.level)
                tw, th = kagra.measure(level_text, 12, self.font)
                kagra.text(level_text, tower.x - tw//2, tower.y - th//2,
                          12, (255, 255, 255), self.font)
            
            # 選択中のタワーは範囲表示
            # （ここでは選択機能は実装しない）
    
    def draw_tower_range(self, x: float, y: float):
        """タワーの射程範囲を表示"""
        # 設置予定位置に範囲を表示
        tower_type = self.selected_tower
        if not tower_type:
            return
        
        # 仮のタワーで範囲を取得
        temp_tower = Tower(tower_type, x, y)
        range_radius = temp_tower.stats["range"]
        
        # 範囲円を描画（簡易的なドットで）
        steps = 32
        for i in range(steps):
            angle = (i / steps) * 2 * math.pi
            px = x + math.cos(angle) * range_radius
            py = y + math.sin(angle) * range_radius
            kagra.fill(int(px), int(py), 2, 2, COLOR_TOWER_RANGE)
    
    def draw_enemies(self):
        """敵を描画"""
        for enemy in self.enemies:
            if not enemy.get("active", True):
                continue
            
            stats = enemy["stats"]
            size = TILE_SIZE // 2
            
            # 敵本体
            kagra.fill(enemy["x"] - size//2, enemy["y"] - size//2, 
                      size, size, stats["color"])
            
            # HPバー
            hp_ratio = enemy["hp"] / stats["hp"]
            hp_width = size * hp_ratio
            hp_color = (100, 200, 100) if hp_ratio > 0.5 else (
                (255, 200, 0) if hp_ratio > 0.25 else (255, 50, 50)
            )
            
            kagra.fill(enemy["x"] - size//2, enemy["y"] - size//2 - 8, 
                      hp_width, 4, hp_color)
            
            # HP枠
            kagra.fill(enemy["x"] - size//2, enemy["y"] - size//2 - 8, 
                      size, 1, (50, 50, 50))  # 上枠
            kagra.fill(enemy["x"] - size//2, enemy["y"] - size//2 - 4, 
                      1, 4, (50, 50, 50))  # 左枠
            kagra.fill(enemy["x"] + size//2, enemy["y"] - size//2 - 4, 
                      1, 4, (50, 50, 50))  # 右枠
    
    def draw_projectiles(self):
        """弾を描画"""
        for projectile in self.projectiles:
            if not projectile.active:
                continue
            
            # 弾の種類に応じた色
            if projectile.tower_type == "arrow":
                color = (200, 150, 100)  # 茶色
                size = 4
            elif projectile.tower_type == "magic":
                color = (150, 100, 255)  # 紫色
                size = 6
            else:  # cannon
                color = (255, 100, 100)  # 赤色
                size = 8
            
            kagra.fill(int(projectile.x) - size//2, int(projectile.y) - size//2, 
                      size, size, color)
    
    def draw_ui(self):
        """UIを描画"""
        # UIパネル
        self.ui_panel.draw()
        
        # ラベルとボタン
        self.coin_label.draw()
        self.wave_label.draw()
        self.base_hp_bar.draw()
        self.shop_label.draw()
        
        for btn in self.tower_buttons:
            btn.draw()
        
        # ゲームコントロール
        self.pause_btn.draw()
        self.speed_btn.draw()
        
        # 選択情報
        self.selected_info.draw()
        
        # コントロール説明
        kagra.text("ESC: 終了 | クリック: タワー設置", 
                  MAP_WIDTH * TILE_SIZE + 20, SH - 30,
                  14, (150, 150, 150), self.font)
    
    def draw_game_status(self):
        """ゲーム状態を描画"""
        if self.game_over:
            # ゲームオーバー表示
            kagra.fill(0, 0, SW, SH, (0, 0, 0, 150))
            kagra.text("ゲームオーバー", SW//2 - 100, SH//2 - 50, 
                      48, (255, 50, 50), self.font)
            kagra.text("ESCキーで終了", SW//2 - 80, SH//2 + 20, 
                      24, (200, 200, 200), self.font)
        
        elif self.game_win:
            # ゲームクリア表示
            kagra.fill(0, 0, SW, SH, (0, 0, 0, 150))
            kagra.text("ゲームクリア！", SW//2 - 100, SH//2 - 50, 
                      48, (100, 255, 100), self.font)
            kagra.text("すべてのウェーブを防衛しました", SW//2 - 100, SH//2 + 20, 
                      24, (200, 200, 200), self.font)


# =============================================================================
# タイトルシーン
# =============================================================================

class TitleScene(kagra.Scene):
    """タイトルシーン"""
    
    def on_enter(self):
        self.font = kagra.assets.font("C:/Windows/Fonts/meiryo.ttc")
        self.timer = 0.0
    
    def update(self, dt):
        self.timer += dt
        
        if kagra.pressed("Z"):
            kagra.go(TowerDefenseGame())
        
        if kagra.pressed("ESCAPE"):
            raise SystemExit
    
    def draw(self):
        kagra.cls(25, 30, 45)
        
        # タイトル
        title = "Pixel Tower Defense"
        w, h = kagra.measure(title, 64, self.font)
        kagra.text(title, float(SW//2 - w//2), 150,
                  size=64, color=(255, 220, 100), font=self.font)
        
        # 説明
        desc = "タワーを配置して敵の侵攻を防げ！"
        w, _ = kagra.measure(desc, 28, self.font)
        kagra.text(desc, float(SW//2 - w//2), 250,
                  size=28, color=(150, 200, 255), font=self.font)
        
        # 操作方法
        controls = [
            "操作方法:",
            "1. 右側のショップからタワーを選択",
            "2. マップ上をクリックしてタワーを設置",
            "3. 敵がベースに到達する前に倒せ！",
            "",
            "Zキー: ゲームスタート",
            "ESCキー: 終了"
        ]
        
        for i, line in enumerate(controls):
            w, _ = kagra.measure(line, 20, self.font)
            kagra.text(line, float(SW//2 - w//2), 320 + i * 30,
                      size=20, color=(200, 200, 200), font=self.font)
        
        # 点滅テキスト
        if int(self.timer * 2) % 2 == 0:
            start_text = "Zキーでスタート！"
            w, _ = kagra.measure(start_text, 32, self.font)
            kagra.text(start_text, float(SW//2 - w//2), 520,
                      size=32, color=(255, 255, 200), font=self.font)


# =============================================================================
# メイン実行
# =============================================================================

if __name__ == "__main__":
    print("Pixel Tower Defense 起動中...")
    print("=" * 50)
    print("注意: このゲームは開発中です。一部機能は未実装です。")
    print("エンジンに不足している機能はPython側で実装されています。")
    print("=" * 50)
    
    kagra.init(width=SW, height=SH, title="Pixel Tower Defense", fps=FPS)
    kagra.run(start_scene=TitleScene())