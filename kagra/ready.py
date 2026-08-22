"""run() 開始後に一度だけ呼ぶフック。

Windows では wgpu レンダラが EventLoop 内でしか作れない。
``kagra.avatar()`` / ``kagra.font()`` は ``on_ready`` か ``Scene.on_enter`` で呼ぶ。
"""
from __future__ import annotations

from typing import Callable


def wrap_on_ready(update, on_ready: Callable[[], None] | None):
    """最初の update の直前に ``on_ready`` を一度だけ挟む。"""
    if on_ready is None:
        return update

    fired = False

    def wrapped(dt):
        nonlocal fired
        if not fired:
            fired = True
            on_ready()
        if update is not None:
            return update(dt)
        return None

    return wrapped
