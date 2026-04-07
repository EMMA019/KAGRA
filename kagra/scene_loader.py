
import json
from kagra.entity import Entity

def load_entity(data, component_registry):
    e = Entity(data["name"])

    t = data["transform"]
    e.transform.x = t["x"]
    e.transform.y = t["y"]
    e.transform.rotation = t["rotation"]
    e.transform.scale_x = t["scale_x"]
    e.transform.scale_y = t["scale_y"]

    for cdata in data["components"]:
        cname = cdata.pop("type")
        if cname in component_registry:
            comp = component_registry[cname](**cdata)
            e.add(comp)

    for ch in data["children"]:
        child = load_entity(ch, component_registry)
        child.transform.set_parent(e.transform)

    return e

def load_scene(scenegraph, path, component_registry):
    with open(path, encoding="utf-8") as f:
        data = json.load(f)

    scenegraph.roots.clear()

    for edata in data:
        e = load_entity(edata, component_registry)
        scenegraph.add(e)
