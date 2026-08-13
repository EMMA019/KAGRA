"""pytest プラグイン: Rust 拡張 `kagra.kagra_core` を import 不能にする。

CI の `python-unit` ジョブは maturin を実行しないため拡張が無い。拡張を
インストール済みのローカル環境でその状況を再現し、「拡張が必要なテストが
pure-python ジョブに混入した」事故を検出する。

    python -m pytest tests -m "not golden" -p tests.no_extension_plugin
"""
import sys


class _Blocker:
    def find_spec(self, fullname, path=None, target=None):
        if fullname in ("kagra.kagra_core", "kagra_core"):
            raise ImportError(f"blocked: {fullname} (no-extension simulation)")
        return None


sys.meta_path.insert(0, _Blocker())
