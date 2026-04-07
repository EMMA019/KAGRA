# kagra/event_bus.py
# イベントバスシステム
#
# ── 使い方（グローバル）──────────────────────────────────────
#
#   import kagra
#
#   # リスナー登録
#   kagra.on("player_died", on_player_died)
#   kagra.on("score_changed", hud.update, priority=10)
#   kagra.once("level_clear", show_result)   # 1回だけ
#
#   # 発火
#   kagra.emit("player_died", {"x": px, "y": py})
#   kagra.emit("score_changed", {"score": 9999})
#
#   # 登録解除
#   kagra.off("player_died", on_player_died)
#   kagra.off_all("player_died")   # そのイベントの全リスナー削除
#
#   # フレーム末尾にまとめて配信（描画中に emit しても安全）
#   kagra.emit("hit", data, deferred=True)
#   kagra.flush_events()   # update() の末尾で呼ぶ
#
# ── Scene ごとに独立したバスを使う ───────────────────────────
#
#   class MyScene(kagra.Scene):
#       def on_enter(self):
#           self.bus = EventBus()
#           self.bus.on("enemy_killed", self.on_enemy_killed)
#
#       def update(self, dt):
#           self.bus.emit("enemy_killed", {"x": 100})
#           self.bus.flush()
#
# ─────────────────────────────────────────────────────────────

from __future__ import annotations
from typing import Callable, Any


class _Listener:
    __slots__ = ("callback", "priority", "once", "_dead")

    def __init__(self, callback: Callable, priority: int, once: bool):
        self.callback = callback
        self.priority = priority
        self.once     = once
        self._dead    = False

    def invoke(self, data: dict):
        if self._dead:
            return
        try:
            self.callback(data)
        except TypeError:
            # 引数なしの callable にも対応
            try:
                self.callback()
            except Exception as e:
                print(f"[EventBus] listener error ({self.callback}): {e}")
        except Exception as e:
            print(f"[EventBus] listener error ({self.callback}): {e}")
        if self.once:
            self._dead = True


class EventBus:
    """イベントバス本体。グローバルでも Scene ローカルでも使える。

    Example::
        bus = EventBus()
        bus.on("hit", lambda d: print(d["damage"]))
        bus.emit("hit", {"damage": 42})
        bus.flush()   # deferred イベントを処理
    """

    def __init__(self):
        # event_name -> list[_Listener]
        self._listeners: dict[str, list[_Listener]] = {}
        # deferred キュー
        self._deferred: list[tuple[str, dict]] = []

    # ── 登録 ─────────────────────────────────────────────────

    def on(self, event: str, callback: Callable,
           priority: int = 0, once: bool = False) -> Callable:
        """リスナーを登録する。callback をそのまま返す（デコレータ兼用）。

        Args:
            event    : イベント名
            callback : 呼ばれる関数。引数は dict 1つ、または無引数
            priority : 数値が大きいほど先に呼ばれる
            once     : True なら1回だけ受け取って自動解除
        """
        listeners = self._listeners.setdefault(event, [])
        listeners.append(_Listener(callback, priority, once))
        listeners.sort(key=lambda l: -l.priority)
        return callback

    def once(self, event: str, callback: Callable, priority: int = 0) -> Callable:
        """1回だけ受け取るリスナーを登録する。"""
        return self.on(event, callback, priority=priority, once=True)

    def off(self, event: str, callback: Callable):
        """指定リスナーを解除する。"""
        if event in self._listeners:
            for l in self._listeners[event]:
                if l.callback is callback:
                    l._dead = True

    def off_all(self, event: str):
        """指定イベントの全リスナーを解除する。"""
        self._listeners.pop(event, None)

    def clear(self):
        """全リスナー・全 deferred キューをクリアする。"""
        self._listeners.clear()
        self._deferred.clear()

    # ── 発火 ─────────────────────────────────────────────────

    def emit(self, event: str, data: dict = None, deferred: bool = False):
        """イベントを発火する。

        Args:
            event    : イベント名
            data     : リスナーに渡す辞書（省略可）。元の dict は変更されない。
            deferred : True なら flush() 呼び出し時にまとめて配信
        """
        # 呼び出し元の dict を変異させないようコピーを取る
        payload = dict(data) if data else {}
        payload.setdefault("_event", event)

        if deferred:
            self._deferred.append((event, payload))
            return

        self._dispatch(event, payload)

    def flush(self):
        """deferred キューを処理する。毎フレームの update 末尾で呼ぶ。"""
        queue = self._deferred[:]
        self._deferred.clear()
        for event, data in queue:
            self._dispatch(event, data)

    def _dispatch(self, event: str, data: dict):
        listeners = self._listeners.get(event)
        if not listeners:
            return
        # dispatch 中に on()/off() が呼ばれても安全なよう snapshot を使う
        for l in list(listeners):
            if not l._dead:
                l.invoke(data)
        # 死んだリスナーを掃除
        self._listeners[event] = [l for l in listeners if not l._dead]

    # ── ユーティリティ ────────────────────────────────────────

    def has_listeners(self, event: str) -> bool:
        """指定イベントにリスナーが登録されているか。"""
        return bool(self._listeners.get(event))

    def listener_count(self, event: str) -> int:
        return len([l for l in self._listeners.get(event, []) if not l._dead])

    def registered_events(self) -> list[str]:
        """登録されている全イベント名のリスト。"""
        return [e for e, ls in self._listeners.items()
                if any(not l._dead for l in ls)]


# ════════════════════════════════════════════════════════
#  グローバルバス（kagra.emit / kagra.on の実体）
# ════════════════════════════════════════════════════════

_global_bus = EventBus()


def get_global_bus() -> EventBus:
    """グローバル EventBus を返す。"""
    return _global_bus


def reset_global_bus():
    """シーン切り替え時などにグローバルバスをリセットする。"""
    _global_bus.clear()
