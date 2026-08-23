"""ActionController の公開面。GPU 不要。"""
from tests.conftest import load_kagra_submodule

action = load_kagra_submodule("vrm_action")


def test_names_lists_built_ins():
    names = action.ActionController.names()
    for need in ("banzai", "nod", "clap", "bow", "wave", "jump_joy"):
        assert need in names
    assert names == list(dict.fromkeys(names))
