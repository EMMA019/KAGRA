# kagra/__init__.py  — shared wgpu 30 mainline
"""KAGRA public API (Python game master + WorldDoc / WorldPlay).

The archived RendererV2 / ``kagra_core`` demo is under ``old/``.
``import kagra`` must succeed without that extension.
"""
from __future__ import annotations

from kagra.annotate import annotate
from kagra.audio import (
    mixer_state,
    play_bgm,
    play_se,
    play_sfx,
    play_wav,
    se,
    set_group_volume,
    set_listener,
    set_mute,
    sound,
    stop_bgm,
    tone,
)
from kagra.brain import Brain, BrainError, KairiBrain, OpenAIBrain, brain
from kagra.contracts import AssetKind, KagraContractError, resolve_asset
from kagra.gameloop import (
    Scene,
    draw_world,
    mouse_clicked,
    mouse_down,
    mouse_pos,
    pressed,
    render_rgba,
    rgba_to_png,
    rgba_to_ppm,
    run,
    was_pressed,
)
from kagra.i18n import add_table, get_lang, set_lang, t
from kagra.juice import Juice, apply_look, apply_post, ease
from kagra.particles import Emitter, Trail
from kagra.land import island_height, open_world_height, overworld_height
from kagra.path import find_path, move_range
from kagra.flow import save_dir
from kagra.save import SlotStore, load_data, save_data
from kagra.trace import debug_trace, debug_trace_summary
from kagra.ui2d import bar, choice_menu, image, list_lines, merge, message, paged_menu, panel, scroll_window
from kagra.verify import load_scenario, run_scenario, run_scenario_path
from kagra.world_expect import eval_world_expect

try:
    from kagra.kagra_shared import (
        ActionSession,
        CardSession,
        FightSession,
        FishCookSession,
        FpsSession,
        NeonRun,
        NovelSession,
        PlatformerSession,
        PuzzlePadSession,
        PuzzleSession,
        RaceSession,
        RhythmSession,
        RpgSession,
        ShopSession,
        SportsSession,
        StealthSession,
        SurvivalSession,
        TdSession,
        WorldDoc,
        WorldPlay,
        render_world_doc,
    )
except ImportError:  # pragma: no cover — `cd kagra-shared && maturin develop`
    try:
        from kagra_shared import (  # type: ignore[no-redef]
            ActionSession,
            CardSession,
            FightSession,
            FishCookSession,
            FpsSession,
            NeonRun,
            NovelSession,
            PlatformerSession,
            PuzzlePadSession,
            PuzzleSession,
            RaceSession,
            RhythmSession,
            RpgSession,
            ShopSession,
            SportsSession,
            StealthSession,
            SurvivalSession,
            TdSession,
            WorldDoc,
            WorldPlay,
            render_world_doc,
        )
    except ImportError:  # pragma: no cover
        WorldDoc = None  # type: ignore[assignment]
        WorldPlay = None  # type: ignore[assignment]
        ActionSession = None  # type: ignore[assignment]
        FightSession = None  # type: ignore[assignment]
        PuzzlePadSession = None  # type: ignore[assignment]
        PuzzleSession = None  # type: ignore[assignment]
        NeonRun = None  # type: ignore[assignment]
        NovelSession = None  # type: ignore[assignment]
        PlatformerSession = None  # type: ignore[assignment]
        RaceSession = None  # type: ignore[assignment]
        CardSession = None  # type: ignore[assignment]
        FpsSession = None  # type: ignore[assignment]
        RhythmSession = None  # type: ignore[assignment]
        RpgSession = None  # type: ignore[assignment]
        ShopSession = None  # type: ignore[assignment]
        SportsSession = None  # type: ignore[assignment]
        StealthSession = None  # type: ignore[assignment]
        SurvivalSession = None  # type: ignore[assignment]
        TdSession = None  # type: ignore[assignment]
        FishCookSession = None  # type: ignore[assignment]

        def render_world_doc(*_a, **_k):  # type: ignore[misc]
            raise ImportError(
                "kagra_shared not installed: `maturin develop --release` "
                "(root pyproject) or `cd kagra-shared && maturin develop --release`"
            )


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
    "ActionSession",
    "AssetKind",
    "Brain",
    "BrainError",
    "Emitter",
    "FishCookSession",
    "Juice",
    "NeonRun",
    "Trail",
    "KagraContractError",
    "CardSession",
    "KairiBrain",
    "FpsSession",
    "NovelSession",
    "OpenAIBrain",
    "PlatformerSession",
    "PuzzlePadSession",
    "PuzzleSession",
    "RaceSession",
    "RhythmSession",
    "RpgSession",
    "Scene",
    "ShopSession",
    "SportsSession",
    "SlotStore",
    "StealthSession",
    "SurvivalSession",
    "TdSession",
    "WorldDoc",
    "WorldPlay",
    "add_table",
    "annotate",
    "apply_look",
    "apply_post",
    "bar",
    "brain",
    "choice_menu",
    "debug_trace",
    "debug_trace_summary",
    "draw_world",
    "ease",
    "eval_world_expect",
    "FightSession",
    "find_path",
    "get_lang",
    "image",
    "island_height",
    "list_lines",
    "load_data",
    "load_scenario",
    "merge",
    "message",
    "mixer_state",
    "mouse_clicked",
    "mouse_down",
    "mouse_pos",
    "move_range",
    "open_world_height",
    "overworld_height",
    "paged_menu",
    "panel",
    "play_bgm",
    "play_se",
    "play_sfx",
    "play_wav",
    "pressed",
    "render_rgba",
    "render_world_doc",
    "resolve_asset",
    "rgba_to_png",
    "rgba_to_ppm",
    "run",
    "run_scenario",
    "run_scenario_path",
    "save_data",
    "save_dir",
    "scroll_window",
    "se",
    "set_group_volume",
    "set_lang",
    "set_listener",
    "set_mute",
    "sound",
    "stop_bgm",
    "t",
    "tone",
    "was_pressed",
]
