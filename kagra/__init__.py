# kagra/__init__.py  — shared wgpu 30 mainline
"""KAGRA public API (Python game master + WorldDoc / WorldPlay).

The archived RendererV2 / ``kagra_core`` demo is under ``old/``.
``import kagra`` must succeed without that extension.
"""
from __future__ import annotations

from kagra.annotate import annotate
from kagra.audio import play_se, play_wav, se, set_listener, sound, tone
from kagra.brain import Brain, BrainError, KairiBrain, OpenAIBrain, brain
from kagra.contracts import AssetKind, KagraContractError, resolve_asset
from kagra.gameloop import (
    Scene,
    draw_world,
    mouse_clicked,
    mouse_down,
    mouse_pos,
    pressed,
    rgba_to_png,
    run,
    was_pressed,
)
from kagra.i18n import add_table, get_lang, set_lang, t
from kagra.land import island_height, open_world_height, overworld_height
from kagra.path import find_path, move_range
from kagra.save import SlotStore, load_data, save_data
from kagra.trace import debug_trace, debug_trace_summary
from kagra.ui2d import bar, choice_menu, image, list_lines, merge, message, paged_menu, panel, scroll_window
from kagra.verify import load_scenario, run_scenario, run_scenario_path
from kagra.world_expect import eval_world_expect

try:
    from kagra.kagra_shared import WorldDoc, WorldPlay, render_world_doc
except ImportError:  # pragma: no cover — wheel / `maturin develop`
    try:
        from kagra_shared import WorldDoc, WorldPlay, render_world_doc
    except ImportError:
        # python-unit CI is extension-free. Dump JSON still works.
        from kagra.world_fallback import WorldDoc, WorldPlay, render_world_doc


# Names that stay on disk (kagra.play / kagra.world3d / …) but must not
# appear on `import kagra`. The archived RendererV2 surface lives in old/.
_PUBLIC_OFF = {
    "Entity",
    "EntityScene",
    "TileMap",
    "TileSet",
    "KagraEditorApp",
    "Walk",
    "Prop",
    "World3D",
    "World",
}

__all__ = [
    "AssetKind",
    "Brain",
    "BrainError",
    "KagraContractError",
    "KairiBrain",
    "OpenAIBrain",
    "Scene",
    "SlotStore",
    "WorldDoc",
    "WorldPlay",
    "add_table",
    "annotate",
    "bar",
    "brain",
    "choice_menu",
    "debug_trace",
    "debug_trace_summary",
    "draw_world",
    "eval_world_expect",
    "find_path",
    "get_lang",
    "image",
    "island_height",
    "list_lines",
    "load_data",
    "load_scenario",
    "merge",
    "message",
    "mouse_clicked",
    "mouse_down",
    "mouse_pos",
    "move_range",
    "open_world_height",
    "overworld_height",
    "paged_menu",
    "panel",
    "play_se",
    "play_wav",
    "pressed",
    "render_world_doc",
    "resolve_asset",
    "rgba_to_png",
    "run",
    "run_scenario",
    "run_scenario_path",
    "save_data",
    "scroll_window",
    "se",
    "set_lang",
    "set_listener",
    "sound",
    "t",
    "tone",
    "was_pressed",
]
