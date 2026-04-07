# kagra/mapscene.py
# RPGマップシーン基底クラス + DebugHUD
#
# MapScene を継承してクラス変数を上書きするだけでマップが動く:
#
#   class TownScene(kagra.MapScene):
#       map_csv   = "maps/town"          # assets/maps/town.csv
#       tileset   = "tiles/tiny_town"    # assets/img/tiles/tiny_town.png
#       tile_size = 16
#       tile_attrs = { 1: kagra.TILE_SOLID, 3: kagra.TILE_WATER }
#
#   kagra.scene.change(TownScene())

from __future__ import annotations
from typing import Optional, TYPE_CHECKING

import kagra
from kagra.tilemap import TileMap
from kagra.camera  import Camera
from kagra.components import TopDownMovement, FourDirAnimator, CameraFollower

if TYPE_CHECKING:
    from kagra.tilemap import TileSet


# ── DebugHUD ──────────────────────────────────────────────────
class DebugHUD:
    """デバッグ情報をオーバーレイ表示するユーティリティ。

    Shift+D でトグル。

    表示内容:
        - FPS
        - プレイヤー座標 / タイル座標
        - 現在のタイルID と属性フラグ
        - 当たり判定ボックス（オプション）

    Example::
        self.hud = DebugHUD()

        def update(self, dt):
            self.hud.update(dt, mover, tilemap, camera)

        def draw(self):
            self.hud.draw()
    """

    FLAG_NAMES = {
        0x01: "SOLID",
        0x02: "WATER",
        0x04: "LADDER",
        0x08: "DOOR",
        0x10: "DAMAGE",
    }

    def __init__(self, enabled: bool = False):
        self.enabled  = enabled
        self._font    = 0
        self._ready   = False
        self._info: dict = {}

    def _init(self):
        if self._ready:
            return
        try:
            import kagra.assets as A
            self._font = A.font("meiryo")
        except Exception:
            self._font = 0
        self._ready = True

    def update(
        self,
        dt: float,
        mover:   Optional[TopDownMovement] = None,
        tilemap: Optional[TileMap] = None,
        camera:  Optional[Camera]  = None,
    ):
        # Shift+D でトグル
        if kagra.key_pressed(kagra.KEY_DOWN):  # TODO: Shift+D に変えたい場合は対応
            pass
        # D キーでトグル（簡易）
        import kagra as kg
        # key code for D: 7 (SDL scancode)
        # winit PhysicalKey::Code(KeyD) = 7
        if kg.key_pressed(7):  # D
            self.enabled = not self.enabled

        if not self.enabled:
            return

        self._info = {
            "fps": int(kagra.get_fps()),
            "dt":  f"{dt*1000:.1f}ms",
        }
        if mover:
            self._info["pos"]  = f"({mover.x:.0f}, {mover.y:.0f})"
            self._info["dir"]  = mover.dir
            self._info["move"] = "ON" if mover.is_moving else "off"

            if tilemap:
                col = int(mover.x // tilemap.tile_w)
                row = int(mover.y // tilemap.tile_h)
                tid = tilemap.get_tile(col, row)
                flags = tilemap.tile_attrs.get(tid, 0) if tid >= 0 else 0
                flag_str = " ".join(
                    name for f, name in self.FLAG_NAMES.items() if flags & f
                ) or "none"
                self._info["tile"] = f"({col},{row}) id={tid} [{flag_str}]"

        if camera:
            self._info["cam"] = f"({camera.x:.0f}, {camera.y:.0f})"

    def draw(self, mover: Optional[TopDownMovement] = None):
        if not self.enabled:
            return
        self._init()

        # 半透明背景パネル
        kagra.rect(4, 4, 280, 14 + len(self._info) * 18, 0, 0, 0)

        if not self._font:
            return

        y = 10
        for key, val in self._info.items():
            kagra.draw_text(self._font, f"{key}: {val}", 8, y, 13, 80, 220, 80)
            y += 17

        # 当たり判定ボックス（moverがあれば描画）
        if mover and kagra.key_down(kagra.KEY_Z):  # Z押しで当たり判定表示
            import kagra as kg
            # カメラ変換なしのワールド座標で描画（デバッグ用）
            kagra.rect(mover.x, mover.y, mover.w, mover.h, 255, 0, 255)


# ── MapScene ──────────────────────────────────────────────────
class MapScene(kagra.Scene):
    """RPGマップシーン基底クラス。

    クラス変数を上書きするだけで動くマップシーンを作れる。
    カスタムロジックは on_map_enter() / update_map() / draw_map() で追加する。

    Class variables (上書き可):
        map_csv   : CSVファイルパス（アセット名形式。複数レイヤーはlist）
        tileset   : タイルセット画像名
        tile_size : タイルサイズ (px)。[w, h] でも可
        tile_attrs: {tile_id: flag} の辞書
        map_cols  : CSVがない場合のマップ列数
        map_rows  : CSVがない場合のマップ行数
        player_start: プレイヤー初期座標 (col, row)
        player_size : プレイヤーの当たりサイズ (w, h) px
        player_speed: 移動速度 px/秒
        camera_lerp : カメラ追従の滑らかさ 0〜1
        bg_color    : 背景色 (r, g, b)
        debug       : True でデバッグHUDを表示

    Example::
        class TownScene(kagra.MapScene):
            map_csv    = "maps/town"
            tileset    = "tiles/tiny_town"
            tile_size  = 16
            tile_attrs = {
                1:  kagra.TILE_SOLID,
                2:  kagra.TILE_SOLID,
                10: kagra.TILE_WATER,
            }
            player_start = (5, 7)
            player_speed = 100
    """

    # ── クラス変数（上書き用） ──────────────────────────────
    map_csv:      str | list[str] | None = None
    tileset:      str | None             = None
    tile_size:    int | list[int]        = 16
    tile_attrs:   dict[int, int]         = {}
    map_cols:     int                    = 20
    map_rows:     int                    = 15
    player_start: tuple[int, int]        = (1, 1)
    player_size:  tuple[float, float]    = (14.0, 20.0)
    player_speed: float                  = 100.0
    camera_lerp:  float                  = 0.12
    bg_color:     tuple[int, int, int]   = (30, 30, 40)
    debug:        bool                   = False

    # プレイヤーテクスチャ辞書（上書き可）
    # 未設定の場合はカラーボックスで代替
    player_textures: dict[str, str] = {}   # {"front": "player/front", ...}

    def on_enter(self):
        self._ready = False

    def _late_init(self):
        """run()開始後の初回updateで呼ばれる初期化。"""
        import kagra.assets as A
        screen_w, screen_h = kagra.get_screen_size()

        # タイルサイズ解決
        if isinstance(self.tile_size, (list, tuple)):
            tw, th = self.tile_size[0], self.tile_size[1]
        else:
            tw = th = self.tile_size

        # タイルセット
        self._tileset = None
        if self.tileset:
            self._tileset = A.tileset(self.tileset, tw, th)

        # TileMapロード（レイヤー対応）
        self._tilemaps: list[TileMap] = []
        csv_list = ([self.map_csv] if isinstance(self.map_csv, str)
                    else self.map_csv) if self.map_csv else []

        if csv_list and self._tileset:
            for csv_name in csv_list:
                path = self._resolve_csv(csv_name)
                try:
                    tm = TileMap.from_csv(self._tileset, path, self.tile_attrs)
                    self._tilemaps.append(tm)
                except FileNotFoundError:
                    print(f"[MapScene] CSVが見つかりません: {path}")
        elif self._tileset:
            # CSVなし: 空マップを生成
            tm = TileMap.empty(
                self._tileset, self.map_cols, self.map_rows,
                tile_attrs=self.tile_attrs
            )
            self._tilemaps.append(tm)

        # カメラ
        map_pw = self._tilemaps[0].pixel_width  if self._tilemaps else screen_w
        map_ph = self._tilemaps[0].pixel_height if self._tilemaps else screen_h
        self.camera = Camera(
            screen_w=float(screen_w),
            screen_h=float(screen_h),
            world_w=float(map_pw),
            world_h=float(map_ph),
        )

        # プレイヤー移動コンポーネント
        col0, row0 = self.player_start
        px = col0 * tw + (tw - self.player_size[0]) / 2
        py = row0 * th + (th - self.player_size[1]) / 2
        collision_map = self._tilemaps[0] if self._tilemaps else None
        self.mover = TopDownMovement(
            x=px, y=py,
            w=self.player_size[0], h=self.player_size[1],
            speed=self.player_speed,
            tilemap=collision_map,
        )

        # プレイヤーアニメーション
        self._anim: Optional[FourDirAnimator] = None
        if self.player_textures:
            loaded = {k: A.image(v) for k, v in self.player_textures.items()}
            self._anim = FourDirAnimator(loaded, frame_rate=8.0)

        # カメラ追従
        self._cam_follower = CameraFollower(self.camera, lerp=self.camera_lerp)

        # デバッグHUD
        self._hud = DebugHUD(enabled=self.debug)

        # カメラを初期位置に
        self.camera.follow(
            self.mover.x, self.mover.y,
            self.mover.w, self.mover.h,
            lerp=1.0
        )

        self.on_map_enter()
        self._ready = True

    def _resolve_csv(self, name: str) -> str:
        import os
        if os.path.splitext(name)[1]:
            return name
        return os.path.join("assets", "maps", name + ".csv")

    # ── オーバーライドポイント ────────────────────────────────

    def on_map_enter(self):
        """マップ初期化完了後に呼ばれる。追加の初期化をここに書く。"""
        pass

    def update_map(self, dt: float):
        """毎フレームの追加ロジックをここに書く。
        mover / camera / hud は self.xxx でアクセスできる。
        """
        pass

    def draw_map(self):
        """タイルマップ・プレイヤー描画後に呼ばれる追加描画。"""
        pass

    # ── Scene インターフェース ────────────────────────────────

    def update(self, dt: float):
        if not self._ready:
            self._late_init()
            return

        # 入力 → 移動
        self.mover.read_kagra_input()
        self.mover.update(dt)

        # アニメーション
        if self._anim:
            self._anim.update(dt, self.mover.dir, self.mover.is_moving)

        # カメラ追従
        self._cam_follower.follow(
            self.mover.center_x, self.mover.center_y
        )

        # デバッグHUD
        collision_map = self._tilemaps[0] if self._tilemaps else None
        self._hud.update(dt, self.mover, collision_map, self.camera)

        # ESCで終了
        if kagra.key_pressed(kagra.KEY_ESCAPE):
            import sys; sys.exit(0)

        self.update_map(dt)

    def draw(self):
        if not self._ready:
            kagra.cls(*self.bg_color)
            return

        kagra.cls(*self.bg_color)

        # タイルマップ描画（全レイヤー）
        for tm in self._tilemaps:
            tm.draw(self.camera)

        # プレイヤー描画
        sx, sy = self.camera.to_screen(self.mover.x, self.mover.y)
        pw, ph = self.player_size

        if self._anim and self._anim.current_texture:
            bob = self._anim.bob_offset if self._anim else 0
            kagra.draw_texture(
                self._anim.current_texture,
                sx, sy + bob, pw, ph
            )
        else:
            # テクスチャなし: カラーボックスで代替
            kagra.rect(sx, sy, pw, ph, 100, 160, 240)

        self.draw_map()

        # デバッグHUD（最前面）
        self._hud.draw(self.mover)

    # ── プロパティ ────────────────────────────────────────────

    @property
    def tilemap(self) -> Optional[TileMap]:
        """最初のレイヤーのTileMapを返す。"""
        return self._tilemaps[0] if self._tilemaps else None

    @property
    def player_x(self) -> float:
        return self.mover.x

    @property
    def player_y(self) -> float:
        return self.mover.y
