"""
space_shooter.py - KAGRA ECS デモ：スペースシューター（難易度強化版）
================================================================
■ 変更点
  - 敵が画面下に逃げると残機-1（イベント enemy_escaped）
  - 敵の出現数増加（最大14体）
  - 敵弾の速度アップ、発射間隔短縮
  - アイテムは取れなくても落下消失（仕様通り）

■ 操作
  ← →     : 移動
  Z / SPACE: ショット
  X        : ボム
  ESC      : 終了
"""
import kagra
import math
import random

SW, SH = 800, 600

# ── 色 ───────────────────────────────────────────────────────
C_BG     = (5,  5,  20)
C_PLAYER = (80, 200, 255)
C_ENEMY  = (255, 80, 80)
C_SNIPER = (255, 160, 80)
C_SHOT_P = (200, 255, 100)
C_SHOT_E = (255, 180, 50)
C_UI     = (255, 220, 80)

# ════════════════════════════════════════════════════════════
#  コンポーネント（データ）
# ════════════════════════════════════════════════════════════

class Velocity(kagra.Component):
    def __init__(self, vx=0.0, vy=0.0):
        super().__init__()
        self.vx, self.vy = vx, vy

class Health(kagra.Component):
    def __init__(self, hp=1):
        super().__init__()
        self.hp = self.max_hp = hp

    def damage(self, d=1):
        self.hp -= d
        if self.hp <= 0:
            self.entity.destroy()
        return self.hp <= 0

class Tag(kagra.Component):
    """汎用タグ付きデータ。"""
    def __init__(self, **kwargs):
        super().__init__()
        for k, v in kwargs.items():
            setattr(self, k, v)

class WeaponLevel(kagra.Component):
    """武器レベル（1～3）"""
    def __init__(self, level=1):
        super().__init__()
        self.level = level
        self.shoot_cooldown = 0.12
        self._timer = 0.0

# ════════════════════════════════════════════════════════════
#  スクリプト（振る舞い）
# ════════════════════════════════════════════════════════════

class PlayerCtrl(kagra.Script):
    SPEED = 320.0

    def start(self):
        self._weapon = self.entity.get(WeaponLevel)
        if not self._weapon:
            self._weapon = WeaponLevel(1)
            self.entity.add(self._weapon)

    def update(self, dt):
        t, v = self.entity.transform, self.entity.get(Velocity)
        v.vx = (-self.SPEED if kagra.down("LEFT") else
                 self.SPEED if kagra.down("RIGHT") else 0.0)
        t.x  = max(20, min(SW - 20, t.x + v.vx * dt))
        t.y  = SH - 60

        w = self._weapon
        w._timer -= dt
        if (kagra.down("Z") or kagra.down("SPACE")) and w._timer <= 0:
            w._timer = w.shoot_cooldown
            if w.level == 1:
                kagra.emit("spawn_bullet",
                           {"x": t.x, "y": t.y-20, "vy": -620, "owner": "player"})
            elif w.level == 2:
                kagra.emit("spawn_bullet",
                           {"x": t.x-8, "y": t.y-20, "vy": -620, "owner": "player"})
                kagra.emit("spawn_bullet",
                           {"x": t.x+8, "y": t.y-20, "vy": -620, "owner": "player"})
            else:  # level >= 3
                kagra.emit("spawn_bullet",
                           {"x": t.x,    "y": t.y-20, "vy": -620, "owner": "player"})
                kagra.emit("spawn_bullet",
                           {"x": t.x-12, "y": t.y-20, "vy": -580, "vx": -40, "owner": "player"})
                kagra.emit("spawn_bullet",
                           {"x": t.x+12, "y": t.y-20, "vy": -580, "vx":  40, "owner": "player"})

class ShotMove(kagra.Script):
    def update(self, dt):
        t, v = self.entity.transform, self.entity.get(Velocity)
        t.x += v.vx * dt
        t.y += v.vy * dt
        if not (0 <= t.x <= SW and -30 <= t.y <= SH + 30):
            self.entity.destroy()

class EnemyAI(kagra.Script):
    """通常の敵：左右に揺れながら降下、下方向に弾を撃つ"""
    def __init__(self):
        super().__init__()
        self._t      = random.uniform(0, math.pi * 2)
        self._base_x = 0.0
        self._shoot_cd = random.uniform(1.5, 2.5)   # 発射間隔短縮

    def start(self):
        self._base_x = self.entity.transform.x

    def update(self, dt):
        t = self.entity.transform
        v = self.entity.get(Velocity)
        self._t += dt * 1.5
        t.x = self._base_x + math.sin(self._t) * 40
        t.y += v.vy * dt

        if t.y > SH + 40:
            # 画面外に逃げられた → 残機減少イベント
            kagra.emit("enemy_escaped", {})
            self.entity.destroy()
            return

        self._shoot_cd -= dt
        if self._shoot_cd <= 0:
            self._shoot_cd = random.uniform(1.5, 2.5)
            kagra.emit("spawn_bullet",
                       {"x": t.x, "y": t.y + 18, "vy": 280, "owner": "enemy"})

class SniperEnemy(kagra.Script):
    """スナイパー敵：自機狙い弾を撃つ"""
    def __init__(self, scene_ref):
        super().__init__()
        self._scene = scene_ref
        self._shoot_cd = random.uniform(1.8, 2.8)

    def update(self, dt):
        t = self.entity.transform
        v = self.entity.get(Velocity)
        t.y += v.vy * dt
        if t.y > SH + 40:
            kagra.emit("enemy_escaped", {})
            self.entity.destroy()
            return

        self._shoot_cd -= dt
        if self._shoot_cd <= 0:
            self._shoot_cd = random.uniform(1.8, 2.8)
            if self._scene._player and not self._scene._player.is_destroyed:
                px = self._scene._player.transform.x
                py = self._scene._player.transform.y
                dx = px - t.x
                dy = py - t.y
                length = math.hypot(dx, dy)
                if length > 0:
                    vx = (dx / length) * 260
                    vy = (dy / length) * 260
                    kagra.emit("spawn_bullet",
                               {"x": t.x, "y": t.y+15, "vx": vx, "vy": vy, "owner": "enemy"})

class PowerupItem(kagra.Script):
    """パワーアップアイテム（ゆっくり降下）"""
    def update(self, dt):
        t = self.entity.transform
        t.y += 60 * dt
        if t.y > SH + 20:
            self.entity.destroy()

# ════════════════════════════════════════════════════════════
#  補助クラス（ECS 外）
# ════════════════════════════════════════════════════════════

class ParticleSystem:
    MAX = 2000

    def __init__(self):
        self.batch = kagra.InstanceBatch(capacity=self.MAX, sprite_w=4, sprite_h=4)
        self._pool: list = []

    def burst(self, x, y, n=14):
        for _ in range(n):
            a = random.uniform(0, math.pi * 2)
            s = random.uniform(60, 220)
            l = random.uniform(0.3, 0.7)
            self._pool.append([x, y, math.cos(a)*s, math.sin(a)*s, l, l])

    def update(self, dt):
        alive = []
        for p in self._pool:
            p[0] += p[2] * dt
            p[1] += p[3] * dt
            p[4] -= dt
            if p[4] > 0:
                alive.append(p)
        self._pool = alive[:self.MAX]

    def draw(self):
        if not self._pool: return
        data = [[p[0], p[1], 4.0, 4.0, 0.0, p[4]/p[5]] for p in self._pool]
        self.batch.update(data)
        self.batch.draw()


class StarField:
    def __init__(self, n=150):
        self._stars = [
            [random.uniform(0, SW), random.uniform(0, SH),
             random.uniform(0.3, 1.5), random.uniform(1, 3)]
            for _ in range(n)
        ]
        self.batch = kagra.InstanceBatch(capacity=n, sprite_w=2, sprite_h=2)

    def update(self, dt):
        for s in self._stars:
            s[1] += s[2] * 30 * dt
            if s[1] > SH:
                s[1] = 0
                s[0] = random.uniform(0, SW)

    def draw(self):
        data = [[s[0], s[1], s[3], s[3], 0.0, s[2]/1.5] for s in self._stars]
        self.batch.update(data)
        self.batch.draw()

# ════════════════════════════════════════════════════════════
#  メインシーン
# ════════════════════════════════════════════════════════════

class ShooterScene(kagra.EntityScene):
    def on_enter(self):
        self.font    = kagra.assets.font("meiryo")
        self.score   = 0
        self.lives   = 3
        self.wave    = 1
        self._game_over = False
        self._flash     = 0.0

        self.bomb_stock = 1
        self.bomb_active = False
        self.bomb_timer = 0.0
        self.invincible = False
        self.invincible_timer = 0.0

        self._wave_cleared = False
        self._wave_cooldown = 1.5

        self.particles = ParticleSystem()
        self.stars     = StarField()

        kagra.on("spawn_bullet",   self._on_spawn_bullet)
        kagra.on("player_hit",     self._on_player_hit)
        kagra.on("enemy_killed",   self._on_enemy_killed)
        kagra.on("enemy_escaped",  self._on_enemy_escaped)   # 新規

        p = self.world.create("player", tag="player")
        p.transform.x = SW // 2
        p.transform.y = SH - 60
        p.add(Velocity())
        p.add(kagra.Collider(28, 28, -14, -14))
        p.add(PlayerCtrl())
        p.add(WeaponLevel(1))
        self._player = p

        self._spawn_wave()

    def on_exit(self):
        kagra.off_all("spawn_bullet")
        kagra.off_all("player_hit")
        kagra.off_all("enemy_killed")
        kagra.off_all("enemy_escaped")

    def _on_spawn_bullet(self, data):
        b = self.world.create("bullet", tag=data["owner"] + "_bullet")
        b.transform.x = data["x"]
        b.transform.y = data["y"]
        b.add(Velocity(vx=data.get("vx", 0), vy=data["vy"]))
        b.add(kagra.Collider(6, 14, -3, -7))
        b.add(ShotMove())

    def _on_player_hit(self, data):
        if self._game_over or self.invincible:
            return
        self.lives -= 1
        self._flash = 0.4
        self.particles.burst(data.get("x", SW//2), SH - 60, n=20)
        if self.lives <= 0:
            self._game_over = True

    def _on_enemy_killed(self, data):
        self.score += data.get("score", 10)
        self.particles.burst(data["x"], data["y"], n=16)
        if random.random() < 0.2:
            item = self.world.create("powerup", tag="powerup")
            item.transform.x = data["x"]
            item.transform.y = data["y"]
            item.add(Velocity(vy=40))
            item.add(kagra.Collider(16, 16, -8, -8))
            item.add(PowerupItem())

    def _on_enemy_escaped(self, data):
        """敵が画面下に逃げたときの処理"""
        if self._game_over or self.invincible:
            return
        self.lives -= 1
        self._flash = 0.4
        if self.lives <= 0:
            self._game_over = True

    def _spawn_wave(self):
        # 敵の数を増やす (基本 5 + wave, 最大14体)
        base_count = 5 + self.wave
        n = min(base_count, 14)

        spacing = SW / (n + 1)
        for i in range(n):
            e = self.world.create("enemy", tag="enemy")
            e.transform.x = spacing * (i + 1)
            e.transform.y = -30 - random.uniform(0, 60)
            hp = 1 + self.wave // 4
            e.add(Velocity(vy=45 + self.wave * 5))
            e.add(Health(hp))
            e.add(Tag(score=10 * self.wave, enemy_type="normal"))
            e.add(kagra.Collider(26, 20, -13, -10))

            if random.random() < 0.35:  # スナイパー率を微増
                e.add(SniperEnemy(self))
                e.get(Tag).enemy_type = "sniper"
            else:
                e.add(EnemyAI())

    def _count_enemies(self):
        enemies = self.world.find_with_tag("enemy")
        return len([e for e in enemies if not e.is_destroyed])

    def _activate_bomb(self):
        self.bomb_stock -= 1
        self.bomb_active = True
        self.bomb_timer = 0.8
        self.invincible = True
        self.invincible_timer = 2.0

        for eb in self.world.find_with_tag("enemy_bullet"):
            eb.destroy()

        for en in self.world.find_with_tag("enemy"):
            if en.is_destroyed:
                continue
            if en.get(Health).damage(2):
                tag = en.get(Tag)
                kagra.emit("enemy_killed", {
                    "x": en.transform.x,
                    "y": en.transform.y,
                    "score": tag.score if tag else 10
                })

        self.particles.burst(SW//2, SH//2, n=60)

    def update(self, dt):
        if kagra.pressed("ESCAPE"):
            raise SystemExit

        if self._game_over:
            if kagra.pressed("Z") or kagra.pressed("SPACE"):
                kagra.scene.change(ShooterScene())
            return

        if kagra.pressed("X") and self.bomb_stock > 0 and not self.bomb_active and not self._wave_cleared:
            self._activate_bomb()

        if self.bomb_active:
            self.bomb_timer -= dt
            if self.bomb_timer <= 0:
                self.bomb_active = False
        if self.invincible:
            self.invincible_timer -= dt
            if self.invincible_timer <= 0:
                self.invincible = False

        super().update(dt)

        self.stars.update(dt)
        self.particles.update(dt)
        kagra.flush_events()

        if self._flash > 0:
            self._flash -= dt

        self._check_collisions()

        if not self._wave_cleared:
            if self._count_enemies() == 0:
                self._wave_cleared = True
                self._wave_cooldown = 1.2
                if self.bomb_stock < 2:
                    self.bomb_stock += 1
        else:
            self._wave_cooldown -= dt
            if self._wave_cooldown <= 0:
                self.wave += 1
                self._spawn_wave()
                self._wave_cleared = False

    def _check_collisions(self):
        p_shots = self.world.find_with_tag("player_bullet")
        enemies = self.world.find_with_tag("enemy")
        e_shots = self.world.find_with_tag("enemy_bullet")
        items   = self.world.find_with_tag("powerup")

        player_c = (self._player.get(kagra.Collider)
                    if self._player and not self._player.is_destroyed else None)

        for b in p_shots:
            if b.is_destroyed: continue
            bc = b.get(kagra.Collider)
            for en in enemies:
                if en.is_destroyed: continue
                ec = en.get(kagra.Collider)
                if bc and ec and bc.is_colliding(ec):
                    b.destroy()
                    if en.get(Health).damage():
                        tag = en.get(Tag)
                        kagra.emit("enemy_killed", {
                            "x": en.transform.x,
                            "y": en.transform.y,
                            "score": tag.score if tag else 10,
                        })
                    break

        if player_c:
            for item in items:
                if item.is_destroyed: continue
                ic = item.get(kagra.Collider)
                if ic and ic.is_colliding(player_c):
                    item.destroy()
                    w = self._player.get(WeaponLevel)
                    if w and w.level < 3:
                        w.level += 1
                        w.shoot_cooldown = max(0.07, 0.12 - (w.level-1)*0.02)

        if not player_c or self.invincible:
            return
        for b in e_shots:
            if b.is_destroyed: continue
            bc = b.get(kagra.Collider)
            if bc and bc.is_colliding(player_c):
                b.destroy()
                kagra.emit("player_hit", {"x": self._player.transform.x})
                return
        for en in enemies:
            if en.is_destroyed: continue
            ec = en.get(kagra.Collider)
            if ec and ec.is_colliding(player_c):
                en.destroy()
                kagra.emit("player_hit", {"x": self._player.transform.x})
                return

    def draw(self):
        kagra.cls(*C_BG)
        kagra.rect(0, 0, SW, SH, *C_BG, 255)

        self.stars.draw()
        self._draw_entities()
        self.particles.draw()

        if self._flash > 0:
            kagra.rect(0, 0, SW, SH, 255, 50, 50, int(self._flash / 0.4 * 100))

        if self.bomb_active:
            alpha = int(80 * (self.bomb_timer / 0.8))
            kagra.rect(0, 0, SW, SH, 255, 255, 255, alpha)

        self._draw_ui()

    def _draw_entities(self):
        for e in self.world.entities:
            if e.is_destroyed or not e.active: continue
            t  = e.transform
            cx, cy = int(t.x), int(t.y)

            if e.tag == "player":
                kagra.rect(cx-2,  cy-22, 4,  18, *C_PLAYER, 255)
                kagra.rect(cx-10, cy-10, 20, 4,  *C_PLAYER, 255)
                kagra.rect(cx-14, cy-4,  28, 8,  *C_PLAYER, 200)
                kagra.circle(cx, cy-20, 5, *C_PLAYER, 255)

            elif e.tag == "enemy":
                tag = e.get(Tag)
                if tag and tag.enemy_type == "sniper":
                    kagra.circle(cx, cy, 14, *C_SNIPER, 220)
                    kagra.circle(cx, cy,  8, 255, 200, 150, 160)
                else:
                    kagra.circle(cx, cy, 14, *C_ENEMY, 220)
                    kagra.circle(cx, cy,  8, 255, 150, 150, 160)

                hp = e.get(Health)
                if hp and hp.max_hp > 1:
                    bw = 28
                    kagra.rect(cx-bw//2, cy-22, bw, 3, 40, 40, 40, 200)
                    fw = int(bw * hp.hp / hp.max_hp)
                    kagra.rect(cx-bw//2, cy-22, fw, 3, 80, 255, 80, 220)

            elif "bullet" in e.tag:
                if "player" in e.tag:
                    kagra.circle(cx, cy, 4, *C_SHOT_P, 255)
                    kagra.rect(cx-1, cy, 2, 10, *C_SHOT_P, 160)
                else:
                    kagra.circle(cx, cy, 5, *C_SHOT_E, 200)

            elif e.tag == "powerup":
                kagra.rect(cx-6, cy-6, 12, 12, 100, 255, 100, 220)
                kagra.draw_text(self.font, "P", cx-6, cy-8, 14, 255, 255, 255)

    def _draw_ui(self):
        kagra.rect(0, 0, SW, 48, 0, 0, 0, 180)
        kagra.draw_text(self.font, f"SCORE  {self.score:>7}", 20, 10, 22, *C_UI)
        kagra.draw_text(self.font, f"WAVE {self.wave}", SW//2 - 50, 10, 22, 160, 210, 255)

        for i in range(self.lives):
            kagra.circle(SW - 40 - i * 30, 24, 10, 255, 80, 100, 255)

        for i in range(self.bomb_stock):
            kagra.rect(SW - 28 - i*30, 40, 20, 6, 255, 220, 80, 255)

        w = self._player.get(WeaponLevel) if self._player else None
        level = w.level if w else 1
        level_text = ["Lv.1", "Lv.2", "Lv.3"][level-1]
        kagra.draw_text(self.font, level_text, 20, 40, 16, 180, 255, 180)

        if self._wave_cleared:
            kagra.draw_text(self.font, f"WAVE {self.wave+1} 準備中...",
                            SW//2 - 100, SH//2, 28, *C_UI)

        if self._game_over:
            kagra.rect(0, SH//2 - 70, SW, 140, 0, 0, 0, 210)
            kagra.draw_text(self.font, "GAME OVER",
                            SW//2 - 100, SH//2 - 50, 40, 255, 80, 80)
            kagra.draw_text(self.font, f"FINAL SCORE  {self.score}",
                            SW//2 - 110, SH//2, 26, *C_UI)
            kagra.draw_text(self.font, "Z / SPACE でリスタート",
                            SW//2 - 100, SH//2 + 42, 18, 160, 160, 220)

        n = len([e for e in self.world.entities if not e.is_destroyed])
        kagra.draw_text(self.font, f"entities: {n}",
                        SW - 130, SH - 20, 13, 60, 80, 100)


if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA - Space Shooter (Hard Edition)", 60)
    kagra.run(start_scene=ShooterScene())