from __future__ import annotations
import subprocess
import time


class SceneRuntime:
    def __init__(
        self,
        runtime_executable: str = "kagra_runtime",
        runtime_args_prefix: list[str] | None = None,
        working_dir: str | None = None,
    ):
        self.runtime_executable = runtime_executable
        self.runtime_args_prefix = runtime_args_prefix or []
        self.working_dir = working_dir
        self.process: subprocess.Popen | None = None
        self.current_scene: str | None = None

    @property
    def is_running(self) -> bool:
        return self.process is not None and self.process.poll() is None

    def build_command(self, scene_path: str) -> list[str]:
        cmd = [self.runtime_executable]
        cmd.extend(self.runtime_args_prefix)
        if scene_path:
            cmd.append(scene_path)
        return cmd

    def start(self, scene_path: str):
        if not scene_path:
            raise ValueError("scene_path is required")

        self.stop()
        self.current_scene = scene_path

        cmd = self.build_command(scene_path)
        cwd = self.working_dir or "."

        self.process = subprocess.Popen(
            cmd,
            cwd=cwd,
        )

    def stop(self, wait_timeout: float = 2.0):
        if self.process is None:
            return

        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=wait_timeout)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=wait_timeout)

        self.process = None

    def restart(self, scene_path: str | None = None):
        target = scene_path or self.current_scene
        if not target:
            raise ValueError("No scene selected to restart")
        self.stop()
        time.sleep(0.15)
        self.start(target)

    def status_text(self) -> str:
        if self.is_running:
            return f"Running: {self.current_scene}"
        if self.current_scene:
            return f"Stopped: {self.current_scene}"
        return "Runtime idle"
