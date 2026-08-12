def test_priority_order(event_bus_mod):
    bus = event_bus_mod.EventBus()
    order = []
    bus.on("e", lambda d: order.append("lo"), priority=0)
    bus.on("e", lambda d: order.append("hi"), priority=10)
    bus.emit("e")
    assert order == ["hi", "lo"]


def test_once_auto_unregister(event_bus_mod):
    bus = event_bus_mod.EventBus()
    hits = []
    bus.once("e", lambda d: hits.append(1))
    bus.emit("e")
    bus.emit("e")
    assert hits == [1]
    assert bus.listener_count("e") == 0


def test_deferred_needs_flush(event_bus_mod):
    bus = event_bus_mod.EventBus()
    hits = []
    bus.on("e", lambda d: hits.append(d["v"]))
    bus.emit("e", {"v": 1}, deferred=True)
    assert hits == []
    bus.flush()
    assert hits == [1]


def test_emit_does_not_mutate_source_dict(event_bus_mod):
    bus = event_bus_mod.EventBus()
    src = {"x": 1}
    seen = []
    bus.on("e", lambda d: seen.append(d))
    bus.emit("e", src)
    assert "_event" not in src
    assert seen[0]["_event"] == "e"
    assert seen[0]["x"] == 1
