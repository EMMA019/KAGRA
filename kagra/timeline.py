
import math
import json
import os


def ease_in_out(t):
    return t * t * (3 - 2 * t)


EASING_FUNCS = {
    "ease_in_out": ease_in_out,
    "linear":      lambda t: t,
    "ease_in":     lambda t: t * t,
    "ease_out":    lambda t: t * (2 - t),
}


# ════════════════════════════════════════════════════════
#  Keyframe / Track  (キーフレームアニメ用)
# ════════════════════════════════════════════════════════

class Keyframe:
    def __init__(self, time, value, easing="ease_in_out"):
        self.time   = time
        self.value  = value
        self.easing = easing

    def to_dict(self) -> dict:
        return {"time": self.time, "value": self.value, "easing": self.easing}

    @classmethod
    def from_dict(cls, d: dict) -> "Keyframe":
        return cls(time=d["time"], value=d["value"], easing=d.get("easing", "ease_in_out"))


class Track:
    """キーフレームトラック（Entity の Transform プロパティを補間する）。"""

    def __init__(self, target=None, prop: str = "", target_name: str = ""):
        self.target      = target
        self.target_name = target_name or (getattr(target, "name", "") if target else "")
        self.prop        = prop
        self.keys: list = []

    def add_key(self, time, value, easing="ease_in_out"):
        self.keys.append(Keyframe(time, value, easing))
        self.keys.sort(key=lambda k: k.time)

    def remove_key(self, time, tolerance=0.01):
        self.keys = [k for k in self.keys if abs(k.time - time) > tolerance]

    def evaluate(self, t):
        if not self.keys:
            return None
        if t <= self.keys[0].time:
            return self.keys[0].value
        for i in range(len(self.keys) - 1):
            k1, k2 = self.keys[i], self.keys[i + 1]
            if k1.time <= t <= k2.time:
                ratio = (t - k1.time) / (k2.time - k1.time)
                ease  = EASING_FUNCS.get(k1.easing, ease_in_out)
                ratio = ease(ratio)
                v1, v2 = k1.value, k2.value
                if isinstance(v1, (int, float)):
                    return v1 + (v2 - v1) * ratio
                return [v1[j] + (v2[j] - v1[j]) * ratio for j in range(len(v1))]
        return self.keys[-1].value

    def to_dict(self) -> dict:
        name = self.target_name or (getattr(self.target, "name", "") if self.target else "")
        return {"target_name": name, "prop": self.prop,
                "keys": [k.to_dict() for k in self.keys]}

    @classmethod
    def from_dict(cls, d: dict) -> "Track":
        t = cls(target=None, prop=d["prop"], target_name=d.get("target_name", ""))
        for kd in d.get("keys", []):
            t.keys.append(Keyframe.from_dict(kd))
        return t


# ════════════════════════════════════════════════════════
#  特殊トラック（Timeline.add() で使う）
# ════════════════════════════════════════════════════════

class EntityAnimTrack:
    """指定時刻にアニメーションクリップを再生するトラック。

    Args:
        target     : AnimatorComponent を持つ Entity
        clip_name  : Animator に登録されたクリップ名
        start      : 再生開始時刻（秒）
        duration   : 再生継続時間（秒）。0 でクリップ長に合わせる
        reset      : True なら開始時に time=0 からリセット
        stop_on_end: True なら duration 経過後に stop()
    """

    def __init__(self, target, clip_name: str,
                 start: float = 0.0, duration: float = 0.0,
                 reset: bool = True, stop_on_end: bool = True):
        self.target      = target
        self.clip_name   = clip_name
        self.start       = start
        self.duration    = duration
        self.reset       = reset
        self.stop_on_end = stop_on_end
        self._fired      = False
        self._stopped    = False

    def update(self, t: float):
        if t >= self.start and not self._fired:
            self._fired = True
            try:
                from kagra.entity import AnimatorComponent
                ac = self.target.get_component(AnimatorComponent)
                if ac:
                    ac.play(self.clip_name, reset=self.reset)
            except Exception:
                pass

        if self.stop_on_end and self.duration > 0:
            end = self.start + self.duration
            if t >= end and not self._stopped:
                self._stopped = True
                try:
                    from kagra.entity import AnimatorComponent
                    ac = self.target.get_component(AnimatorComponent)
                    if ac and ac.animator:
                        ac.animator.stop()
                except Exception:
                    pass

    def reset_state(self):
        self._fired   = False
        self._stopped = False


class CameraTrack:
    """カメラを指定時刻から目標座標へスムーズに移動するトラック。

    Args:
        camera   : Camera オブジェクト
        start    : 開始時刻（秒）
        duration : 移動にかける時間（秒）
        end_x    : 目標 x 座標
        end_y    : 目標 y 座標
        easing   : イージング名
    """

    def __init__(self, camera, start: float = 0.0, duration: float = 1.0,
                 end_x: float = 0.0, end_y: float = 0.0,
                 easing: str = "ease_in_out"):
        self.camera    = camera
        self.start     = start
        self.duration  = max(duration, 0.0001)
        self.end_x     = end_x
        self.end_y     = end_y
        self.easing    = easing
        self._start_x  = 0.0
        self._start_y  = 0.0
        self._captured = False

    def update(self, t: float):
        if t < self.start:
            self._captured = False
            return
        if not self._captured:
            self._start_x  = self.camera.x
            self._start_y  = self.camera.y
            self._captured = True

        progress = min((t - self.start) / self.duration, 1.0)
        ease_fn  = EASING_FUNCS.get(self.easing, ease_in_out)
        p        = ease_fn(progress)
        self.camera.x = self._start_x + (self.end_x - self._start_x) * p
        self.camera.y = self._start_y + (self.end_y - self._start_y) * p

    def reset_state(self):
        self._captured = False


class EventTrack:
    """指定時刻にコールバックを1回だけ発火するトラック。

    Args:
        time     : 発火時刻（秒）
        callback : 引数なしの callable
    """

    def __init__(self, time: float, callback):
        self.time     = time
        self.callback = callback
        self._fired   = False

    def update(self, t: float):
        if not self._fired and t >= self.time:
            self._fired = True
            try:
                self.callback()
            except Exception as e:
                print(f"[EventTrack] callback error: {e}")

    def reset_state(self):
        self._fired = False


# ════════════════════════════════════════════════════════
#  Timeline
# ════════════════════════════════════════════════════════

class Timeline:
    """キーフレーム・アニメ・カメラ・イベントトラックを束ねるタイムライン。

    Example::
        tl = Timeline(length=8.0, loop=True, name="Intro")
        tl.add(EntityAnimTrack(player, "dance", start=0.0, duration=2.0))
        tl.add(CameraTrack(cam, start=0.0, duration=3.0, end_x=400, end_y=0))
        tl.add(EventTrack(0.5, lambda: print("fired!")))
        player = TimelinePlayer(tl)
        player.play()
    """

    def __init__(self, name: str = "Timeline",
                 length: float = 0.0, loop: bool = False):
        self.name     = name
        self.loop     = loop
        self.duration = length

        self.tracks: list   = []   # Track（キーフレーム）
        self._special: list = []   # EntityAnimTrack / CameraTrack / EventTrack

        self.time    = 0.0
        self.playing = True

    def add_track(self, track: Track) -> Track:
        """キーフレームトラックを追加する（旧API互換）。"""
        self.tracks.append(track)
        return track

    def add(self, track) -> "Timeline":
        """任意のトラックを追加する。chainable。"""
        if isinstance(track, Track):
            self.tracks.append(track)
        else:
            self._special.append(track)
        return self

    def remove_track(self, track):
        if isinstance(track, Track):
            self.tracks = [t for t in self.tracks if t is not track]
        else:
            self._special = [t for t in self._special if t is not track]

    @property
    def end_time(self) -> float:
        if self.duration > 0:
            return self.duration
        t = 0.0
        for track in self.tracks:
            if track.keys:
                t = max(t, track.keys[-1].time)
        for sp in self._special:
            st  = getattr(sp, "start", getattr(sp, "time", 0.0))
            dur = getattr(sp, "duration", 0.0)
            t   = max(t, st + dur)
        return t

    def update(self, dt: float):
        if not self.playing:
            return
        self.time += dt

        end = self.end_time
        if end > 0 and self.time > end:
            if self.loop:
                self.time = self.time % end
                for sp in self._special:
                    if hasattr(sp, "reset_state"):
                        sp.reset_state()
            else:
                self.time    = end
                self.playing = False

        # キーフレームトラック
        for track in self.tracks:
            val = track.evaluate(self.time)
            if val is None or track.target is None:
                continue
            comp_dict = {}
            if hasattr(track.target, "components"):
                for c in track.target.components:
                    comp_dict[type(c).__name__] = c
            comp = comp_dict.get("Transform")
            if comp is not None:
                setattr(comp, track.prop, val)
            else:
                try:
                    setattr(track.target, track.prop, val)
                except Exception:
                    pass

        # 特殊トラック
        for sp in self._special:
            sp.update(self.time)

    def play(self):
        self.playing = True

    def pause(self):
        self.playing = False

    def seek(self, t: float):
        self.time = max(0.0, t)
        for sp in self._special:
            if hasattr(sp, "reset_state"):
                sp.reset_state()

    # ── シリアライズ ──────────────────────────────────────────

    def to_dict(self) -> dict:
        return {
            "version":  1,
            "name":     self.name,
            "loop":     self.loop,
            "duration": self.duration,
            "tracks":   [t.to_dict() for t in self.tracks],
        }

    @classmethod
    def from_dict(cls, d: dict) -> "Timeline":
        tl = cls(name=d.get("name", "Timeline"),
                 length=d.get("duration", 0.0),
                 loop=d.get("loop", False))
        for td in d.get("tracks", []):
            tl.tracks.append(Track.from_dict(td))
        return tl

    def save(self, path: str):
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.to_dict(), f, ensure_ascii=False, indent=2)

    @classmethod
    def load(cls, path: str) -> "Timeline":
        with open(path, "r", encoding="utf-8") as f:
            return cls.from_dict(json.load(f))

    def bind_entities(self, entity_map: dict):
        for track in self.tracks:
            if track.target_name in entity_map:
                track.target = entity_map[track.target_name]


# ════════════════════════════════════════════════════════
#  TimelinePlayer
# ════════════════════════════════════════════════════════

class TimelinePlayer:
    """Timeline を管理・再生するプレイヤー。

    EntityScene の add_timeline() に登録して使う。

    Example::
        tl = Timeline(length=8.0, loop=True)
        tl.add(EventTrack(1.0, callback))
        player = TimelinePlayer(tl)
        player.play()
        scene.add_timeline(player)
    """

    def __init__(self, timeline: Timeline):
        self.timeline  = timeline
        self._playing  = False
        self._finished = False

    @property
    def time(self) -> float:
        return self.timeline.time

    @property
    def finished(self) -> bool:
        return self._finished

    def play(self):
        self._playing  = True
        self._finished = False
        self.timeline.play()

    def pause(self):
        self._playing = False
        self.timeline.pause()

    def stop(self):
        self._playing  = False
        self._finished = True
        self.timeline.pause()

    def seek(self, t: float):
        self.timeline.seek(t)

    def update(self, dt: float):
        if not self._playing:
            return
        self.timeline.update(dt)
        if not self.timeline.playing and not self.timeline.loop:
            self._finished = True
            self._playing  = False
