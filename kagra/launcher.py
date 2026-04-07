# launcher.py
# KAGRA エディタ起動スクリプト

from kagra.editor_app import KagraEditorApp

if __name__ == "__main__":
    app = KagraEditorApp(
        runtime_executable="python",
        runtime_args_prefix=[],   # 例: ["examples/phase6_demo.py"] など
    )
    app.mainloop()
