# KAGRA Game Engine

KAGRA is a hybrid game engine combining Python (easy scripting) and Rust (high-performance rendering, VRM, FBX).
It comes with sample games and experimental FBX/BVH retargeting.

<img width="1919" height="1029" alt="image" src="https://github.com/user-attachments/assets/e8c94080-0465-498e-aca9-d80e71165308" />
<img width="1276" height="744" alt="image" src="https://github.com/user-attachments/assets/4d9f3564-b926-492a-abb8-5000581cc1ed" />

## Features

- **VRM avatars** – load, animate, SpringBone, MToon, look-at, expressions
- **3D tilemap engine** – walls, floors, items, dynamic visibility
- **2D action / platformer** – ECS, physics, UI, event bus, shop system
- **Top-down physics** – Rigidbody, BoxCollider, tile-based collision
- **Camera** – 2D follow, 3D orbit, zoom, shake
- **FBX / BVH retargeting** – experimental (Mixamo-ready)
- **Rust core** – GPU skinning, wgpu rendering, fog / shadows
- **Agent-friendly tooling** – API index, binding consistency tests, golden renders

## Requirements

- Python 3.10 or later
- Rust (latest stable) + Cargo
- maturin – `pip install maturin`

## Quick Start (Windows CMD)

```cmd
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv
.venv\Scripts\activate.bat
pip install maturin
maturin develop
```

This builds the Rust core and installs the `kagra` package into the virtual environment.

## Sample Game 1: 3D Maze Explorer

A fully playable 3D maze game with tilemap, collectable items, and a VRM avatar.

```cmd
python examples/3Dmaze.py
```

### Controls

- ↑ / ↓ – move forward/backward
- ← / → – rotate (camera follows)
- SPACE (hold) – overhead camera view
- ESC – exit

Place any VRM file at `assets/model/player.vrm` to see your own character.

## Sample Game 2: Defend the Crystal (2D Action)

A tower-defense style platformer – protect the crystal from waves of enemies, upgrade your stats, and survive as long as possible.

```cmd
python examples/2Daction.py
```

### Controls

- ← / → – move
- Z or ↑ – jump
- X – attack (melee + magic missile if MP ≥ 10)
- ESC – open shop / pause (spend coins to heal or upgrade)

### Features demonstrated

- ECS (`World`, `Script`, `Rigidbody`, `BoxCollider`)
- Tilemap with solid tiles
- Event bus (`kagra.on`, `kagra.emit`)
- UI (`ProgressBar`, `VBox`, `Button`, `Label`)
- Camera shake, damage numbers, particle effects
- Shop system with in-game currency

No external assets required – textures are generated procedurally.

## Reference: VRM Orb Rush

```cmd
python examples/vrm_orb_rush.py
```

Collect stars, dodge bombs, with sound / particles / difficulty curve. Needs `assets/Emma.vrm`.

## Minimal Code Example

```python
import kagra
from kagra.camera import Camera

class MyScene(kagra.Scene):
    def on_enter(self):
        self.cam = Camera(1280, 720)
        self.avatar = kagra.avatar("assets/model/player.vrm")
        self.avatar.play("idle")

    def update(self, dt):
        self.avatar.update(dt)
        self.cam.update(dt)

    def draw(self):
        kagra.cls(30, 40, 60)
        kagra.draw_vrm(self.avatar.vrm_id)

kagra.init()
kagra.run(start_scene=MyScene())
```

## Project Structure

```text
KAGRA/
├── kagra/               # Python API layer
├── kagra-core/          # Rust core (rendering, VRM, FBX)
├── examples/            # Sample games
├── docs/API_INDEX.md    # Auto-generated public API index
├── tests/               # Unit + golden image tests
├── tools/               # Dev utilities
├── assets/              # Models, textures, fonts
├── pyproject.toml
└── README.md
```

## Experimental: FBX / BVH Retargeting

Mixamo FBX files often have a -90° X-axis pre-rotation.

The engine corrects this using a world-space delta method.

For best results, provide a T-pose FBX as bind pose (e.g. `assets/T-Pose.fbx`).

Retargeting is still experimental – some animations may show side-to-side sway.

## Contributing

Issues and pull requests are welcome.
Please follow existing style: Python type hints, Rust `cargo fmt`.

Public API surface: see [`docs/API_INDEX.md`](docs/API_INDEX.md) (`python tools/gen_api_index.py`).

## License

MIT – see [LICENSE](LICENSE).

KAGRA – named after the Kamioka Gravitational Wave Detector.
Solid, precise, and built for fun.
