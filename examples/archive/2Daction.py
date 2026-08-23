"""
KAGRA Defend the Crystal - 防衛型プラットフォーマー
===================================================
KAGRA v3 リファレンス完全準拠の1000行規模サンプルゲームです。

【遊び方】
- 目的: 中央のクリスタルを敵の群れから守り抜け！
- 移動: ← →
- ジャンプ: Z または ↑
- 攻撃（近接＆魔法）: X （MPが10以上あると自動で魔法弾も飛びます）
- ショップ（ポーズ）: ESCキーで開き、コインで能力を強化
"""

import math
import random
import os
import struct
import tempfile
import zlib

import kagra
from kagra.physics import Rigidbody, BoxCollider, PhysicsSystem
from kagra.tilemap import TileSet, TileMap, TILE_SOLID
from kagra.camera import Camera
from kagra.effects import EffectManager
from kagra.ui import ProgressBar, VBox, Button, Label, Tween, Easing
from kagra.timeline import Timeline, Track


# =============================================================================
# 1. 定数・イベント名定義
# =============================================================================
SW, SH = 1280, 720
FPS = 60

# Event Bus 用イベント名（タイポ防止のため定数化）
EV_PLAYER_DMG  = "player_damaged"
EV_ENEMY_DMG   = "enemy_damaged"
EV_CRYSTAL_DMG = "crystal_damaged"
EV_SCORE_UP    = "score_up"
EV_COIN_GET    = "coin_get"
EV_CAM_SHAKE   = "camera_shake"
EV_SPAWN_ENEMY = "spawn_enemy"
EV_GAME_OVER   = "game_over"

# 物理定数
GRAVITY = 1100.0


# =============================================================================
# 2. グローバルステート管理
# =============================================================================
class GameState:
    """ゲーム全体の進行状況とプレイヤーのステータスを保持"""
    def __init__(self):
        self.reset()

    def reset(self):
        # プレイヤー
        self.max_hp = 100
        self.hp = 100
        self.max_mp = 50
        self.mp = 50
        self.attack_power = 15
        self.speed_level = 1
        
        # クリスタル（防衛対象）
        self.crystal_max_hp = 200
        self.crystal_hp = 200
        
        # 進行状況
        self.score = 0
        self.coins = 0
        self.wave = 1
        self.game_over = False

g_state = GameState()


# =============================================================================
# 3. テクスチャ自動生成ユーティリティ
# =============================================================================
def make_tileset_png(tile_size: int, colors: list) -> int:
    """リファレンスv3準拠：Pillow不要でPNGテクスチャを生成してロードする"""
    W = tile_size * len(colors)
    H = tile_size
    rows = []
    for y in range(H):
        row = bytearray()
        for x in range(W):
            tile = x // tile_size
            r, g, b, a = colors[tile]
            # ブロック(土)の場合は少しノイズを入れる
            if tile == 2 and (x % 4 == 0 or y % 4 == 0):
                r, g, b = max(0, r-20), max(0, g-20), max(0, b-20)
            row += bytes([r, g, b, a])
        rows.append(bytes([0]) + row)

    raw = zlib.compress(b"".join(rows))

    def png_chunk(tag, data):
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    png = (b"\x89PNG\r\n\x1a\n"
           + png_chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
           + png_chunk(b"IDAT", raw)
           + png_chunk(b"IEND", b""))

    path = os.path.join(tempfile.gettempdir(), "kagra_crystal_tileset.png")
    with open(path, "wb") as f:
        f.write(png)
    return kagra.load_texture(path)


# =============================================================================
# 4. マップデータ
# =============================================================================
# 0:空, 1:草ブロック, 2:土ブロック
LEVEL_MAP = [
    [0]*40,
    [0]*40,
    [0]*40,
    [0]*40,
    [0]*40,
    [0]*40,
    [0]*40,
    [0,0,0,0,0,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,0,0,0,0,0],
    [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
    [1,1,0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,1,0,0,0,0,0,0,0,0,1,1],
    [2,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,2],
    [2,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,2],
    [2,2,1,1,1,1,1,1,1,1,1,1,1,1,1,1,2,2,2,2,2,2,2,2,1,1,1,1,1,1,1,1,1,1,1,1,1,1,2,2],
    [2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2],
    [2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2],
]

TILE_SIZE = 40
MAP_W = len(LEVEL_MAP[0]) * TILE_SIZE
MAP_H = len(LEVEL_MAP) * TILE_SIZE

# タイル属性定義
TILE_ATTRS = {
    1: TILE_SOLID,
    2: TILE_SOLID
}


# =============================================================================
# 5. Entity Scripts (ECS パターン)
# =============================================================================

class PlayerScript(kagra.Script):
    """プレイヤーの操作と状態管理"""
    def start(self):
        self.rb = self.entity.get(Rigidbody)
        self.col = self.entity.get(BoxCollider)
        self.world = self.entity.world
        
        self.time = 0.0
        self.invincible = 0.0
        self.flash = 0.0
        self.facing_right = True
        
        # 攻撃状態
        self.attack_timer = 0.0
        self.attack_rect = None  # 近接攻撃の判定矩形
        
        # イベント登録
        kagra.on(EV_PLAYER_DMG, self._on_damaged)

    def update(self, dt: float):
        self.time += dt
        self.invincible = max(0.0, self.invincible - dt)
        self.flash = max(0.0, self.flash - dt * 3.0)
        self.attack_timer = max(0.0, self.attack_timer - dt)
        self.attack_rect = None

        if g_state.game_over:
            self.rb.vx = 0
            return

        # MP自動回復
        g_state.mp = min(g_state.max_mp, g_state.mp + dt * 2.0)

        # ── 移動 ──
        speed = 220 + (g_state.speed_level * 20)
        target_vx = 0.0
        if kagra.key_down(kagra.KEY_LEFT):
            target_vx = -speed
            self.facing_right = False
        if kagra.key_down(kagra.KEY_RIGHT):
            target_vx = speed
            self.facing_right = True
            
        # 慣性付き移動 (lerp)
        self.rb.vx += (target_vx - self.rb.vx) * 15.0 * dt

        # ── ジャンプ ──
        if (kagra.key_pressed(kagra.KEY_Z) or kagra.key_pressed(kagra.KEY_UP)) and self.rb.on_ground:
            self.rb.add_impulse(0, -600)
            EffectWrapper.spark(self.entity.transform.x + 16, self.entity.transform.y + 40)

        # ── 攻撃 (X) : 近接＆魔法 ──
        if kagra.key_pressed(kagra.KEY_X) and self.attack_timer <= 0:
            self.attack_timer = 0.4
            
            # 1. 近接攻撃の判定
            ax = self.entity.transform.x + (32 if self.facing_right else -32)
            ay = self.entity.transform.y
            self.attack_rect = (ax, ay, 40, 48)
            # 少し前進
            self.rb.vx += 300 if self.facing_right else -300
            self._check_melee_attack()

            # 2. 魔法弾の発射（MPが10以上あれば自動発射）
            if g_state.mp >= 10:
                g_state.mp -= 10
                bullet = self.world.create("Bullet", tag="bullet")
                bx = self.entity.transform.x + (20 if self.facing_right else -20)
                by = self.entity.transform.y + 16
                bullet.transform.set_pos(bx, by)
                bullet.add(BulletScript(facing_right=self.facing_right, damage=g_state.attack_power * 1.5))

    def _check_melee_attack(self):
        """近接攻撃の判定処理 (他を知らなくていい疎結合)"""
        if not self.attack_rect: return
        ax, ay, aw, ah = self.attack_rect
        enemies = self.world.find_with_tag("enemy")
        for enemy in enemies:
            ex, ey = enemy.transform.x, enemy.transform.y
            # 簡易短形判定
            if kagra.collide_rect(ax, ay, aw, ah, ex, ey, 36, 36):
                # EventBusでダメージ処理を委譲（id(entity) を使用）
                kagra.emit(EV_ENEMY_DMG, {
                    "target_id": id(enemy),
                    "damage": g_state.attack_power,
                    "from_x": self.entity.transform.x,
                    "knockback": 400
                })

    def _on_damaged(self, data):
        if self.invincible > 0 or g_state.game_over:
            return
        g_state.hp -= data.get("damage", 10)
        self.invincible = 1.5
        self.flash = 1.0
        
        kd = 1 if data.get("from_x", 0) < self.entity.transform.x else -1
        self.rb.vx = kd * 350
        self.rb.vy = -300
        
        EffectWrapper.fx.flash(255, 60, 60, duration=0.2)
        kagra.emit(EV_CAM_SHAKE, {"amount": 10.0, "decay": 12.0})
        
        if g_state.hp <= 0:
            g_state.game_over = True
            kagra.emit(EV_GAME_OVER)


class BulletScript(kagra.Script):
    """魔法弾スクリプト"""
    def __init__(self, facing_right: bool, damage: float):
        super().__init__()
        self.facing_right = facing_right
        self.damage = damage

    def start(self):
        self.world = self.entity.world
        self.speed = 600.0 if self.facing_right else -600.0
        self.life = 1.5
        self.time = 0.0

    def update(self, dt: float):
        self.time += dt
        self.life -= dt
        if self.life <= 0:
            self.entity.destroy()
            return

        # キネマティックに自前で座標更新
        self.entity.transform.x += self.speed * dt
        
        # パーティクルトレイル
        if int(self.time * 60) % 2 == 0:
            EffectWrapper.spark(self.entity.transform.x + 8, self.entity.transform.y + 8)

        # 敵との衝突判定
        bx, by = self.entity.transform.x, self.entity.transform.y
        for enemy in self.world.find_with_tag("enemy"):
            if kagra.collide_rect(bx, by, 16, 16, enemy.transform.x, enemy.transform.y, 36, 36):
                kagra.emit(EV_ENEMY_DMG, {
                    "target_id": id(enemy),
                    "damage": self.damage,
                    "from_x": bx,
                    "knockback": 200
                })
                EffectWrapper.fx.spark(bx, by, count=5)
                self.entity.destroy()
                break


class CrystalScript(kagra.Script):
    """防衛対象のクリスタル"""
    def start(self):
        self.flash = 0.0
        self.time = 0.0
        kagra.on(EV_CRYSTAL_DMG, self._on_damaged)

    def update(self, dt: float):
        self.time += dt
        self.flash = max(0.0, self.flash - dt * 3.0)
        
        # フワフワ浮かせる演出
        self.entity.transform.y = 440 + math.sin(self.time * 2.0) * 10.0

    def _on_damaged(self, data):
        if g_state.game_over: return
        
        dmg = data.get("damage", 10)
        g_state.crystal_hp -= dmg
        self.flash = 1.0
        
        EffectWrapper.fx.damage(self.entity.transform.x + 24, self.entity.transform.y - 20, dmg, critical=True)
        EffectWrapper.fx.spark(self.entity.transform.x + 24, self.entity.transform.y + 24, count=8)
        kagra.emit(EV_CAM_SHAKE, {"amount": 15.0, "decay": 15.0})
        
        if g_state.crystal_hp <= 0:
            g_state.game_over = True
            kagra.emit(EV_GAME_OVER)


class EnemyBaseScript(kagra.Script):
    """敵の共通基底スクリプト"""
    def start(self):
        self.world = self.entity.world
        self.rb = self.entity.get(Rigidbody)
        self.hp = getattr(self.entity, "_hp", 30)
        self.speed = getattr(self.entity, "_speed", 50)
        self.score = getattr(self.entity, "_score", 100)
        
        self.time = 0.0
        self.flash = 0.0
        self.dir = 1
        
        self.crystal = self.world.find_with_name("Crystal")
        self._eid = id(self.entity)
        kagra.on(EV_ENEMY_DMG, self._on_damaged)

    def _on_damaged(self, data):
        # target_id を比較して自分宛てか判定（リファレンス準拠）
        if data.get("target_id") != self._eid:
            return
            
        self.hp -= data.get("damage", 10)
        self.flash = 0.3
        
        if self.rb:
            kd = 1 if data.get("from_x", 0) < self.entity.transform.x else -1
            self.rb.vx = kd * data.get("knockback", 200)
            self.rb.vy = -150
            
        EffectWrapper.fx.damage(self.entity.transform.x, self.entity.transform.y, data.get("damage", 10))

        if self.hp <= 0:
            kagra.emit(EV_SCORE_UP, {"amount": self.score})
            kagra.emit(EV_COIN_GET, {"amount": random.randint(1, 3)})
            EffectWrapper.fx.spark(self.entity.transform.x + 16, self.entity.transform.y + 16, count=10)
            self.entity.destroy()

    def check_attack(self):
        """プレイヤーおよびクリスタルへのダメージ判定を各updateで呼ぶ"""
        if g_state.game_over: return
        ex, ey = self.entity.transform.x, self.entity.transform.y
        ew, eh = 36, 36
        
        # プレイヤーへの攻撃
        player = self.world.find_with_name("Player")
        if player:
            px, py = player.transform.x, player.transform.y
            if kagra.collide_rect(ex, ey, ew, eh, px, py, 32, 48):
                kagra.emit(EV_PLAYER_DMG, {"damage": 10, "from_x": ex})
                
        # クリスタルへの攻撃
        if self.crystal:
            cx, cy = self.crystal.transform.x, self.crystal.transform.y
            if kagra.collide_rect(ex, ey, ew, eh, cx, cy, 48, 48):
                kagra.emit(EV_CRYSTAL_DMG, {"damage": 5})
                # 自爆ダメージ
                kagra.emit(EV_ENEMY_DMG, {"target_id": self._eid, "damage": 999, "from_x": cx})


class SlimeScript(EnemyBaseScript):
    """地上を歩きクリスタルを目指す敵"""
    def update(self, dt):
        self.time += dt
        self.flash = max(0.0, self.flash - dt * 4.0)
        
        # クリスタルの方向へ進む
        if self.crystal:
            self.dir = 1 if self.crystal.transform.x > self.entity.transform.x else -1
            
        self.rb.vx = self.dir * self.speed
        self.check_attack()


class BatScript(EnemyBaseScript):
    """空中をサイン波で飛びながらクリスタルを目指す敵"""
    def start(self):
        super().start()
        self.base_y = self.entity.transform.y
        if self.rb:
            self.rb.kinematic = True # 物理影響を無視
            
    def update(self, dt):
        self.time += dt
        self.flash = max(0.0, self.flash - dt * 4.0)
        
        if self.crystal:
            self.dir = 1 if self.crystal.transform.x > self.entity.transform.x else -1
            
        # キネマティックに移動
        self.entity.transform.x += self.dir * self.speed * dt
        self.entity.transform.y = self.base_y + math.sin(self.time * 3.0) * 40.0
        
        self.check_attack()


class SpawnerScript(kagra.Script):
    """ウェーブ管理と敵のスポーン"""
    def start(self):
        self.world = self.entity.world
        self.time = 0.0
        self.spawn_timer = 2.0
        self.wave_timer = 0.0

    def update(self, dt: float):
        if g_state.game_over: return
        
        self.time += dt
        self.spawn_timer -= dt
        self.wave_timer += dt
        
        # 30秒ごとにウェーブ進行（難易度上昇）
        if self.wave_timer >= 30.0:
            self.wave_timer = 0.0
            g_state.wave += 1
            EffectWrapper.fx.heal(SW//2, SH//2 - 100, f"WAVE {g_state.wave} START!")
            
        if self.spawn_timer <= 0:
            # ウェーブが進むほどスポーン間隔が短くなる
            self.spawn_timer = max(0.5, 3.0 - (g_state.wave * 0.2))
            self._spawn_enemy()

    def _spawn_enemy(self):
        typ = random.choice(["slime", "bat"])
        # 左右の画面外からスポーン
        x = random.choice([-50, MAP_W + 50])
        y = 300 if typ == "bat" else 100
        
        enemy = self.world.create(f"Enemy_{self.time}", tag="enemy")
        enemy.transform.set_pos(x, y)
        
        if typ == "slime":
            enemy.add(Rigidbody(gravity=GRAVITY))
            col = enemy.add(BoxCollider(w=36, h=36))
            col.layer = "enemy"
            enemy._hp = 30 + (g_state.wave * 10)
            enemy._speed = 50 + (g_state.wave * 5)
            enemy._score = 100
            enemy.add(SlimeScript())
        else:
            enemy.add(Rigidbody(gravity=GRAVITY, kinematic=True))
            col = enemy.add(BoxCollider(w=36, h=36))
            col.layer = "enemy"
            enemy._hp = 20 + (g_state.wave * 5)
            enemy._speed = 80 + (g_state.wave * 10)
            enemy._score = 150
            enemy.add(BatScript())


# =============================================================================
# 6. エフェクトマネージャのラッパー
# =============================================================================
class EffectWrapper:
    """kagra.EffectManager をグローバルに扱いやすくする"""
    fx = None

    @classmethod
    def init(cls, font):
        cls.fx = EffectManager()
        cls.fx.set_font(font)

    @classmethod
    def update(cls, dt):
        if cls.fx: cls.fx.update(dt)

    @classmethod
    def draw(cls):
        if cls.fx: cls.fx.draw()

    @classmethod
    def spark(cls, x, y):
        if cls.fx: cls.fx.spark(x, y, count=6)


# =============================================================================
# 7. Scene 実装
# =============================================================================

class TitleScene(kagra.Scene):
    """タイトル画面"""
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        self.time = 0.0
        
        # 背景の雲アニメーション用 Timeline
        self.bg_timeline = Timeline("TitleBG", loop=True)
        self.bg_scroll = 0.0
        track = Track(target=self, prop="bg_scroll")
        track.add_key(0.0, 0.0)
        track.add_key(10.0, -SW)
        self.bg_timeline.add(track)
        self.bg_timeline.play()

    def update(self, dt: float):
        self.time += dt
        self.bg_timeline.update(dt)
        
        if kagra.key_pressed(kagra.KEY_Z):
            g_state.reset()
            kagra.scene.change(GameScene())
        elif kagra.key_pressed(kagra.KEY_ESCAPE):
            raise SystemExit

    def draw(self):
        kagra.cls(20, 30, 50)
        
        # 背景描画
        for i in range(8):
            cx = (i * 200 + self.bg_scroll) % (SW + 200) - 100
            cy = 100 + (i % 3) * 50
            kagra.rect(cx, cy, 150, 40, 60, 70, 90, 150)

        tw, _ = kagra.measure_text(self.font, "DEFEND THE CRYSTAL", 72)
        kagra.draw_text(self.font, "DEFEND THE CRYSTAL", (SW - tw)/2, 200, 72, color=(100, 200, 255))
        
        if int(self.time * 2) % 2 == 0:
            pw, _ = kagra.measure_text(self.font, "Press Z to Start", 32)
            kagra.draw_text(self.font, "Press Z to Start", (SW - pw)/2, 450, 32, color=(255, 255, 255))
            
        kagra.draw_text(self.font, "Controls: Arrows(Move/Jump), Z, X, ESC", 20, SH - 40, 20, color=(150, 150, 150))


class GameScene(kagra.Scene):
    """ゲーム本編"""
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        EffectWrapper.init(self.font)
        self.time = 0.0
        
        # --- システム初期化 ---
        self.world = kagra.World()
        self.physics = PhysicsSystem(gravity=GRAVITY)
        
        self.cam = Camera(screen_w=SW, screen_h=SH, world_w=MAP_W, world_h=MAP_H, zoom=1.0)
        kagra.set_camera(self.cam)

        # --- タイルマップ構築 ---
        tile_colors = [
            (0, 0, 0, 0),        # 0:空
            (80, 200, 80, 255),  # 1:草
            (120, 80, 40, 255),  # 2:土
        ]
        self.tileset_tex = make_tileset_png(TILE_SIZE, tile_colors)
        ts = TileSet(self.tileset_tex, TILE_SIZE, TILE_SIZE)
        self.tilemap = TileMap(ts, LEVEL_MAP, TILE_ATTRS, tile_w=TILE_SIZE, tile_h=TILE_SIZE)
        self.physics.set_tilemap(self.tilemap)

        # --- エンティティ生成 ---
        # プレイヤー
        self.player = self.world.create("Player", tag="player")
        self.player.transform.set_pos(MAP_W/2 - 100, 300)
        self.player.add(Rigidbody(gravity=GRAVITY, drag=0.05))
        p_col = self.player.add(BoxCollider(w=32, h=48))
        p_col.layer = "player"
        self.ps = self.player.add(PlayerScript())

        # クリスタル（拠点）
        self.crystal = self.world.create("Crystal", tag="crystal")
        self.crystal.transform.set_pos(MAP_W/2 - 24, 440)
        # クリスタルは物理無効（キネマティック）
        self.crystal.add(Rigidbody(gravity=0, kinematic=True))
        c_col = self.crystal.add(BoxCollider(w=48, h=48))
        c_col.layer = "crystal"
        self.cs = self.crystal.add(CrystalScript())

        # スポナー
        spawner = self.world.create("Spawner")
        spawner.add(SpawnerScript())

        # --- UI 初期化 ---
        self.hp_bar = ProgressBar(x=20, y=20, w=250, h=24, max_val=g_state.max_hp, value=g_state.hp,
                                  fg_color=(80, 220, 80), label_font=self.font, label_fmt="HP {value}/{max}", smooth=True)
        self.mp_bar = ProgressBar(x=20, y=50, w=200, h=16, max_val=g_state.max_mp, value=g_state.mp,
                                  fg_color=(80, 150, 255), label_font=self.font, label_fmt="MP", label_size=12, smooth=True)
        self.crystal_bar = ProgressBar(x=SW/2 - 150, y=20, w=300, h=24, max_val=g_state.crystal_max_hp, value=g_state.crystal_hp,
                                       fg_color=(100, 255, 200), label_font=self.font, label_fmt="CRYSTAL HP {value}/{max}", smooth=True)

        # --- Event Bus 登録 ---
        kagra.on(EV_SCORE_UP,  lambda d: setattr(g_state, "score", g_state.score + d.get("amount", 0)))
        kagra.on(EV_COIN_GET,  lambda d: setattr(g_state, "coins", g_state.coins + d.get("amount", 0)))
        kagra.on(EV_CAM_SHAKE, lambda d: self.cam.shake(amount=d.get("amount", 5.0), decay=d.get("decay", 10.0)))
        kagra.on(EV_GAME_OVER, lambda _: kagra.scene.change(GameOverScene()))

    def on_exit(self):
        """リファレンス準拠：シーン遷移時にイベントを全解除"""
        kagra.off_all(EV_SCORE_UP)
        kagra.off_all(EV_COIN_GET)
        kagra.off_all(EV_CAM_SHAKE)
        kagra.off_all(EV_GAME_OVER)
        # Script側で登録したものも安全のため解除
        kagra.off_all(EV_PLAYER_DMG)
        kagra.off_all(EV_ENEMY_DMG)
        kagra.off_all(EV_CRYSTAL_DMG)

    def update(self, dt: float):
        self.time += dt
        
        # ポーズ（ショップ）画面へ
        if kagra.key_pressed(kagra.KEY_ESCAPE) and not g_state.game_over:
            kagra.scene.push(ShopScene())
            return

        # ECS & Physics 更新
        self.world.update(dt)
        self.physics.update(dt, self.world)
        kagra.flush_events()

        # カメラ追従と更新
        if self.player:
            px, py = self.player.transform.x, self.player.transform.y
            self.cam.follow(px, py, obj_w=32, obj_h=48, lerp=0.1)
        self.cam.update(dt)
        
        # エフェクト更新
        EffectWrapper.update(dt)

        # UI更新
        self.hp_bar.max_val = g_state.max_hp
        self.hp_bar.value = g_state.hp
        ratio = g_state.hp / g_state.max_hp
        if ratio > 0.5: self.hp_bar.fg_color = (80, 220, 80)
        elif ratio > 0.2: self.hp_bar.fg_color = (255, 200, 50)
        else: self.hp_bar.fg_color = (255, 60, 60)
        self.hp_bar.update(dt)
        
        self.mp_bar.max_val = g_state.max_mp
        self.mp_bar.value = g_state.mp
        self.mp_bar.update(dt)
        
        self.crystal_bar.max_val = g_state.crystal_max_hp
        self.crystal_bar.value = g_state.crystal_hp
        self.crystal_bar.update(dt)

    def draw(self):
        kagra.cls(30, 40, 60)
        
        # タイルマップ
        self.tilemap.draw(self.cam)

        # ── エンティティの描画（アセットなし環境向けに矩形で表現） ──
        # クリスタル
        if self.crystal:
            cx, cy = self.cam.to_screen(self.crystal.transform.x, self.crystal.transform.y)
            cw = self.cam.scale_to_screen(48)
            r, g, b = (255, 255, 255) if self.cs.flash > 0 else (100, 255, 200)
            kagra.rect(cx + cw*0.2, cy, cw*0.6, cw, r, g, b)
            # コア
            kagra.rect(cx + cw*0.35, cy + cw*0.3, cw*0.3, cw*0.4, 255, 255, 255)

        # 敵
        for enemy in self.world.find_with_tag("enemy"):
            ex, ey = self.cam.to_screen(enemy.transform.x, enemy.transform.y)
            ew = self.cam.scale_to_screen(36)
            script = enemy.get(SlimeScript) or enemy.get(BatScript)
            r, g, b = (255, 255, 255) if script and script.flash > 0 else (150, 50, 150)
            if enemy.get(SlimeScript):
                kagra.rect(ex, ey + ew*0.2, ew, ew*0.8, 100, 200, 80) # スライムは緑
            else:
                kagra.rect(ex, ey, ew, ew*0.6, r, g, b) # コウモリ

        # 魔法弾
        for bullet in self.world.find_with_tag("bullet"):
            bx, by = self.cam.to_screen(bullet.transform.x, bullet.transform.y)
            bw = self.cam.scale_to_screen(16)
            kagra.rect(bx, by, bw, bw, 100, 200, 255)

        # プレイヤー
        if self.player and not g_state.game_over:
            px, py = self.cam.to_screen(self.player.transform.x, self.player.transform.y)
            pw, ph = self.cam.scale_to_screen(32), self.cam.scale_to_screen(48)
            
            if self.ps.invincible <= 0 or int(self.ps.invincible * 15) % 2 == 0:
                r, g, b = (255, 150, 150) if self.ps.flash > 0 else (80, 160, 255)
                kagra.rect(px, py, pw, ph, r, g, b)
                # 剣の軌跡
                if self.ps.attack_rect:
                    ax, ay, aw, ah = self.ps.attack_rect
                    asx, asy = self.cam.to_screen(ax, ay)
                    kagra.rect(asx, asy, self.cam.scale_to_screen(aw), self.cam.scale_to_screen(ah), 255, 255, 255, 180)

        # ── エフェクト・UI描画 ──
        EffectWrapper.draw()
        
        self.hp_bar.draw()
        self.mp_bar.draw()
        self.crystal_bar.draw()
        
        hud_txt = f"WAVE: {g_state.wave}   SCORE: {g_state.score:06d}   COIN: {g_state.coins:03d}"
        tw, _ = kagra.measure_text(self.font, hud_txt, 24)
        kagra.draw_text(self.font, hud_txt, SW - tw - 20, 20, 24, color=(255, 220, 80))


class ShopScene(kagra.Scene):
    """ポーズ兼ショップ画面 (push で重ねる)"""
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        
        # UI構築
        self.vbox = VBox(x=SW/2 - 200, y=150, w=400, gap=16, padding=20)
        self.vbox.add(Label(self.font, "SHOP & PAUSE", size=48, color=(255, 220, 80), align="center"))
        
        # アップグレードボタンの定義
        def buy_heal():
            if g_state.coins >= 10 and g_state.hp < g_state.max_hp:
                g_state.coins -= 10
                g_state.hp = g_state.max_hp
        
        def buy_atk():
            if g_state.coins >= 20:
                g_state.coins -= 20
                g_state.attack_power += 5
                
        def buy_crystal_heal():
            if g_state.coins >= 15 and g_state.crystal_hp < g_state.crystal_max_hp:
                g_state.coins -= 15
                g_state.crystal_hp = min(g_state.crystal_max_hp, g_state.crystal_hp + 50)

        self.btn_heal = Button(self.font, "Player Full Heal (10 Coins)", 0, 0, 400, 50, on_confirm=buy_heal)
        self.btn_atk  = Button(self.font, "Attack Power UP (20 Coins)", 0, 0, 400, 50, on_confirm=buy_atk)
        self.btn_crys = Button(self.font, "Crystal Heal 50 (15 Coins)", 0, 0, 400, 50, on_confirm=buy_crystal_heal)
        
        self.vbox.add(self.btn_heal)
        self.vbox.add(self.btn_atk)
        self.vbox.add(self.btn_crys)
        
        self.vbox.add(Label(self.font, "", size=20)) # Spacer
        self.vbox.add(Button(self.font, "Resume Game", 0, 0, 400, 50, on_confirm=lambda: kagra.scene.pop()))
        self.vbox.add(Button(self.font, "Quit to Title", 0, 0, 400, 50, on_confirm=lambda: kagra.scene.change(TitleScene())))
        self.vbox.layout()

    def update(self, dt: float):
        if kagra.key_pressed(kagra.KEY_ESCAPE):
            kagra.scene.pop()
            
        # ボタンの状態更新 (コイン不足なら押せないように見せるなど)
        self.vbox.update(dt)

    def draw(self):
        # 半透明の黒でゲーム画面を暗くする
        kagra.rect(0, 0, SW, SH, 0, 0, 0, 180)
        
        self.vbox.draw()
        
        coin_txt = f"Your Coins: {g_state.coins}"
        cw, _ = kagra.measure_text(self.font, coin_txt, 32)
        kagra.draw_text(self.font, coin_txt, (SW - cw)/2, 80, 32, color=(255, 255, 255))
        
        stat_txt = f"ATK: {g_state.attack_power}  /  HP: {int(g_state.hp)}  /  CRYSTAL: {int(g_state.crystal_hp)}"
        sw, _ = kagra.measure_text(self.font, stat_txt, 24)
        kagra.draw_text(self.font, stat_txt, (SW - sw)/2, SH - 100, 24, color=(150, 200, 255))


class GameOverScene(kagra.Scene):
    """ゲームオーバー画面"""
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        self.alpha = 0.0
        self.time = 0.0
        # Tween を使ったフェードイン
        self.tween = Tween(start=0.0, end=1.0, duration=2.0, easing=Easing.out_quad,
                           on_update=lambda v: setattr(self, "alpha", v))
                           
    def update(self, dt: float):
        self.time += dt
        self.tween.update(dt)
        # Tween.done を使用
        if self.tween.done and kagra.key_pressed(kagra.KEY_Z):
            kagra.scene.change(TitleScene())

    def draw(self):
        kagra.cls(20, 0, 0)
        
        tw, _ = kagra.measure_text(self.font, "GAME OVER", 80)
        kagra.draw_text(self.font, "GAME OVER", (SW - tw)/2, 200, 80, color=(255, 60, 60))
        
        res_txt = f"WAVE REACHED: {g_state.wave}    FINAL SCORE: {g_state.score}"
        rw, _ = kagra.measure_text(self.font, res_txt, 36)
        kagra.draw_text(self.font, res_txt, (SW - rw)/2, 350, 36, color=(255, 255, 255))
        
        if self.tween.done and int(self.time * 2) % 2 == 0:
            pw, _ = kagra.measure_text(self.font, "Press Z to Return Title", 32)
            kagra.draw_text(self.font, "Press Z to Return Title", (SW - pw)/2, 500, 32, color=(150, 150, 150))
            
        # 画面全体フェード
        a = int(255 * (1.0 - self.alpha))
        if a > 0:
            kagra.rect(0, 0, SW, SH, 0, 0, 0, a)


# =============================================================================
# 8. エントリポイント
# =============================================================================
if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="Defend the Crystal - KAGRA v3", fps=FPS)
    kagra.run(start_scene=TitleScene())