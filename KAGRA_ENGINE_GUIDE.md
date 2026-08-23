# KAGRA ゲームエンジン 完全ガイド（履歴）

> **このファイルは現行仕様ではない。** Phase 6 時点のメモ。
> 今の入口は [README](README.md) / [docs/API_INDEX.md](docs/API_INDEX.md) /
> [docs/REVIEW.ja.md](docs/REVIEW.ja.md) / [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md)。
> 中身は増やさない。エージェントはここから API をコピーしないこと。
>
> 2D / タイルマップ系のサンプルは `examples/archive/`。推奨は `examples/` 直下の VRM ゲーム。

---

## 目次

1. [エンジン総評](#1-エンジン総評)
2. [アーキテクチャ概観](#2-アーキテクチャ概観)
3. [セットアップ / ビルド方法](#3-セットアップ--ビルド方法)
4. [基本的な使い方](#4-基本的な使い方)
5. [シーン管理](#5-シーン管理)
6. [描画 API](#6-描画-api)
7. [入力 API](#7-入力-api)
8. [ECS（Entity / Component / World）](#8-ecsentity--component--world)
9. [物理エンジン](#9-物理エンジン)
10. [タイルマップ](#10-タイルマップ)
11. [アニメーションシステム](#11-アニメーションシステム)
12. [VRM / 3D キャラクター](#12-vrm--3d-キャラクター)
13. [UI システム](#13-ui-システム)
14. [オーディオ](#14-オーディオ)
15. [エフェクト](#15-エフェクト)
16. [イベントバス](#16-イベントバス)
17. [Tween / イージング](#17-tween--イージング)
18. [タイムライン](#18-タイムライン)
19. [アセット管理](#19-アセット管理)
20. [手続き生成マップ](#20-手続き生成マップ)
21. [BGM 同期 / リズムゲーム](#21-bgm-同期--リズムゲーム)
22. [Boids シミュレーション](#22-boids-シミュレーション)
23. [Scriptable Object / DataObject](#23-scriptable-object--dataobject)
24. [カメラ（2D / 3D）](#24-カメラ2d--3d)
25. [エディタ](#25-エディタ)
26. [サンプルゲーム一覧](#26-サンプルゲーム一覧)
27. [他の人に使ってもらう方法](#27-他の人に使ってもらう方法)
28. [今後の改善点・ロードマップ](#28-今後の改善点ロードマップ)

---

## 1. エンジン総評

KAGRA は **Rust の高速レンダリングコア** と **Python の柔軟なゲームロジック** を組み合わせた、国産ゲームエンジンです。

### 強み

| カテゴリ | 詳細 |
|---------|------|
| **VRM 完全対応** | スプリングボーン・ブレンドシェイプ・BVH/FBX モーション再生を標準搭載。VRM キャラを 3 行で動かせる |
| **日本語 IME 対応** | 変換中テキストの取得・IME カーソル位置設定をネイティブサポート |
| **2層 API 設計** | `kagra.image()` などのシンプル API と低レベル API を併用可能 |
| **ECS 設計** | Unity に近い Component システムで学習コストが低い |
| **GPU Boids** | Compute Shader による数千体の群行動シミュレーション |
| **BGM 同期** | BPM ベースのリズムゲームフレームワーク標準搭載 |
| **豊富なサンプル** | 2D アクション・3D 迷路・VRM ロマンス・Boids など 6 種以上 |

### 向いているプロジェクト

- VRM キャラを使ったゲーム・デモ・インタラクティブコンテンツ
- 日本語対応の RPG・ノベルゲーム
- Python でプロトタイピングしたいゲーム
- 2D アクション・横スクロール・タイルベースゲーム
- リズムゲーム・音楽ゲーム

---

## 2. アーキテクチャ概観

```
┌─────────────────────────────────────────────────────────┐
│                    Python ゲームコード                    │
│         (your_game.py / scene / components)             │
└────────────────────────┬────────────────────────────────┘
                         │ import kagra
┌────────────────────────▼────────────────────────────────┐
│               kagra/__init__.py  (Python API 層)         │
│  シンプル API:  kagra.image() / kagra.text() / kagra.se() │
│  ECS:          Entity / Component / World               │
│  高レベル:      VrmAvatar / Physics / TileMap / UI ...   │
└────────────────────────┬────────────────────────────────┘
                         │ PyO3 (FFI)
┌────────────────────────▼────────────────────────────────┐
│              kagra_core  (Rust クレート)                  │
│  window.rs     : winit イベントループ                     │
│  renderer.rs   : wgpu 描画 (2D/3D/スキニング)            │
│  audio.rs      : kira / cpal オーディオエンジン           │
│  vrm.rs        : VRM モデル管理・ボーン操作               │
│  fbx_loader.rs : FBX パーサ（Mixamo 対応）               │
│  boids_gpu.rs  : Compute Shader Boids                   │
│  instance_renderer.rs : GPU インスタンス描画              │
│  text.rs       : fontdue テキストラスタライズ             │
│  input.rs      : HID 入力状態管理                        │
└─────────────────────────────────────────────────────────┘
```

### 主要ファイル一覧

| ファイル | 役割 |
|---------|------|
| `__init__.py` | Python API の全エクスポート・シンプル API 実装 |
| `entity.py` | ECS コア（Entity / Component / World） |
| `physics.py` | 2D 物理（Rigidbody / BoxCollider / PhysicsSystem） |
| `physics3d.py` | 3D 物理（AABB / RigidBody3D） |
| `tilemap.py` | タイルマップ描画・衝突判定 |
| `ui.py` | UI コンポーネント群（Button / Label / MessageWindow 等） |
| `vrm_avatar.py` | VRM 統合管理クラス |
| `vrm_anim.py` | VRM アニメーター |
| `vrm_spring.py` | スプリングボーンシミュレーション |
| `bvh_player.py` | BVH モーションプレイヤー |
| `fbx_player.py` | FBX モーションプレイヤー |
| `effects.py` | パーティクル・ダメージ数字・フラッシュ |
| `event_bus.py` | イベントバス |
| `bgm_sync.py` | BGM 同期・リズムゲームシステム |
| `anim_state.py` | 2D アニメーションステートマシン |
| `timeline.py` | タイムライン・キーフレームアニメ |
| `mapgen.py` | 手続き生成マップ（街・ダンジョン・フィールド） |
| `assets.py` | アセット管理（パス解決・キャッシュ） |
| `scriptable.py` | Scriptable Object（JSON データ駆動設計） |
| `camera.py` | 2D カメラ（スクロール・シェイク） |
| `camera3d.py` | 3D カメラ（パース・ビュー行列） |
| `editor_app.py` | Tkinter ベース エディタ |
| `launcher.py` | エディタ起動スクリプト |

---

## 3. セットアップ / ビルド方法

### 必要環境

| ツール | バージョン | 用途 |
|--------|-----------|------|
| Python | 3.10 以上 | ゲームスクリプト実行 |
| Rust | 1.70 以上 (`rustup` 推奨) | コアのビルド |
| maturin | 1.x | Rust → Python バインディングビルド |

### インストール手順

```bash
# 1. Rust のインストール（未インストールの場合）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. maturin のインストール
pip install maturin

# 3. リポジトリをクローン
git clone https://github.com/yourname/kagra.git
cd kagra

# 4. 開発用ビルド（仮想環境推奨）
python -m venv .venv
source .venv/bin/activate        # Windows: .venv\Scripts\activate
pip install maturin

maturin develop                  # Rust コアをビルドして Python から使えるようにする

# 5. 依存パッケージ（Python 側）
pip install numpy                # 一部モジュールで使用
```

### ビルド確認

```python
import kagra
kagra.init(1280, 720, "Hello KAGRA")
print("OK!")
```

> **注意**: `maturin develop` は Rust コードを変更するたびに再実行が必要です。  
> Python コード (`kagra/*.py`) の変更は即時反映されます。

---

## 4. 基本的な使い方

### 最小構成

```python
import kagra

kagra.init(1280, 720, "My Game")  # 幅, 高さ, タイトル

def update(dt):
    if kagra.pressed("ESCAPE"):
        exit()

def draw():
    kagra.cls(0, 0, 0)           # 黒でクリア
    kagra.text(100, 100, "Hello KAGRA!", size=36)

kagra.run(update=update, draw=draw)
```

### Scene クラスを使った構成（推奨）

```python
import kagra

class TitleScene(kagra.Scene):
    def on_enter(self):
        self.font = kagra.font("meiryo")  # フォント読み込み

    def update(self, dt):
        if kagra.pressed("RETURN"):
            kagra.go(GameScene())         # シーン遷移

    def draw(self):
        kagra.cls(10, 10, 40)
        kagra.text(400, 300, "PRESS ENTER", font=self.font, size=48)

class GameScene(kagra.Scene):
    def on_enter(self):
        self.x = 100.0

    def update(self, dt):
        if kagra.key("RIGHT"):
            self.x += 200 * dt
        if kagra.pressed("ESCAPE"):
            kagra.pop()

    def draw(self):
        kagra.cls(20, 20, 50)
        kagra.fill(self.x, 300, 64, 64, color=(80, 200, 255))

kagra.init(1280, 720, "My Game")
kagra.run(start_scene=TitleScene())
```

---

## 5. シーン管理

シーンはスタック構造で管理されます。

```python
kagra.go(scene)    # 現在のシーンを破棄して遷移（on_exit → on_enter）
kagra.push(scene)  # 現在のシーンを残してスタックに積む（on_pause → on_enter）
kagra.pop()        # 直前のシーンに戻る（on_exit → on_resume）
```

### ライフサイクル

| メソッド | タイミング |
|---------|-----------|
| `on_enter()` | シーンがアクティブになったとき（最初 or push 後） |
| `on_exit()` | シーンが破棄されるとき |
| `on_pause()` | push で上にシーンが積まれたとき |
| `on_resume()` | pop で戻ってきたとき |
| `update(dt)` | 毎フレーム（dt = 経過秒数） |
| `draw()` | 毎フレーム（update の後） |

### 使用例（ポーズ画面）

```python
class GameScene(kagra.Scene):
    def update(self, dt):
        if kagra.pressed("ESCAPE"):
            kagra.push(PauseScene())   # ゲームを止めずにポーズ画面を表示

class PauseScene(kagra.Scene):
    def update(self, dt):
        if kagra.pressed("ESCAPE"):
            kagra.pop()                # ゲームに戻る
```

---

## 6. 描画 API

### シンプル API（推奨）

```python
# 画面クリア
kagra.cls(r, g, b)                        # 例: kagra.cls(0, 0, 0)

# テキスト
font = kagra.font("meiryo")               # フォント読み込み（キャッシュ済）
kagra.text(x, y, "テキスト", font=font, size=24, color=(255,255,255))

# 画像
tex = kagra.load("assets/img/player.png") # テクスチャ読み込み
kagra.image(tex, x, y, w=64, h=64)
kagra.image(tex, x, y, w=64, h=64, alpha=0.5, rotation_deg=45.0)

# 矩形（塗りつぶし）
kagra.fill(x, y, w, h, color=(255, 0, 0))
kagra.fill(x, y, w, h, color=(0, 200, 100, 128))  # アルファ付き

# プログレスバー
kagra.bar(x, y, w, h, value=75, max_value=100, fill=(60, 220, 80))

# 円（スキャンライン方式）
kagra.circle(x, y, radius=32, r=255, g=100, b=100)
```

### 低レベル API

```python
# テクスチャをサブ矩形で描画（スプライトシート対応）
kagra.draw_texture(
    tid,          # テクスチャID
    x, y,         # 描画先
    w=64, h=64,   # 描画サイズ
    sx=0, sy=0,   # ソース矩形（UV）
    sw=16, sh=16, # ソースサイズ
    alpha=1.0,
    rotation_deg=0.0,
    flip_x=False,
)

# カスタムシェーダー
shader_id = kagra.load_shader_src(wgsl_source_code)
kagra.image(tex, x, y, shader_id=shader_id, shader_params=[time, 0, 0, 0])
```

### テキストサイズ取得

```python
w, h = kagra.measure(font_id, "テキスト", size=24)
# 中央揃えの例
kagra.text(screen_w//2 - w//2, y, "テキスト", font=font_id, size=24)
```

---

## 7. 入力 API

```python
# キーボード
kagra.key("LEFT")          # 押し続けているか (bool)
kagra.pressed("Z")         # このフレームで押したか（1フレームのみ True）
kagra.released("SPACE")    # このフレームで離したか

# デフォルトキー名
# UP / DOWN / LEFT / RIGHT / Z / X / SPACE / RETURN / ESCAPE
# A / B / C / D / S / U / I / K / L

# マウス
x, y = kagra.mouse()          # マウス座標
kagra.mouse_btn(1)             # 左ボタン押し続け (1=左, 2=右, 3=中)
kagra.mouse_click(1)           # このフレームでクリックしたか

# 日本語入力
chars = kagra.get_typed_chars()   # 確定入力文字列（\x08 = バックスペース）
preedit = kagra.get_preedit_text() # IME 変換中テキスト
kagra.set_ime_cursor_pos(x, y)    # IME 候補ウィンドウ位置を指定

# キーマップのカスタマイズ
# keymap.json を作成してキーコードをカスタマイズ可能
# {
#   "Z": 29,
#   "JUMP": 44
# }
```

---

## 8. ECS（Entity / Component / World）

### 基本的な使い方

```python
from kagra.entity import Entity, World, Transform, SpriteRenderer

world = World()

# Entity 作成
player = Entity(name="Player")
player.add(Transform(x=100, y=300))
player.add(SpriteRenderer(texture_id=tex, w=64, h=64))
world.add(player)

# カスタムコンポーネント（Script を継承）
class PlayerScript(kagra.Script):
    def start(self):
        self.speed = 200.0

    def update(self, dt):
        tf = self.entity.get(Transform)
        if kagra.key("RIGHT"):
            tf.x += self.speed * dt

player.add(PlayerScript())

# World の更新・描画
def update(dt):
    world.update(dt)

def draw():
    world.draw()
```

### 組み込みコンポーネント

| コンポーネント | 説明 |
|--------------|------|
| `Transform` | 位置・回転・スケール（親子関係対応） |
| `Sprite` / `SpriteRenderer` | スプライト描画 |
| `TextRenderer` | テキスト描画 |
| `RigRenderer` | スケルトンメッシュ描画 |
| `RectRenderer` | 矩形描画 |
| `AnimatorComponent` | アニメーション管理 |
| `Collider` | 衝突判定ボックス |
| `Script` | カスタムスクリプトの基底クラス |
| `TopDownMovement` | 8 方向移動コンポーネント |
| `FourDirAnimator` | 4 方向アニメーター |
| `CameraFollower` | カメラ追従コンポーネント |

### 親子関係

```python
parent = Entity(name="Parent")
parent.add(Transform(x=200, y=200))

child = Entity(name="Child")
child.add(Transform(x=50, y=0))  # 親からの相対座標

child_tf = child.get(Transform)
parent_tf = parent.get(Transform)
child_tf.set_parent(parent_tf)

# world_x / world_y でワールド座標を取得
print(child_tf.world_x)  # → 250
```

---

## 9. 物理エンジン

### 2D 物理（Physics）

```python
from kagra.physics import Rigidbody, BoxCollider, PhysicsSystem

physics = PhysicsSystem(gravity=980.0)
physics.set_tilemap(tilemap)  # タイルマップとの衝突を有効化

# Entity に追加
rb  = player.add(Rigidbody(gravity=980.0))
col = player.add(BoxCollider(w=32, h=48, offset_x=0, offset_y=0))
col.layer = "player"
col.mask  = ["enemy", "wall"]   # この layer の Entity とだけ衝突判定

# 力を加える（ジャンプ）
rb.vy = -600.0

# 毎フレーム
def update(dt):
    physics.update(dt, world)
```

### 衝突イベント

```python
kagra.on("collision", on_collision)

def on_collision(data):
    a = data["entity_a"]
    b = data["entity_b"]
    # overlap_x / overlap_y で押し返し量を取得
```

### トップダウン物理（TopDownPhysicsSystem）

```python
from kagra.physics import TopDownPhysicsSystem

physics = TopDownPhysicsSystem()
physics.set_tilemap(tilemap)
# 毎フレーム
physics.update(dt, world)
```

### 3D 物理（Physics3D）

ゲーム用キャラクターコントローラ。回転積分はしない。

```python
from kagra.physics3d import Physics3D

phys = Physics3D(gravity=9.8)
player = phys.add_capsule(0, 1.0, 0, radius=0.25, height=1.7)
wall = phys.add_obb(3, 0, 0, 0.4, 2.0, 2.0, yaw=0.4, is_static=True)
zone = phys.add_body(0, 0, 2, 2, 2, 2, trigger=True, is_static=True)

def update(dt):
    player.vx = speed_x
    player.vz = speed_z
    phys.update(dt)
    phys.sync_vrm(player, avatar)  # set_vrm_offset
```

ピック: `cam.ray_from_screen(sx, sy)` / `avatar.pick(sx, sy)` → `"head"` など。
ブルーム: `kagra.set_bloom(threshold=0.85, intensity=0.35)` — 高輝度だけ加算。

---

## 10. タイルマップ

```python
from kagra.tilemap import TileSet, TileMap, TILE_SOLID, TILE_WATER, TILE_LADDER

# タイルセット読み込み
tex = kagra.load("assets/tiles/dungeon.png")
tileset = TileSet(tex, tile_w=16, tile_h=16, spacing=0)

# タイルデータ（2D 配列）
data = [
    [1, 1, 1, 1, 1],
    [1, 0, 0, 0, 1],
    [1, 0, 0, 0, 1],
    [1, 1, 1, 1, 1],
]

# タイル属性（タイルID → フラグ）
attrs = {1: TILE_SOLID}

tilemap = TileMap(tileset, data, attrs, tile_w=16, tile_h=16)

# 描画
def draw():
    tilemap.draw(camera)

# 当たり判定クエリ
solid = tilemap.is_solid(world_x, world_y)
tile_id = tilemap.get_tile(col, row)
tilemap.set_tile(col, row, new_tile_id)
```

### タイル属性フラグ

| 定数 | 値 | 意味 |
|------|-----|------|
| `TILE_SOLID` | 0x01 | 衝突あり |
| `TILE_WATER` | 0x02 | 水（歩行不可） |
| `TILE_LADDER` | 0x04 | はしご |
| `TILE_DOOR` | 0x08 | ドア（マップ遷移トリガー） |
| `TILE_DAMAGE` | 0x10 | ダメージ床 |

---

## 11. アニメーションシステム

### 2D スプライトアニメ（AnimStateMachine）

```python
from kagra.anim_state import AnimStateMachine

anim = AnimStateMachine(tileset, tile_w=16, tile_h=16)
anim.add_state("idle",   frames=[0, 1],       fps=4,  loop=True)
anim.add_state("walk",   frames=[4, 5, 6, 7], fps=10, loop=True)
anim.add_state("attack", frames=[8, 9, 10],   fps=12, loop=False, next="idle")

# 毎フレーム
anim.update(dt)
anim.draw(x, y, w=64, h=64, flip_x=facing_left)

# 状態遷移
anim.transition("walk")
anim.transition("attack")

# ワンショット終了確認
if anim.finished("attack"):
    anim.transition("idle")
```

### スケルトン 2D アニメ（Skeleton）

```python
from kagra.skeleton import Skeleton, SkeletonAnimator, AnimationClip, Bone

# JSON または コードでスケルトンを定義
skel = Skeleton()
root = Bone("root", x=0, y=0)
arm  = Bone("arm",  x=32, y=0, parent=root)
skel.add(root); skel.add(arm)

animator = SkeletonAnimator(skel)
clip = AnimationClip("wave")
# キーフレームを追加...
animator.play("wave")

def update(dt):
    animator.update(dt)

def draw():
    animator.draw(x, y)
```

---

## 12. VRM / 3D キャラクター

### シンプル API（推奨）

```python
class GameScene(kagra.Scene):
    def on_enter(self):
        # 1行でキャラを読み込み
        self.av = kagra.avatar("assets/Emma.vrm")

        # モーションを登録（BVH または FBX）
        self.av.load_motion("idle",  "assets/motions/idle.bvh")
        self.av.load_motion("walk",  "assets/motions/walk.bvh")
        self.av.load_motion("dance", "assets/motions/dance.fbx")

    def update(self, dt):
        if kagra.key("RIGHT"):
            self.av.play("walk")
        else:
            self.av.play("idle")

        self.av.update(dt)  # アニメ + スプリングボーン + まばたき

    def draw(self):
        kagra.cls(20, 20, 40)
        kagra.draw_vrm(self.av.vrm_id)
```

### 表情・ブレンドシェイプ

```python
# 表情を設定
self.av.set_expression("Fcl_ALL_Joy",     0.8)  # 喜び
self.av.set_expression("Fcl_ALL_Sorrow",  0.5)  # 悲しみ
self.av.set_expression("Fcl_MTH_A",       1.0)  # 口を開ける

# 全表情リセット
self.av.reset_expressions()

# 利用可能な表情名を取得
print(self.av.expressions)

# まばたき制御
self.av.blink_enabled = True   # 自動まばたき ON/OFF
```

### ボーン直接操作

```python
import kagra

# ボーン回転（クォータニオン [x, y, z, w]）
kagra._engine.set_vrm_bone_rot(vrm_id, "Head", 0.0, 0.1, 0.0, 0.995)

# ボーン移動
kagra._engine.set_vrm_bone_trans(vrm_id, "Hips", 0.0, 0.05, 0.0)

# T ポーズに戻す
kagra._engine.reset_vrm_pose(vrm_id)
```

### カメラ設定

```python
# VRM 用の 3D カメラ（投影行列を設定）
kagra.set_3d_camera(
    eye_x=0.0, eye_y=1.2, eye_z=3.0,   # カメラ位置
    at_x=0.0,  at_y=1.0, at_z=0.0,     # 注視点
    fov_deg=45.0
)
```

---

## 13. UI システム

### 基本 UI コンポーネント

```python
from kagra.ui import Panel, Label, Button, ProgressBar, VBox, HBox

# シンプルボタン
clicked = kagra.button(x, y, w=200, h=60, label="スタート")

# クラスベース Button
btn = Button(x, y, w=200, h=60, label="スタート",
             on_click=lambda: kagra.go(GameScene()))
btn.update(dt)
btn.draw()

# ラベル
lbl = Label(x, y, "スコア: 0", font=font_id, size=20)
lbl.draw()

# パネル（背景矩形）
panel = Panel(x, y, w=300, h=200, color=(0, 0, 0, 180))
panel.draw()

# プログレスバー
bar = ProgressBar(x, y, w=200, h=20, max_value=100)
bar.value = 75
bar.draw()
```

### VBox / HBox（レイアウト）

```python
vbox = VBox(x=100, y=100, spacing=10)
vbox.add(Label(0, 0, "タイトル", size=24))
vbox.add(Button(0, 0, w=200, h=50, label="スタート", on_click=start))
vbox.add(Button(0, 0, w=200, h=50, label="終了",   on_click=quit))
vbox.layout()  # 位置を自動計算

def update(dt):
    vbox.update(dt)
def draw():
    vbox.draw()
```

### メッセージウィンドウ（RPG 向け）

```python
from kagra.ui import MessageWindow, DialogScript, ChoiceMenu

# 台詞表示（文字送りアニメ付き）
msg = MessageWindow(x=40, y=560, w=1200, h=140, font_id=font)
msg.show("こんにちは！いっしょに冒険しましょう！")

def update(dt):
    msg.update(dt)
    if kagra.pressed("Z"):
        msg.advance()  # 次のページへ

# 選択肢
choice = ChoiceMenu(["はい", "いいえ", "どちらでも"], font_id=font)
choice.update(dt)
if choice.selected is not None:
    print(f"選択: {choice.selected}")

# スクリプト形式（ノベルゲーム）
script = DialogScript([
    {"speaker": "ナレーター", "text": "物語は始まる..."},
    {"speaker": "主人公",     "text": "なんだここは？"},
    {"choices": ["進む", "戻る"], "var": "choice_result"},
])
```

### セーブ / ロード

```python
from kagra.ui import SaveLoad

sl = SaveLoad("save_data")
sl.save({"hp": 80, "score": 1200, "level": 3}, slot=1)

data = sl.load(slot=1)
if data:
    hp = data["hp"]
```

### Tween（UI アニメ）

```python
from kagra.ui import Tween, TweenManager, Easing

tween = Tween(
    start=0.0, end=1.0, duration=0.5,
    easing=Easing.out_bounce,
    on_update=lambda v: setattr(panel, "alpha", v),
    on_complete=lambda: print("完了！")
)

manager = TweenManager()
manager.add(tween)

def update(dt):
    manager.update(dt)
```

---

## 14. オーディオ

```python
# BGM 再生
kagra.bgm("assets/audio/title.ogg")               # ループ ON（デフォルト）
kagra.bgm("assets/audio/ending.ogg", loop=False, vol=0.6)

# 効果音
kagra.se("assets/audio/coin.wav")
kagra.se("assets/audio/explosion.wav", vol=0.8)

# 低レベル API
kagra.play_bgm("assets/audio/bgm.ogg", loop_=True, volume=0.8)
kagra.play_se("assets/audio/jump.wav", volume=1.0)
kagra.stop_bgm()

# 対応フォーマット
# OGG Vorbis（BGM 推奨）/ WAV（SE 推奨）/ MP3
```

---

## 15. エフェクト

```python
from kagra.effects import EffectManager

effects = EffectManager()
effects.set_font(font_id)  # 数字表示用フォントを設定

# 一行でエフェクト生成
effects.damage(x, y, 42)           # 赤のダメージ数字が浮かび上がる
effects.heal(x, y, 20)             # 緑の回復数字
effects.slash(x, y)                # 剣閃エフェクト
effects.spark(x, y, count=12)      # 火花パーティクル
effects.levelup(x, y)             # LEVEL UP! テキスト
effects.flash(r=255, g=255, b=255) # 画面フラッシュ

def update(dt):
    effects.update(dt)

def draw():
    effects.draw()
```

---

## 16. イベントバス

モジュール間の疎結合通信に使います。

```python
import kagra

# リスナー登録
kagra.on("player_died",    on_player_died)
kagra.on("score_changed",  hud.update, priority=10)
kagra.once("level_clear",  show_result)  # 1 回だけ

# 発火
kagra.emit("player_died",   {"x": px, "y": py})
kagra.emit("score_changed", {"score": 9999})

# 遅延発火（描画中に emit しても安全）
kagra.emit("hit", data, deferred=True)
kagra.flush_events()  # update() の末尾で呼ぶ

# 登録解除
kagra.off("player_died", on_player_died)
kagra.off_all("player_died")

# シーンごとに独立したバスを使う場合
from kagra.event_bus import EventBus
bus = EventBus()
bus.on("enemy_killed", on_enemy_killed)
bus.emit("enemy_killed", {"x": 100})
bus.flush()
```

---

## 17. Tween / イージング

```python
from kagra.ui import Tween, Easing

# 利用可能なイージング関数
Easing.linear
Easing.in_quad / out_quad / in_out_quad
Easing.in_cubic / out_cubic / in_out_cubic
Easing.in_sine / out_sine / in_out_sine
Easing.out_bounce
Easing.out_elastic

# 使用例
tween = Tween(
    start=0.0, end=300.0, duration=1.0,
    easing=Easing.out_bounce,
    loop=False,
    ping_pong=True  # 往復アニメ
)

def update(dt):
    tween.update(dt)
    enemy.x = tween.value
    if tween.done:
        print("アニメ完了")
```

---

## 18. タイムライン

```python
from kagra.timeline import Timeline, Track, EntityAnimTrack, CameraTrack, EventTrack

tl = Timeline(name="intro", duration=5.0)

# Entity のプロパティをアニメーション
track = Track(target=player.get(Transform), prop="x")
track.add_key(time=0.0, value=0.0,   easing="ease_in_out")
track.add_key(time=2.0, value=400.0, easing="ease_out")
tl.add_track(track)

# イベントトラック
event_track = EventTrack()
event_track.add_event(time=1.5, name="boss_roar")
tl.add_track(event_track)

# 再生
from kagra.timeline import TimelinePlayer
player = TimelinePlayer(tl)
player.play()

def update(dt):
    player.update(dt)
    if player.finished:
        print("イントロ終了")

# JSON 保存 / 読み込み
from kagra.anim_io import save_timeline, load_timeline
save_timeline(tl, "saves/intro.json")
tl2 = load_timeline("saves/intro.json")
```

---

## 19. アセット管理

```python
import kagra

# AssetManager 経由（推奨）
kagra.assets.base_dir  = "assets"  # デフォルト
kagra.assets.image_dir = "img"     # デフォルト

# テクスチャ（キャッシュ自動）
tex = kagra.assets.image("player/front")  # → assets/img/player/front.png

# タイルセット
ts = kagra.assets.tileset("tiles/dungeon", 16, 16)

# フォント（システムフォント自動検索）
font = kagra.assets.font("meiryo")   # Windows/Mac/Linux 自動対応
font = kagra.assets.font("gothic")
font = kagra.assets.font("mincho")

# 直接読み込み（低レベル）
tex  = kagra.load("path/to/image.png")
font = kagra.load_font("path/to/font.ttf")
```

### アセットスキャン

```python
from kagra.asset_scan import scan_assets

# assets/ ディレクトリを自動スキャンして一覧を取得
manifest = scan_assets("assets/")
for entry in manifest.entries:
    print(entry.path, entry.type)
```

---

## 20. 手続き生成マップ

```python
from kagra.mapgen import MapGen, TownTiles, DungeonTiles, FieldTiles

# 街マップ生成
data = MapGen.town(cols=30, rows=24, seed=42)

# ダンジョン生成（部屋＋通路）
data = MapGen.dungeon(cols=40, rows=30, seed=123)

# フィールド生成（地形ノイズ）
data = MapGen.field(cols=50, rows=40, seed=999)

# TileMap として使用
from kagra.tilemap import TileMap, TILE_SOLID
attrs = {TownTiles.WALL_H: TILE_SOLID, TownTiles.WALL_V: TILE_SOLID}
tilemap = TileMap(tileset, data, attrs)

# 個別タイルを手動で配置
data[5][8] = TownTiles.SHOP_SIGN
```

---

## 21. BGM 同期 / リズムゲーム

```python
from kagra.bgm_sync import BgmSync, BgmCue, RhythmJudge, LiveScore

# 振り付けデータ
CHOREO = [
    {"time": 1.0, "cue": "UP",    "pose": "arm_up"},
    {"time": 2.5, "cue": "RIGHT", "pose": "wave_right"},
    {"time": 4.0, "cue": "JUMP",  "pose": "jump"},
]

class DanceScene(kagra.Scene):
    def on_enter(self):
        self.sync = BgmSync(CHOREO, bpm=128)
        kagra.bgm("assets/audio/song.ogg")
        self.sync.start()
        kagra.on("bgm_cue", self._on_cue)

        # リズム判定
        self.judge = RhythmJudge(bpm=128)
        self.score = LiveScore()

    def _on_cue(self, data):
        if data["cue"] == "UP":
            self.avatar.play("arm_up")

    def update(self, dt):
        self.sync.update(dt)

        # プレイヤーの入力を判定
        if kagra.pressed("UP"):
            result = self.judge.judge(self.sync.current_beat)
            self.score.add(result)  # "PERFECT" / "GOOD" / "MISS"
```

---

## 22. Boids シミュレーション

### CPU Boids

```python
# 群れの生成（CPU / rayon 並列処理）
boid_id = kagra.create_boids(count=500, width=1280, height=720)
kagra.update_boids(boid_id, dt)
kagra.draw_boids(boid_id, texture_id, size=8.0)
```

### GPU Boids（Compute Shader）

```python
# GPU で数千体を処理
boid_id = kagra.create_boids_gpu(count=5000, width=1920, height=1080)
kagra.update_boids_gpu(boid_id, dt)
kagra.draw_boids_gpu(boid_id, texture_id, size=6.0)
```

### インスタンスレンダラー

```python
from kagra.instances import InstanceBatch

batch = InstanceBatch(texture_id=tex, w=16, h=16)
for i in range(1000):
    batch.add(x=random.uniform(0, 1280), y=random.uniform(0, 720),
              rotation=random.uniform(0, 360), alpha=0.8)
batch.draw()
batch.clear()
```

---

## 23. Scriptable Object / DataObject

JSON ファイルからゲームデータを読み込み、Entity に展開するデータ駆動設計システムです。

```python
# data/enemies/goblin.json
# {
#   "_type": "enemy",
#   "name": "ゴブリン",
#   "hp": 30,
#   "speed": 80,
#   "sprite": "enemies/goblin"
# }

import kagra
from kagra.scriptable import spawn_rule

# SpawnRule を定義
@kagra.spawn_rule("enemy")
def build_enemy(data, entity):
    entity.add(EnemyScript(hp=data.hp, speed=data.speed))
    entity.add(SpriteRenderer(kagra.load(data.sprite), 32, 32))

# データ読み込みと Entity 生成
goblin_data = kagra.load_data("enemies/goblin")
goblin = kagra.spawn_from(goblin_data, world)

# ドット記法でアクセス
print(goblin_data.hp)       # → 30
print(goblin_data["name"])  # → "ゴブリン"
```

---

## 24. カメラ（2D / 3D）

### 2D カメラ

```python
from kagra.camera import Camera

camera = Camera(screen_w=1280, screen_h=720)

# 追従
camera.follow(target_x=player.x, target_y=player.y, speed=5.0)

# 境界制限
camera.set_bounds(0, 0, map_pixel_w, map_pixel_h)

# カメラシェイク
camera.shake(duration=0.5, strength=8.0)

# 描画時に渡す
def draw():
    tilemap.draw(camera)
    world.draw(camera)
```

### 3D カメラ

```python
from kagra.camera3d import Camera3D

cam3d = Camera3D()
cam3d.position = [0.0, 1.6, 3.0]
cam3d.look_at   = [0.0, 1.0, 0.0]
cam3d.fov       = 60.0

# 投影行列を engine に送信
cam3d.apply()

# TPS カメラ（回転追従）
cam3d.set_tps(target_pos, yaw=self.yaw, pitch=self.pitch, distance=3.0)
```

---

## 25. エディタ

KAGRA には Tkinter ベースの GUI エディタが同梱されています。

### 起動方法

```bash
python launcher.py
# または
python -m kagra.editor_app
```

### 機能

| 機能 | 説明 |
|------|------|
| Hierarchy パネル | Entity ツリーの表示・選択 |
| Inspector | 選択 Entity のプロパティ編集 |
| Timeline エディタ | キーフレームの追加・削除 |
| シーン保存 / 読み込み | JSON 形式で保存 |
| ランタイム起動 / 停止 | エディタからゲームを実行 |

---

## 26. サンプルゲーム一覧

| ファイル | 内容 | 学べること |
|---------|------|-----------|
| `2Daction.py` | 防衛型プラットフォーマー（約 830 行） | Physics / TileMap / ECS / UI / Effects / EventBus の総合活用 |
| `3Dmaze.py` | 3D TPS 迷路ゲーム | Camera3D / VRM / 手続き生成迷路 |
| `space.py` | スペースシューター（縦スクロール） | ECS / ボス戦 / Boids |
| `othello.py` | オセロゲーム | UI / ゲームロジック |
| `vrm_romance_v2.py` | VRM ロマンスシミュレーター | VrmAvatar / 表情 / BGM 同期 |
| `boids_night_sky.py` | 夜空の Boids デモ | GPU Boids / パーティクル |

### サンプルの実行

```bash
python 2Daction.py
python 3Dmaze.py
python space.py
```

---

## 27. 他の人に使ってもらう方法

### Step 1: リポジトリの整備

```
kagra/
├── README.md                   ← このガイド（短縮版）
├── KAGRA_ENGINE_GUIDE.md       ← 本ドキュメント
├── pyproject.toml              ← maturin 設定
├── Cargo.toml                  ← Rust 依存関係
├── kagra/                      ← Python パッケージ
│   ├── __init__.py
│   └── ...
├── src/                        ← Rust ソース
│   ├── lib.rs
│   └── ...
├── examples/                   ← サンプルゲーム（移動推奨）
│   ├── 2Daction.py
│   ├── 3Dmaze.py
│   └── ...
└── assets/                     ← サンプルアセット
```

### Step 2: pyproject.toml の設定

```toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "kagra"
version = "0.1.0"
description = "KAGRA Game Engine - 2D/3D game engine with VRM support"
requires-python = ">=3.10"
dependencies = ["numpy"]

[tool.maturin]
features = ["pyo3/extension-module"]
python-source = "."
```

### Step 3: GitHub への公開

```bash
# .gitignore に追加すべきもの
target/          # Rust ビルドキャッシュ
*.pyd            # Windows ビルド成果物
*.so             # Linux/Mac ビルド成果物
__pycache__/
.venv/
```

### Step 4: README.md の最小テンプレート

```markdown
# KAGRA ゲームエンジン

Rust + Python の 2D/3D ゲームエンジン。VRM キャラ対応。

## クイックスタート

1. Rust をインストール: https://rustup.rs
2. pip install maturin
3. git clone このリポジトリ
4. maturin develop
5. python examples/2Daction.py
```

### Step 5: Wheel の配布（オプション）

ビルド済みの `.whl` ファイルを配布すれば、受け取った人は `maturin` や Rust が不要になります。

```bash
# ビルド済みホイールを作成
maturin build --release

# → target/wheels/kagra-0.1.0-cp311-cp311-win_amd64.whl
# GitHub Releases に添付すると便利
```

受け取った側のインストール：

```bash
pip install kagra-0.1.0-cp311-cp311-win_amd64.whl
```

---

## 28. 今後の改善点・ロードマップ（履歴。現行は docs/ROADMAP.ja.md）

### 🔴 優先度：高

| 課題 | 詳細 | 対策案 |
|------|------|--------|
| **ビルド自動化** | 現状は `maturin develop` を手動実行 | `setup.py` / `Makefile` でワンコマンド化 |
| **パッケージ配布** | `pip install kagra` が現状できない | GitHub Releases に OS 別 `.whl` を置く |
| **エラーメッセージ改善** | `kagra_core が見つかりません` だけでは初心者が詰まる | 詳細な解決手順を含むメッセージに変更 |
| **テスト整備** | test ファイルが 2 つのみ | pytest でユニットテスト・シーンテストを追加 |

### 🟡 優先度：中

| 課題 | 詳細 | 対策案 |
|------|------|--------|
| **エディタの強化** | Tkinter Inspector が最小限 | Transform の数値スピナー / カラーピッカー追加 |
| **シェーダーシステム** | WGSL を直書きする必要がある | マテリアル / エフェクトライブラリの高レベル API |
| **3D 物理の拡張** | AABB のみ、メッシュ衝突未対応 | OBB / カプセル衝突の追加 |
| **アニメーションブレンド** | VRM のモーション間補間が単純 | ブレンドツリー / レイヤードアニメーション |
| **Asset Hot Reload** | ファイル変更時に手動再起動が必要 | `watchdog` ライブラリで自動リロード |

### 🟢 優先度：低（将来の拡張）

| 課題 | 詳細 |
|------|------|
| **Web 対応** | WebAssembly（Pyodide + wgpu web）へのポーティング |
| **マルチスレッド更新** | 物理・AI 処理の並列化（現状はメインスレッドのみ） |
| **ネットワーク機能** | WebSocket ベースのマルチプレイヤー |
| **スクリプト言語拡張** | Lua または独自スクリプトの組み込み |
| **CI/CD** | GitHub Actions で Windows/Mac/Linux の自動ビルド |
| **デバッグオーバーレイ** | FPS グラフ・コリジョン可視化・プロファイラ |

### よくある問題と解決策

```
Q: maturin develop でエラーが出る
A: Rust が古い可能性があります。rustup update で最新化してください。

Q: VRM が読み込めない
A: VRM 1.0 形式のみ対応しています。UniVRM でエクスポートする際に
   VRM 1.0 を選択してください。

Q: 音が出ない
A: AudioEngine の初期化に失敗しても警告のみでゲームは動作します。
   オーディオデバイスが認識されているか確認してください（Bluetooth
   ヘッドセット等は認識されない場合があります）。

Q: フォントが表示されない
A: フォントパスを絶対パスで指定するか、kagra.assets.font("meiryo")
   でシステムフォントを自動検索させてください。
```

---

## ライセンス・クレジット

- レンダリング: [wgpu](https://github.com/gfx-rs/wgpu)（Apache 2.0）
- ウィンドウ: [winit](https://github.com/rust-windowing/winit)（Apache 2.0）
- Python バインディング: [PyO3](https://github.com/PyO3/pyo3)（Apache 2.0）
- テキスト: [fontdue](https://github.com/mooman219/fontdue)（MIT）
- オーディオ: [kira](https://github.com/tesselode/kira)（MIT）

---

*このドキュメントは KAGRA Engine v3 (Phase 6) 時点の仕様に基づいています。*
