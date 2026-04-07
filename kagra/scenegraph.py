
class SceneGraph:
    def __init__(self):
        self.roots = []

    def add(self, entity):
        if entity.transform.parent is None:
            self.roots.append(entity)

    def traverse(self):
        for root in self.roots:
            yield from self._walk(root)

    def _walk(self, entity):
        yield entity
        for child in entity.transform.children:
            if hasattr(child, "entity"):
                yield from self._walk(child.entity)
