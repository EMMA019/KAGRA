from kagra.anim_mixer import AnimMixer


def test_crossfade_ramps_and_finishes():
    m = AnimMixer()
    m.play("Walk")
    m.update(0.5)
    m.cross_fade_to("Run", 0.4)
    assert m.previous_weight() == 1.0
    m.update(0.1)
    assert abs(m.previous_weight() - 0.75) < 1e-6
    f = m.fields()
    assert f["clip_name"] == "Run" and f["fade_from"] == "Walk"
    assert abs(f["fade_from_clip"] - 0.6) < 1e-6
    m.update(0.3)
    assert m.previous is None and m.current_weight() == 1.0
    assert m.fields()["fade_from"] is None


def test_play_hard_cut_and_noop():
    m = AnimMixer()
    m.cross_fade_to("Idle", 0.5)
    assert m.previous is None
    m.update(1.0)
    m.cross_fade_to("Idle", 0.5)
    assert abs(m.time - 1.0) < 1e-9
    m.play("Jump")
    assert m.time == 0.0 and m.previous_weight() == 0.0
