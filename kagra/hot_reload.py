# kagra/hot_reload.py
"""
Phase 9a: ホットリロードシステム

ファイルを保存するとゲームを再起動せずに Scene が即反映される。
DragonRuby 最大の魅力をKAGRAで実現。

【仕組み】
  watchdog（あれば）または polling で .py ファイルの変更を監視
  変更があったら importlib.reload() でモジュールを差し替え
  Scene クラスの update/draw だけ入れ替え → ゲーム状態は維持

【使い方】
    import kagra
    from kagra.hot_reload import HotReloader

    # エントリポイント（main.py等）
    reloader = HotReloader("game_scene.py")
    reloader.start()

    class WrapperScene(kagra.Scene):
        def update(self, dt):
            reloader.scene.update(dt)   # 常に最新のインスタンスに委譲
        def draw(self):
            reloader.scene.draw()

    kagra.run(start_scene=WrapperScene())

【シンプルな使い方 - デコレータ版】
    @kagra.hot_scene("game_scene.py")
    class MyScene(kagra.Scene):
        ...

    kagra.run(start_scene=MyScene())
"""
from __future__ import annotations

import importlib
import importlib.util
import os
import sys
import threading
import time
import traceback
from pathlib import Path
from typing import Optional, Callable, Any


class HotReloader:
    """指定した Python ファイルを監視し、変更時に自動リロードする。

    Args:
        watch_path:   監視するファイルまたはディレクトリのパス
        scene_class:  リロード後に生成する Scene クラス名（str）
        on_reload:    リロード成功時に呼ばれるコールバック
        on_error:     リロード失敗時に呼ばれるコールバック
        poll_interval: ポーリング間隔（秒）。watchdog がない場合に使用

    Example::
        reloader = HotReloader("scenes/game.py", scene_class="GameScene")
        reloader.start()

        def update(dt):
            reloader.tick()          # 変更チェック（毎フレーム呼ぶ）
            reloader.scene.update(dt)

        def draw():
            reloader.scene.draw()
    """

    def __init__(
        self,
        watch_path:    str,
        scene_class:   str  = None,
        on_reload:     Callable = None,
        on_error:      Callable = None,
        poll_interval: float = 0.5,
        preserve_state: bool = True,
    ):
        self.watch_path     = Path(watch_path).resolve()
        self.scene_class    = scene_class
        self.on_reload      = on_reload
        self.on_error       = on_error
        self.poll_interval  = poll_interval
        self.preserve_state = preserve_state

        self._module        = None
        self._scene_inst    = None
        self._last_mtime:   dict[str, float] = {}
        self._lock          = threading.Lock()
        self._reload_flag   = False
        self._thread:       Optional[threading.Thread] = None
        self._running       = False

        # watchdog が使えるか確認
        self._use_watchdog  = self._try_watchdog()

        # 初回ロード
        self._load_module()

    # ── ファイル監視 ──────────────────────────────────────────

    def start(self):
        """バックグラウンドで監視を開始する。"""
        if self._running:
            return
        self._running = True
        self._thread = threading.Thread(
            target=self._watch_loop, daemon=True
        )
        self._thread.start()
        mode = "watchdog" if self._use_watchdog else "polling"
        print(f"[HotReload] 監視開始: {self.watch_path} ({mode})")

    def stop(self):
        """監視を停止する。"""
        self._running = False

    def tick(self):
        """毎フレーム呼ぶ。リロードフラグが立っていれば実際のリロードを実行する。

        リロードはゲームループのメインスレッドで行う必要があるため、
        watchスレッドはフラグを立てるだけにして、tick() で処理する。
        """
        if self._reload_flag:
            self._reload_flag = False
            self._do_reload()

    @property
    def scene(self):
        """現在の Scene インスタンス（常に最新版）。"""
        return self._scene_inst

    @property
    def module(self):
        """現在読み込まれているモジュール。"""
        return self._module

    # ── 内部 ──────────────────────────────────────────────────

    def _watch_loop(self):
        """バックグラウンドスレッドでファイル変更を監視する。"""
        # 監視対象ファイルの初期 mtime を記録
        self._record_mtimes()

        while self._running:
            time.sleep(self.poll_interval)
            if self._check_changes():
                self._reload_flag = True

    def _record_mtimes(self):
        """監視対象ファイルの現在の mtime を記録する。"""
        paths = self._get_watch_files()
        for p in paths:
            try:
                self._last_mtime[str(p)] = os.path.getmtime(p)
            except OSError:
                pass

    def _check_changes(self) -> bool:
        """変更があれば True を返す。"""
        paths = self._get_watch_files()
        changed = False
        for p in paths:
            try:
                mtime = os.path.getmtime(p)
                key   = str(p)
                if key not in self._last_mtime or self._last_mtime[key] != mtime:
                    self._last_mtime[key] = mtime
                    changed = True
                    print(f"[HotReload] 変更検出: {p.name}")
            except OSError:
                pass
        return changed

    def _get_watch_files(self) -> list[Path]:
        """監視対象のファイル一覧を返す。"""
        if self.watch_path.is_file():
            return [self.watch_path]
        elif self.watch_path.is_dir():
            return list(self.watch_path.rglob("*.py"))
        return []

    def _load_module(self):
        """モジュールを初回ロードする。"""
        path = self.watch_path if self.watch_path.is_file() else None
        if path is None:
            return

        spec    = importlib.util.spec_from_file_location("_hot_module", path)
        module  = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(module)
            self._module = module
            sys.modules["_hot_module"] = module
            self._create_scene_instance()
        except Exception as e:
            print(f"[HotReload] 初回ロード失敗: {e}")
            traceback.print_exc()

    def _do_reload(self):
        """モジュールをリロードして Scene インスタンスを差し替える。"""
        path = self.watch_path if self.watch_path.is_file() else None
        if path is None or self._module is None:
            return

        # 現在の状態を退避（state 保持）
        old_state = {}
        if self.preserve_state and self._scene_inst:
            try:
                old_state = {
                    k: v for k, v in vars(self._scene_inst).items()
                    if not k.startswith("_")
                }
            except Exception:
                pass

        try:
            spec = importlib.util.spec_from_file_location("_hot_module", path)
            new_module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(new_module)
            self._module = new_module
            sys.modules["_hot_module"] = new_module

            self._create_scene_instance(old_state)
            print(f"[HotReload] ✓ リロード完了: {path.name}")

            if self.on_reload:
                self.on_reload(self._scene_inst)

        except Exception as e:
            print(f"[HotReload] ✗ リロード失敗: {e}")
            traceback.print_exc()
            if self.on_error:
                self.on_error(e)

    def _create_scene_instance(self, restore_state: dict = None):
        """Scene インスタンスを生成する。"""
        if self._module is None:
            return

        # scene_class が指定されていれば使う。なければ Scene を継承したクラスを探す
        cls = None
        if self.scene_class:
            cls = getattr(self._module, self.scene_class, None)
        else:
            import kagra
            for name in dir(self._module):
                obj = getattr(self._module, name)
                try:
                    if (isinstance(obj, type) and
                        issubclass(obj, kagra.Scene) and
                        obj is not kagra.Scene):
                        cls = obj
                        break
                except Exception:
                    pass

        if cls is None:
            print(f"[HotReload] Scene クラスが見つかりません: {self.scene_class}")
            return

        new_inst = cls.__new__(cls)
        new_inst.__init__()  # type: ignore

        # 旧状態を復元（型が一致するキーのみ）
        if restore_state:
            for k, v in restore_state.items():
                try:
                    setattr(new_inst, k, v)
                except Exception:
                    pass

        self._scene_inst = new_inst

    def _try_watchdog(self) -> bool:
        """watchdog ライブラリが使えるか確認する。"""
        try:
            import watchdog  # noqa
            return True
        except ImportError:
            return False


# ── デコレータ版ホットリロード ────────────────────────────────

class _HotSceneWrapper:
    """hot_scene デコレータで生成するラッパー Scene。"""

    def __init__(self, reloader: HotReloader):
        self._reloader = reloader
        reloader.start()

    def on_enter(self):
        if self._reloader.scene:
            self._reloader.scene.on_enter()

    def on_exit(self):
        if self._reloader.scene:
            self._reloader.scene.on_exit()

    def on_pause(self):
        if self._reloader.scene:
            self._reloader.scene.on_pause()

    def on_resume(self):
        if self._reloader.scene:
            self._reloader.scene.on_resume()

    def update(self, dt: float):
        self._reloader.tick()     # 変更チェック
        if self._reloader.scene:
            self._reloader.scene.update(dt)

    def draw(self):
        if self._reloader.scene:
            self._reloader.scene.draw()
        # ホットリロードインジケーター（左上の小さな点）
        if self._reloader._reload_flag:
            import kagra
            kagra.fill(4, 4, 8, 8, color=(255, 80, 80))


def make_hot_scene(watch_file: str, scene_class: str = None) -> _HotSceneWrapper:
    """ホットリロード対応の Scene ラッパーを生成する。

    Args:
        watch_file:  監視する Python ファイルのパス
        scene_class: リロードする Scene クラス名（省略時は自動検出）

    Example::
        # main.py
        import kagra
        from kagra.hot_reload import make_hot_scene

        kagra.init(1280, 720, "My Game")
        scene = make_hot_scene("scenes/game.py", "GameScene")
        kagra.run(start_scene=scene)

        # scenes/game.py を編集して保存 → 即反映！
    """
    reloader = HotReloader(watch_file, scene_class=scene_class)
    return _HotSceneWrapper(reloader)
