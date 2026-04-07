from __future__ import annotations
from typing import TYPE_CHECKING, Optional

# ── 描画ヘルパー（遅延インポートで循環回避） ──────────────
def _kagra():
    import kagra
    return kagra

def _draw_texture(*args, **kwargs):
    _kagra().draw_texture(*args, **kwargs)

def _draw_rect(x, y, w, h, r, g, b):
    _kagra().rect(x, y, w, h, r, g, b)


class Component:
    def __init__(self):
        self.entity: 'Entity' = None
        self.enabled: bool = True
        self._started: bool = False

    def _start_if_needed(self):
        if not self._started:
            self.start()
            self._started = True

    def start(self): pass
    def update(self, dt: float): pass
    def draw(self, camera=None): pass


class Script(Component):
    pass


class Transform(Component):
    def __init__(
        self,
        x: float = 0.0,
        y: float = 0.0,
        rotation: float = 0.0,
        scale_x: float = 1.0,
        scale_y: float = 1.0,
    ):
        super().__init__()
        self.x = x
        self.y = y
        self.rotation = rotation
        self.scale_x = scale_x
        self.scale_y = scale_y
        self.parent: Optional["Transform"] = None
        self.children: list["Transform"] = []

    def set_pos(self, x: float, y: float):
        self.x = x
        self.y = y

    def translate(self, dx: float, dy: float):
        self.x += dx
        self.y += dy

    def set_parent(self, parent: Optional["Transform"]):
        if self.parent is parent:
            return
        if self.parent and self in self.parent.children:
            self.parent.children.remove(self)
        self.parent = parent
        if parent is not None and self not in parent.children:
            parent.children.append(self)

    @property
    def world_x(self) -> float:
        if self.parent is None:
            return self.x
        return self.parent.world_x + self.x

    @property
    def world_y(self) -> float:
        if self.parent is None:
            return self.y
        return self.parent.world_y + self.y


class SpriteRenderer(Component):
    def __init__(
        self,
        texture_id: int,
        w: float,
        h: float,
        sx: float = 0, sy: float = 0, sw: float = 0, sh: float = 0,
        z_order: int = 0,
    ):
        super().__init__()
        self.texture_id = texture_id
        self.w = w
        self.h = h
        self.sx = sx
        self.sy = sy
        self.sw = sw if sw > 0 else w
        self.sh = sh if sh > 0 else h
        self.alpha = 1.0
        self.visible = True
        self.z_order = z_order
        self.flip_x = False
        self.flip_y = False

    def set_frame(self, frame):
        if isinstance(frame, (tuple, list)) and len(frame) == 4:
            self.sx, self.sy, self.sw, self.sh = frame

    def draw(self, camera=None):
        if not self.enabled or not self.visible:
            return
        t = self.entity.transform
        x, y = t.world_x, t.world_y
        if camera:
            if not camera.is_visible(x, y, self.w, self.h):
                return
            x, y = camera.to_screen(x, y)
        _draw_texture(
            self.texture_id, x, y, self.w, self.h,
            self.sx, self.sy, self.sw, self.sh,
            self.alpha, t.rotation, 0.5, 0.5, self.flip_x, self.flip_y
        )


Sprite = SpriteRenderer


class TextRenderer(Component):
    def __init__(self, font_id: int, text: str, size: int = 20,
                 r: int = 255, g: int = 255, b: int = 255, z_order: int = 0):
        super().__init__()
        self.font_id = font_id
        self.text = text
        self.size = size
        self.r = r
        self.g = g
        self.b = b
        self.z_order = z_order

    def draw(self, camera=None):
        if not self.enabled:
            return
        kg = _kagra()
        t = self.entity.transform
        x, y = t.world_x, t.world_y
        if camera:
            x, y = camera.to_screen(x, y)
        kg.draw_text(self.font_id, self.text, x, y, self.size, self.r, self.g, self.b)


class RigRenderer(Component):
    def __init__(self, rig_id: int, z_order: int = 0):
        super().__init__()
        self.rig_id = rig_id
        self.z_order = z_order

    def draw(self, camera=None):
        if not self.enabled:
            return
        kg = _kagra()
        t = self.entity.transform
        x, y = t.world_x, t.world_y
        if camera:
            x, y = camera.to_screen(x, y)
        kg.draw_rig(self.rig_id, x, y)


class RectRenderer(Component):
    def __init__(self, w: float, h: float, r: int = 255, g: int = 255, b: int = 255, z_order: int = 0):
        super().__init__()
        self.w = w
        self.h = h
        self.r = r
        self.g = g
        self.b = b
        self.visible = True
        self.z_order = z_order

    def draw(self, camera=None):
        if not self.enabled or not self.visible:
            return
        t = self.entity.transform
        x, y = t.world_x, t.world_y
        if camera:
            if not camera.is_visible(x, y, self.w, self.h):
                return
            x, y = camera.to_screen(x, y)
        _draw_rect(x, y, self.w, self.h, self.r, self.g, self.b)


class AnimatorComponent(Component):
    """
    Phase6: ECS接続用アニメータコンポーネント
    target_renderer を指定しなければ SpriteRenderer を自動探索する。
    """
    def __init__(self, animator=None, target_renderer=None):
        super().__init__()
        self.animator = animator
        self.target_renderer = target_renderer

    def start(self):
        if self.target_renderer is None:
            self.target_renderer = (
                self.entity.get_component(SpriteRenderer) or
                self.entity.get_component(Sprite) or
                self.entity.get_component(RigRenderer)
            )

    def add_clip(self, clip):
        if self.animator:
            self.animator.add_clip(clip)

    def play(self, name: str, reset: bool = True):
        if self.animator:
            self.animator.play(name, reset=reset)

    def stop(self):
        if self.animator:
            self.animator.stop()

    def update(self, dt: float):
        if not self.animator:
            return
        self.animator.update(dt)


class Collider(Component):
    def __init__(self, w: float, h: float, offset_x: float = 0, offset_y: float = 0):
        super().__init__()
        self.w = w
        self.h = h
        self.offset_x = offset_x
        self.offset_y = offset_y

    @property
    def rect(self):
        t = self.entity.transform
        return (t.world_x + self.offset_x, t.world_y + self.offset_y, self.w, self.h)

    def is_colliding(self, other: 'Collider') -> bool:
        x1, y1, w1, h1 = self.rect
        x2, y2, w2, h2 = other.rect
        return not (x1+w1 <= x2 or x2+w2 <= x1 or y1+h1 <= y2 or y2+h2 <= y1)

    def get_collisions(self, tag: str) -> list['Entity']:
        if not self.entity.world or not self.enabled:
            return []
        hits = []
        for o in self.entity.world.find_with_tag(tag):
            if o is self.entity:
                continue
            other_col = o.get_component(Collider)
            if other_col and other_col.enabled and self.is_colliding(other_col):
                hits.append(o)
        return hits


class Entity:
    def __init__(self, name: str = "Entity", tag: str = ""):
        self.name = name
        self.tag = tag
        self.world: 'World' = None
        self.components: list[Component] = []
        self.is_destroyed = False
        self.active = True

        self.transform = Transform()
        self.add_component(self.transform)

    def add_component(self, component: Component) -> Component:
        component.entity = self
        self.components.append(component)
        if self.world:
            self.world._index_add(self, type(component))
            component._start_if_needed()
        return component

    def add(self, component: Component) -> Component:
        return self.add_component(component)

    def remove_component(self, comp_class) -> bool:
        for c in list(self.components):
            if isinstance(c, comp_class):
                self.components.remove(c)
                if self.world:
                    self.world._index_remove(self, comp_class)
                return True
        return False

    def get_component(self, comp_class):
        for c in self.components:
            if isinstance(c, comp_class):
                return c
        return None

    def get(self, comp_class):
        return self.get_component(comp_class)

    def has_component(self, comp_class) -> bool:
        return any(isinstance(c, comp_class) for c in self.components)

    def has(self, comp_class) -> bool:
        return self.has_component(comp_class)

    def set_parent(self, parent: "Entity"):
        self.transform.set_parent(parent.transform)
        return self

    def destroy(self):
        self.is_destroyed = True

    def update(self, dt: float):
        for c in self.components:
            if c.enabled and not isinstance(c, AnimatorComponent):
                c.update(dt)

    def draw(self, camera=None):
        for c in self.components:
            if c.enabled:
                c.draw(camera)


class World:
    def __init__(self):
        self.entities: list[Entity] = []
        self._comp_index: dict[type, set[Entity]] = {}
        self._tag_index: dict[str, set[Entity]] = {}

    def _index_add(self, entity: Entity, comp_type: type):
        self._comp_index.setdefault(comp_type, set()).add(entity)

    def _index_remove(self, entity: Entity, comp_type: type):
        if comp_type in self._comp_index:
            self._comp_index[comp_type].discard(entity)

    def _index_remove_entity(self, entity: Entity):
        for s in self._comp_index.values():
            s.discard(entity)
        if entity.tag and entity.tag in self._tag_index:
            self._tag_index[entity.tag].discard(entity)

    def spawn(self, entity: Entity) -> Entity:
        entity.world = self
        self.entities.append(entity)
        for c in entity.components:
            self._index_add(entity, type(c))
        if entity.tag:
            self._tag_index.setdefault(entity.tag, set()).add(entity)
        for c in entity.components:
            c._start_if_needed()
        return entity

    def create(self, name: str = "Entity", tag: str = "") -> Entity:
        e = Entity(name=name, tag=tag)
        return self.spawn(e)

    def query(self, *comp_types: type) -> list[Entity]:
        if not comp_types:
            return [e for e in self.entities if not e.is_destroyed and e.active]
        sets = [self._comp_index.get(t, set()) for t in comp_types]
        if not sets:
            return []
        smallest = min(sets, key=len)
        result = smallest.copy()
        for s in sets:
            if s is not smallest:
                result &= s
        return [e for e in result if not e.is_destroyed and e.active]

    def find_with_tag(self, tag: str) -> list[Entity]:
        s = self._tag_index.get(tag, set())
        return [e for e in s if not e.is_destroyed and e.active]

    def find_with_name(self, name: str):
        for e in self.entities:
            if e.name == name and not e.is_destroyed:
                return e
        return None

    def update(self, dt: float):
        for e in list(self.entities):
            if e.active and not e.is_destroyed:
                e.update(dt)
        destroyed = [e for e in self.entities if e.is_destroyed]
        for e in destroyed:
            self._index_remove_entity(e)
        self.entities = [e for e in self.entities if not e.is_destroyed]

    def draw(self, camera=None):
        renderable = [e for e in self.entities if e.active and not e.is_destroyed]

        def get_z(e):
            for cls in (SpriteRenderer, TextRenderer, RigRenderer, RectRenderer):
                c = e.get_component(cls)
                if c:
                    return getattr(c, "z_order", 0)
            return 0

        renderable.sort(key=get_z)
        for e in renderable:
            e.draw(camera)


class EntityScene:
    """
    World + scheduler + timeline players を持つ基本シーン。
    デフォルトで AnimationSystem / TimelineSystem が有効。
    """
    def __init__(self):
        self.world = World()
        self.camera = None
        self._timeline_players = []

        # デフォルトスケジューラ（AnimationSystem を含む）
        # ユーザーは scene.scheduler.add(MySystem()) で拡張できる
        from kagra.systems import SystemScheduler, AnimationSystem, TimelineSystem
        self.scheduler = SystemScheduler()
        self.scheduler.add(AnimationSystem())
        self.scheduler.add(TimelineSystem())

    def on_enter(self):  pass
    def on_exit(self):   pass
    def on_pause(self):  pass
    def on_resume(self): pass

    def add_timeline(self, player):
        self._timeline_players.append(player)
        return player

    def update(self, dt: float):
        if self.scheduler:
            self.scheduler.update(dt, world=self.world, scene=self)
        self.world.update(dt)

    def draw(self):
        self.world.draw(self.camera)
