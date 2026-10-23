"""GPU point and fat-line dump helpers stay pure Python."""

from kagra.strokes import fat_line, point_cloud


def test_point_cloud_and_fat_line_are_dump_records():
    dots = point_cloud([(0, 1, 2), (3, 4, 5)], color=(10, 20, 30), size=0.2, id="dots")
    assert dots["id"] == "dots"
    assert dots["positions"] == [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]
    assert dots["color"] == [10, 20, 30, 255]
    assert dots["size"] == 0.2
    stroke = fat_line([(0, 0, 0), (1, 0, 0)], color=(1, 2, 3, 4), width=6, id="bar")
    assert stroke["points"][1] == [1.0, 0.0, 0.0]
    assert stroke["width"] == 6.0
    assert stroke["color"] == [1, 2, 3, 4]
