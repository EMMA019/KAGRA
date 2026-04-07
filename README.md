# KAGRA Game Engine

**KAGRA** is a hybrid game engine combining **Python** (easy scripting) and **Rust** (high-performance rendering, VRM, FBX). It features a built-in 3D tilemap maze, VRM avatar support, and experimental FBX/BVH retargeting.

![3D Maze Screenshot](https://via.placeholder.com/800x400?text=3D+Maze+Explorer)  
*Replace with actual screenshot later*

## ✨ Features

- **VRM avatar** loading, animation, and SpringBone physics
- **3D tilemap engine** with walls, floors, items, and goal
- **Top‑down physics** (Rigidbody, BoxCollider) for tile‑based movement
- **Camera3D** with orbit, zoom, and follow modes
- **FBX / BVH** animation retargeting (experimental)
- **Rust‑powered** GPU skinning and rendering (via `wgpu`)
- **Minimal dependencies** – just Python and Rust toolchain

## 📦 Requirements

- Python 3.10 or later
- Rust (latest stable) + Cargo
- `maturin` (Python‑Rust binding tool)

Install `maturin`:
```bash
pip install maturin
🚀 Installation
Clone the repository and build the Rust core:

bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
maturin develop
This compiles the Rust extension (kagra_core) and installs the Python package in development mode.

🎮 Run the 3D Maze Sample
bash
python examples/3Dmaze.py
Controls:

↑ ↓ – move forward / backward

← → – rotate (camera follows)

SPACE (hold) – overhead camera view

ESC – exit

Make sure you have a VRM model at assets/model/player.vrm (you can place any VRM there).

🧪 Basic API Example
python
import kagra
from kagra.camera3d import Camera3D

class MyScene(kagra.Scene):
    def on_enter(self):
        self.cam = Camera3D(1280, 720)
        self.cam.use_orbit(radius=3.0, target=(0,0.9,0))
        self.avatar = kagra.avatar("assets/model/player.vrm")
        self.avatar.play("idle")

    def update(self, dt):
        self.avatar.update(dt)
        self.cam.update(kagra._engine)

    def draw(self):
        kagra.cls(30,40,60)
        kagra.draw_vrm(self.avatar.vrm_id)

kagra.init()
kagra.run(start_scene=MyScene())
📂 Project Structure
text
KAGRA/
├── kagra/               # Python API layer
├── kagra-core/          # Rust core (rendering, VRM, FBX)
├── examples/            # Sample games (3Dmaze.py, etc.)
├── assets/              # Models, textures, fonts
├── pyproject.toml       # Python build config
├── Cargo.toml           # Rust build config
└── README.md
⚠️ Notes on FBX / BVH Retargeting
Mixamo FBX files often have a -90° X‑axis pre‑rotation. The engine attempts to correct this using a world‑space delta method.

For best results, provide a T‑pose FBX as bind pose (e.g. assets/T-Pose.fbx).

Retargeting is experimental – walking animations may still exhibit side‑to‑side sway or floating.

🤝 Contributing
Issues and pull requests are welcome. Please follow the existing code style (Python with type hints, Rust with cargo fmt).

📄 License
MIT (or your chosen license – add a LICENSE file)

KAGRA – named after the Kamioka Gravitational Wave Detector. Because game engines should be solid and precise. 🚀

text

You can replace the placeholder screenshot link later. Also consider adding a `LICENSE` file (e.g., MIT). After saving, stage and commit it:
