def test_transform_world_coords(entity_mod):
    parent = entity_mod.Transform(10, 20)
    child = entity_mod.Transform(3, 4)
    child.set_parent(parent)
    assert child.world_x == 13
    assert child.world_y == 24
    child.set_parent(None)
    assert child.world_x == 3
    assert child not in parent.children


def test_collider_aabb(entity_mod):
    a = entity_mod.Entity("a")
    b = entity_mod.Entity("b")
    ca = a.add(entity_mod.Collider(10, 10))
    cb = b.add(entity_mod.Collider(10, 10))
    b.transform.set_pos(5, 0)
    assert ca.is_colliding(cb)
    b.transform.set_pos(20, 0)
    assert not ca.is_colliding(cb)


def test_world_tag_and_destroy(entity_mod):
    world = entity_mod.World()
    e = entity_mod.Entity("mob", tag="enemy")
    world.spawn(e)
    assert world.find_with_tag("enemy") == [e]
    e.destroy()
    world.update(0)
    assert world.find_with_tag("enemy") == []
