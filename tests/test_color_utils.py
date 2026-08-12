def test_clamp_u8(color_utils):
    assert color_utils.clamp_u8(-1) == 0
    assert color_utils.clamp_u8(300) == 255
    assert color_utils.clamp_u8(12.9) == 12


def test_norm_color(color_utils):
    assert color_utils.norm_color((300, -1, 10)) == (255, 0, 10, 255)
    assert color_utils.norm_color((1, 2, 3, 64)) == (1, 2, 3, 64)


def test_resolve_rgb(color_utils):
    assert color_utils.resolve_rgb(128) == (128, 128, 128, 255)
    assert color_utils.resolve_rgb((1, 2, 3), a=64) == (1, 2, 3, 64)
    assert color_utils.resolve_rgb(10, 20, 30, 40) == (10, 20, 30, 40)
