# KAGRA – Hybrid Python/Rust Game Engine

**KAGRA** is a lightweight game engine that combines **Python** (easy scripting) with **Rust** (high‑performance rendering, VRM, FBX).  
It comes with a complete 3D tilemap maze, VRM avatar support, and **experimental FBX/BVH retargeting**.

<img width="1919" height="1029" alt="image" src="https://github.com/user-attachments/assets/c669cddd-9f4c-4d36-9f07-8a965a0a996a" />


## ✨ Features

- **VRM avatars** – load, animate, and simulate SpringBone physics
- **3D tilemap engine** – walls, floors, items, goal, dynamic visibility
- **Top‑down physics** – Rigidbody, BoxCollider, tile‑based collision
- **Camera3D** – orbit, zoom, follow, overhead mode
- **FBX / BVH retargeting** – experimental, Mixamo‑ready (world‑space delta method)
- **Rust core** – GPU skinning, `wgpu` rendering, high performance
- **Minimal setup** – just Python, Rust, and `maturin`

## 📦 Requirements

- **Python** 3.10 or later
- **Rust** (latest stable) + Cargo
- `maturin` – install via `pip install maturin`

## 🚀 Quick Start (Windows CMD)

```cmd
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv
.venv\Scripts\activate.bat
maturin develop
This builds the Rust core and installs the kagra package into the virtual environment.

🎮 Run the 3D Maze Sample
cmd
python examples/3Dmaze.py
Controls

↑ / ↓ – move forward/backward

← / → – rotate (camera follows)

SPACE (hold) – overhead camera view

ESC – exit

Make sure you have a VRM model at assets/player.vrm (you can place any VRM there).

🧪 Minimal Code Example
python
import kagra
from kagra.camera3d import Camera3D

class MyScene(kagra.Scene):
    def on_enter(self):
        self.cam = Camera3D(1280, 720)
        self.cam.use_orbit(radius=3.0, target=(0, 0.9, 0))
        self.avatar = kagra.avatar("assets/model/player.vrm")
        self.avatar.play("idle")

    def update(self, dt):
        self.avatar.update(dt)
        self.cam.update(kagra._engine)

    def draw(self):
        kagra.cls(30, 40, 60)
        kagra.draw_vrm(self.avatar.vrm_id)

kagra.init()
kagra.run(start_scene=MyScene())
📂 Project Structure
text
KAGRA/
├── kagra/               # Python API layer
├── kagra-core/          # Rust core (rendering, VRM, FBX)
├── examples/            # Sample games (3Dmaze.py, ...)
├── assets/              # Models, textures, fonts
├── pyproject.toml       # Python build config
├── Cargo.toml           # Rust build config
└── README.md
⚠️ Experimental: FBX / BVH Retargeting
Mixamo FBX files often have a -90° X‑axis pre‑rotation.

The engine corrects this using a world‑space delta method (no external tools required).

For best results, provide a T‑pose FBX as bind pose (e.g. assets/T-Pose.fbx).

Retargeting is still experimental – some animations may show side‑to‑side sway or floating.

🤝 Contributing
Issues and pull requests are welcome.
Please follow the existing style: Python with type hints, Rust with cargo fmt.

📄 License
MIT – add a LICENSE file if you wish.

KAGRA – named after the Kamioka Gravitational Wave Detector.
Solid, precise, and built for fun.
