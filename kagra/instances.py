# kagra/instances.py
"""
GPU インスタンシング API
100万スプライトを Python から1行で描画できる。

Example::
    # セットアップ
    batch = kagra.InstanceBatch(capacity=1_000_000, sprite_w=8, sprite_h=8)

    # 毎フレーム更新
    data = [[x, y, 1.0, 1.0, 0.0, 1.0] for x, y in positions]
    batch.update(data)

    # 描画
    batch.draw()
"""
from __future__ import annotations
from typing import Optional


class InstanceBatch:
    """GPU インスタンシングバッチ。

    1 draw call で最大 capacity 個のスプライトを描画する。
    Pure Python では不可能な 100万スプライトをリアルタイムで動かせる。

    data フォーマット: [[x, y, scale_x, scale_y, rotation_rad, alpha], ...]

    Example::
        batch = kagra.InstanceBatch(capacity=1_000_000, sprite_w=8, sprite_h=8)

        # numpy でも list でも OK
        import numpy as np
        positions = np.random.rand(100000, 6).astype(np.float32)
        positions[:, 0] *= 1280  # x
        positions[:, 1] *= 720   # y
        positions[:, 2] = 8.0    # scale_x
        positions[:, 3] = 8.0    # scale_y
        positions[:, 4] = 0.0    # rotation
        positions[:, 5] = 1.0    # alpha

        batch.update(positions)
        batch.draw()
    """

    def __init__(
        self,
        capacity:  int   = 100_000,
        texture_id: int  = 0,
        sprite_w:  float = 16.0,
        sprite_h:  float = 16.0,
    ):
        """
        Args:
            capacity:   最大スプライト数（事前確保）
            texture_id: テクスチャ ID（0 = デフォルト白丸）
            sprite_w:   スプライト幅（ピクセル）
            sprite_h:   スプライト高さ（ピクセル）
        """
        import kagra
        self._batch_id = kagra.get_engine().create_instance_batch(
            texture_id, capacity, float(sprite_w), float(sprite_h)
        )
        self._count = 0

    def update(self, data):
        """インスタンスデータを更新する。

        Args:
            data: [[x, y, scale_x, scale_y, rotation_rad, alpha], ...] の
                  list, tuple, または numpy array（float32）

        Example::
            # list
            batch.update([[100, 200, 1.0, 1.0, 0.0, 1.0]])

            # numpy（高速）
            import numpy as np
            arr = np.zeros((1000000, 6), dtype=np.float32)
            arr[:, 0] = x_positions
            arr[:, 1] = y_positions
            arr[:, 2] = 8.0   # scale_x
            arr[:, 3] = 8.0   # scale_y
            arr[:, 5] = 1.0   # alpha
            batch.update(arr)
        """
        import kagra
        # numpy array → list of [f32; 6] に変換
        if hasattr(data, 'tolist'):
            flat = data.tolist()
        else:
            flat = [list(row) for row in data]
        self._count = len(flat)
        kagra.get_engine().update_instance_batch(self._batch_id, flat)

    def draw(self):
        """描画キューに積む（毎フレーム draw() の中で呼ぶ）。"""
        import kagra
        kagra.get_engine().draw_instance_batch(self._batch_id)

    @property
    def count(self) -> int:
        """現在描画中のスプライト数。"""
        return self._count

    @property
    def batch_id(self) -> int:
        return self._batch_id
