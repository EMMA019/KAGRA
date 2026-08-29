"""ローカライズ（kagra.i18n — Phase 7）の純ロジックテスト。

拡張非依存。グローバル言語状態を汚さないよう、各テストの最後に ja へ戻す。
"""
import pytest

from tests.conftest import load_kagra_submodule

i18n = load_kagra_submodule("i18n")


@pytest.fixture(autouse=True)
def _reset_lang():
    i18n.set_lang("ja")
    yield
    i18n.set_lang("ja")


def test_fallback_to_key():
    assert i18n.t("no.such.key") == "no.such.key", "未登録キーはキーそのもの"


def test_ja_table():
    i18n.add_table("ja", {"greet": "こんにちは"})
    assert i18n.t("greet") == "こんにちは"


def test_lang_switch():
    i18n.add_table("ja", {"ok": "はい"})
    i18n.add_table("en", {"ok": "Yes"})
    assert i18n.t("ok") == "はい"
    i18n.set_lang("en")
    assert i18n.t("ok") == "Yes"
    assert i18n.get_lang() == "en"


def test_missing_lang_falls_back_to_ja():
    i18n.add_table("ja", {"ok": "はい"})
    i18n.set_lang("fr")  # テーブル無し
    assert i18n.t("ok") == "はい", "ja へフォールバック"


def test_format_placeholders():
    i18n.add_table("ja", {"day": "DAY {day}    所持金 {money}G"})
    assert i18n.t("day", day=3, money=120) == "DAY 3    所持金 120G"


def test_format_missing_kwarg_returns_raw():
    i18n.add_table("ja", {"day": "DAY {day}"})
    assert i18n.t("day") == "DAY {day}", "kwargs が無ければ生テキスト"


def test_load_json(tmp_path):
    p = tmp_path / "ja.json"
    p.write_text('{"start": "はじめる"}', encoding="utf-8")
    i18n.load_json("ja", p)
    assert i18n.t("start") == "はじめる"


def test_load_json_non_object_raises(tmp_path):
    p = tmp_path / "bad.json"
    p.write_text('[1, 2]', encoding="utf-8")
    with pytest.raises(ValueError):
        i18n.load_json("ja", p)


def test_available_langs():
    i18n.add_table("ja", {"a": "1"})
    i18n.add_table("en", {"a": "1"})
    i18n.add_table("de", {"a": "1"})
    assert i18n.available_langs() == ["de", "en", "ja"]


def test_bunny_garden_choices_follow_lang(tmp_path):
    bg = load_kagra_submodule("bunny_garden")
    g = bg.BunnyGarden(save_path=tmp_path / "s.json", start_day=1)
    ja_choices = g._choices()
    assert ja_choices[0] == "話す"
    i18n.set_lang("en")
    assert g._choices()[0] == "Talk"
    i18n.set_lang("ja")
    assert g._choices()[0] == "話す"


def test_torneko_menu_close_follows_lang(tmp_path):
    tk = load_kagra_submodule("torneko")
    g = tk.Torneko(seed=1, save_path=tmp_path / "t.json", start_floor=1)
    g.state = "menu"
    i18n.set_lang("ja")
    draw = g.draw  # draw はウィンドウ描画なので呼ばない。テーブルだけ確認
    assert i18n.t("menu.close") == "閉じる"
    i18n.set_lang("en")
    assert i18n.t("menu.close") == "Close"
