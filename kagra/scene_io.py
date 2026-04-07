
import json

def serialize_transform(t):
    return {
        "x": t.x,
        "y": t.y,
        "rotation": t.rotation,
        "scale_x": t.scale_x,
        "scale_y": t.scale_y,
    }

def serialize_component(c):
    data = {"type": c.__class__.__name__}
    if hasattr(c, "__dict__"):
        for k, v in c.__dict__.items():
            if k == "entity":
                continue
            if isinstance(v, (int, float, str, bool, list, dict)):
                data[k] = v
    return data

def serialize_entity(e):
    return {
        "name": e.name,
        "transform": serialize_transform(e.transform),
        "components": [serialize_component(c) for c in e.components],
        "children": [
            serialize_entity(child.entity)
            for child in e.transform.children
            if hasattr(child, "entity")
        ],
    }

def save_scene(scenegraph, path):
    data = []
    for root in scenegraph.roots:
        data.append(serialize_entity(root))
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
