def test_box_collider_overlaps(physics_mod, entity_mod):
    a = entity_mod.Entity("a")
    b = entity_mod.Entity("b")
    ca = a.add(physics_mod.BoxCollider(10, 10))
    cb = b.add(physics_mod.BoxCollider(10, 10))
    b.transform.set_pos(5, 0)
    assert ca.overlaps(cb) == (5.0, 10.0)
    b.transform.set_pos(50, 0)
    assert ca.overlaps(cb) is None


def test_rigidbody_integrate_gravity(physics_mod, entity_mod):
    e = entity_mod.Entity("p")
    rb = e.add(physics_mod.Rigidbody(gravity=980.0, drag=0.0))
    y0 = e.transform.y
    rb._integrate(0.1)
    assert rb.vy > 0
    assert e.transform.y > y0


def test_rigidbody_kinematic_and_max_speed(physics_mod, entity_mod):
    e = entity_mod.Entity("p")
    rb = e.add(physics_mod.Rigidbody(gravity=0.0, kinematic=True))
    rb.vx = 100
    rb._integrate(1.0)
    assert e.transform.x == 0.0

    rb.kinematic = False
    rb.vx = 5000
    rb.vy = 0
    rb.max_speed = 100
    rb._integrate(0.0)
    speed = (rb.vx ** 2 + rb.vy ** 2) ** 0.5
    assert speed <= 100.0 + 1e-6


def test_ray_aabb(physics_mod):
    t = physics_mod.PhysicsSystem._ray_aabb(0, 5, 1, 0, 10, 0, 10, 10)
    assert t is not None and abs(t - 10.0) < 1e-5
    assert physics_mod.PhysicsSystem._ray_aabb(0, 50, 1, 0, 10, 0, 10, 10) is None
