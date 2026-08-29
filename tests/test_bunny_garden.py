"""バニーガーデン（kagra.bunny_garden）の純ロジックテスト。

kagra_core / kagra_shared に依存しない（UI 描画は呼ばない）。保存は
tmp_path を使い、ユーザーの ~/.kagra に触らない。
"""
import json

from tests.conftest import load_kagra_submodule

bg = load_kagra_submodule("bunny_garden")


def _game(tmp_path, start_day=1):
    return bg.BunnyGarden(save_path=tmp_path / "save.json", start_day=start_day)


def test_new_game_defaults(tmp_path):
    g = _game(tmp_path)
    assert g.game["day"] == 1
    assert g.game["money"] == 300
    assert g.game["stock"]["モヒート"] == 3
    assert g.game["affection"][bg.CHAR] == 0
    assert g.state == "msg"
    assert g.message  # 挨拶が表示される


def test_talk_raises_affection(tmp_path):
    g = _game(tmp_path)
    g._drain()
    g._do_choice(0)
    assert g.game["affection"][bg.CHAR] == 3


def test_praise_raises_2_to_5(tmp_path):
    g = _game(tmp_path)
    g._drain()
    g._do_choice(2)
    assert 2 <= g.game["affection"][bg.CHAR] <= 5


def test_rng_is_deterministic_per_day(tmp_path):
    a = _game(tmp_path)
    b = _game(tmp_path)
    assert a._rnd(1000) == b._rnd(1000), "同じ日 → 同じ乱数列（再現可能）"
    a._next_day()
    b._next_day()
    assert a._rnd(1000) == b._rnd(1000), "日が変わっても同じ seed なら一致"


def test_drink_consumes_stock_and_money(tmp_path):
    g = _game(tmp_path)
    g._drain()
    money0, stock0 = g.game["money"], g.game["stock"]["モヒート"]
    g._do_choice(1)          # 飲み物メニューへ
    g._do_drink(0)           # モヒート
    assert g.game["stock"]["モヒート"] == stock0 - 1
    assert g.game["money"] == money0 - 80
    assert g.game["affection"][bg.CHAR] == 8


def test_drink_hides_empty_stock(tmp_path):
    g = _game(tmp_path)
    g._drain()
    g._do_choice(1)
    g.game["stock"]["モヒート"] = 0
    g.game["stock"]["オレンジジュース"] = 0
    items = g._drink_items()
    assert items == ["やめる"], "在庫ゼロなら「やめる」だけ"


def test_close_day_adds_income_and_saves(tmp_path):
    g = _game(tmp_path)
    g._drain()
    g._set_aff(40)
    g._do_choice(3)          # 閉店
    assert g.state == "end"
    assert g.game["money"] >= 300 + 100
    assert (tmp_path / "save.json").exists(), "閉店でセーブされる"


def test_save_load_roundtrip(tmp_path):
    g = _game(tmp_path)
    g._drain()
    g._do_choice(0)
    g._set_aff(23)
    g._save()
    h = _game(tmp_path)      # 同じ save パス → ロード
    assert h.game == g.game, "セーブ/ロードで状態が一致"


def test_special_event_at_50(tmp_path):
    g = _game(tmp_path)
    g._set_aff(49)
    g._drain()
    g._do_choice(0)          # +3 → 52
    assert "special" in g.game["events"]
    assert len(g.queue) >= 1, "特別イベントの台詞が積まれる"


def test_affection_clamped(tmp_path):
    g = _game(tmp_path)
    g._set_aff(99)
    g._drain()
    g._do_choice(0)          # +3 → 100 上限
    assert g.game["affection"][bg.CHAR] == 100


def test_headless_policy_three_days(tmp_path):
    g = _game(tmp_path)
    for _ in range(3):
        g._drain()
        g._do_choice(0)      # 話す
        g._drain()
        g._do_choice(2)      # ほめる
        g._drain()
        g._do_choice(1)
        g._do_drink(0)       # モヒート
        g._drain()
        g._do_choice(3)      # 閉店
        assert g.state == "end"
        g._next_day()
    assert g.game["day"] == 4
    assert g.game["money"] > 300
    assert g.game["affection"][bg.CHAR] > 30
    assert g.game["stock"]["モヒート"] == 0, "3 日で 3 杯飲んだ"
