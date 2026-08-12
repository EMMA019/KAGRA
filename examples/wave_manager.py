"""
wave_manager.py - ウェーブ管理システム（Python実装）

エンジンに不足しているウェーブ管理機能をPython側で実装します。
TODO: エンジン側にゲームロジックフレームワークとして組み込み
"""

from typing import List, Dict, Any
import random


class WaveManager:
    """敵ウェーブ管理クラス"""
    
    def __init__(self):
        """ウェーブ定義を初期化"""
        self.current_wave = 0
        self.waves = self._create_wave_definitions()
        self.enemies_in_wave = []
        self.spawn_timer = 0.0
        self.enemy_index = 0
        self.wave_active = False
        self.wave_finished = False
        
    def _create_wave_definitions(self) -> List[Dict[str, Any]]:
        """
        ウェーブ定義を作成
        
        敵タイプ:
            - "slime": スライム (HP: 30, 速度: 60, 報酬: 5, ダメージ: 1)
            - "goblin": ゴブリン (HP: 50, 速度: 80, 報酬: 8, ダメージ: 2)
            - "orc": オーク (HP: 100, 速度: 40, 報酬: 15, ダメージ: 3)
        """
        return [
            # ウェーブ 1: 基本スライムのみ
            {
                "enemies": ["slime"] * 5,
                "spawn_delay": 2.0,  # 敵出現間隔（秒）
                "pre_wave_delay": 3.0,  # ウェーブ開始前の待機時間
                "description": "基本ウェーブ"
            },
            # ウェーブ 2: スライムとゴブリンの混合
            {
                "enemies": ["slime"] * 4 + ["goblin"] * 2,
                "spawn_delay": 1.8,
                "pre_wave_delay": 5.0,
                "description": "ゴブリンが登場"
            },
            # ウェーブ 3: より多くの敵
            {
                "enemies": ["slime"] * 3 + ["goblin"] * 4,
                "spawn_delay": 1.5,
                "pre_wave_delay": 5.0,
                "description": "敵数増加"
            },
            # ウェーブ 4: オークが登場
            {
                "enemies": ["slime"] * 2 + ["goblin"] * 3 + ["orc"] * 1,
                "spawn_delay": 1.3,
                "pre_wave_delay": 6.0,
                "description": "オークが登場"
            },
            # ウェーブ 5: ボスウェーブ
            {
                "enemies": ["goblin"] * 3 + ["orc"] * 3,
                "spawn_delay": 1.0,
                "pre_wave_delay": 8.0,
                "description": "最終ウェーブ"
            },
            # 以降はランダム生成（無限モード用）
        ]
    
    def get_enemy_stats(self, enemy_type: str) -> Dict[str, float]:
        """敵タイプごとのステータスを取得"""
        stats = {
            "slime": {
                "hp": 30.0,
                "speed": 60.0,  # ピクセル/秒
                "reward": 5,
                "damage": 1,
                "color": (100, 200, 100)  # 緑
            },
            "goblin": {
                "hp": 50.0,
                "speed": 80.0,
                "reward": 8,
                "damage": 2,
                "color": (200, 100, 100)  # 赤
            },
            "orc": {
                "hp": 100.0,
                "speed": 40.0,
                "reward": 15,
                "damage": 3,
                "color": (150, 100, 50)  # 茶
            }
        }
        return stats.get(enemy_type, stats["slime"]).copy()
    
    def start_wave(self, wave_number: int = None) -> bool:
        """
        ウェーブを開始
        
        Args:
            wave_number: 開始するウェーブ番号（Noneの場合は次のウェーブ）
            
        Returns:
            ウェーブ開始成功かどうか
        """
        if wave_number is not None:
            self.current_wave = wave_number
            
        if self.current_wave >= len(self.waves):
            # 無限モード用にランダムウェーブを生成
            self._generate_random_wave()
        else:
            wave_def = self.waves[self.current_wave]
            self.enemies_in_wave = wave_def["enemies"].copy()
            
        self.enemy_index = 0
        self.spawn_timer = 0.0
        self.wave_active = True
        self.wave_finished = False
        
        # 最初のウェーブ開始前の遅延を設定
        if self.current_wave < len(self.waves):
            self.spawn_timer = -self.waves[self.current_wave]["pre_wave_delay"]
        
        print(f"ウェーブ {self.current_wave + 1} 開始: {self.get_wave_description()}")
        return True
    
    def _generate_random_wave(self):
        """無限モード用のランダムウェーブを生成"""
        wave_num = self.current_wave + 1
        
        # ウェーブ番号に応じて敵数を増加
        base_count = 5 + min(20, wave_num // 2)
        
        enemies = []
        for _ in range(base_count):
            # ウェーブが進むほど強い敵が出現しやすくなる
            rand = random.random()
            if wave_num >= 10 and rand < 0.3:
                enemies.append("orc")
            elif wave_num >= 5 and rand < 0.5:
                enemies.append("goblin")
            else:
                enemies.append("slime")
        
        # シャッフル
        random.shuffle(enemies)
        
        self.enemies_in_wave = enemies
        
    def update(self, dt: float) -> Dict[str, Any]:
        """
        ウェーブの更新
        
        Args:
            dt: 経過時間（秒）
            
        Returns:
            次の敵の情報（出現する場合）または空の辞書
        """
        if not self.wave_active or self.wave_finished:
            return {}
        
        self.spawn_timer += dt
        
        # ウェーブ開始前の遅延中は何もしない
        if self.spawn_timer < 0:
            return {}
        
        wave_def = self.waves[self.current_wave] if self.current_wave < len(self.waves) else None
        spawn_delay = wave_def["spawn_delay"] if wave_def else 1.0
        
        if self.spawn_timer >= spawn_delay and self.enemy_index < len(self.enemies_in_wave):
            # 次の敵をスポーン
            enemy_type = self.enemies_in_wave[self.enemy_index]
            self.enemy_index += 1
            self.spawn_timer = 0.0
            
            # すべての敵が出たかチェック
            if self.enemy_index >= len(self.enemies_in_wave):
                self.wave_active = False
            
            return {
                "type": enemy_type,
                "stats": self.get_enemy_stats(enemy_type),
                "remaining": len(self.enemies_in_wave) - self.enemy_index
            }
        
        return {}
    
    def finish_wave(self):
        """現在のウェーブを完了としてマーク"""
        if not self.wave_finished:
            self.wave_finished = True
            self.wave_active = False
            print(f"ウェーブ {self.current_wave + 1} 完了")
    
    def next_wave(self) -> bool:
        """
        次のウェーブに進む
        
        Returns:
            次のウェーブがあるかどうか
        """
        self.current_wave += 1
        self.wave_finished = False
        
        if self.current_wave >= len(self.waves) and self.current_wave < 20:  # 最大20ウェーブ
            # 無限モード用のランダムウェーブを生成
            self._generate_random_wave()
            return True
        elif self.current_wave >= 20:
            # ゲームクリア
            return False
        
        return True
    
    def get_wave_description(self) -> str:
        """現在のウェーブの説明を取得"""
        if self.current_wave < len(self.waves):
            return self.waves[self.current_wave]["description"]
        else:
            enemy_counts = {}
            for enemy in self.enemies_in_wave:
                enemy_counts[enemy] = enemy_counts.get(enemy, 0) + 1
            
            desc_parts = []
            for enemy, count in enemy_counts.items():
                desc_parts.append(f"{count} {enemy}")
            
            return f"無限ウェーブ: {', '.join(desc_parts)}"
    
    def get_wave_info(self) -> Dict[str, Any]:
        """現在のウェーブ情報を取得"""
        total_enemies = len(self.enemies_in_wave)
        remaining = total_enemies - self.enemy_index
        
        if self.current_wave < len(self.waves):
            wave_def = self.waves[self.current_wave]
            pre_wave_delay = wave_def["pre_wave_delay"]
            spawn_delay = wave_def["spawn_delay"]
        else:
            pre_wave_delay = 5.0
            spawn_delay = 1.0
        
        return {
            "wave_number": self.current_wave + 1,
            "total_waves": len(self.waves),
            "total_enemies": total_enemies,
            "enemies_remaining": remaining,
            "enemies_spawned": self.enemy_index,
            "spawn_delay": spawn_delay,
            "pre_wave_delay": pre_wave_delay,
            "description": self.get_wave_description(),
            "wave_active": self.wave_active,
            "wave_finished": self.wave_finished,
            "time_to_next_spawn": max(0, spawn_delay - self.spawn_timer) if self.wave_active else 0
        }
    
    def is_wave_complete(self) -> bool:
        """現在のウェーブが完了したかどうか"""
        return self.wave_finished or (not self.wave_active and self.enemy_index >= len(self.enemies_in_wave))
    
    def reset(self):
        """ウェーブマネージャーをリセット"""
        self.current_wave = 0
        self.enemies_in_wave = []
        self.spawn_timer = 0.0
        self.enemy_index = 0
        self.wave_active = False
        self.wave_finished = False


if __name__ == "__main__":
    # テストコード
    print("WaveManager テスト実行...")
    
    manager = WaveManager()
    
    # ウェーブ情報表示
    for i in range(min(6, len(manager.waves))):
        manager.current_wave = i
        print(f"ウェーブ {i+1}: {manager.get_wave_description()}")
    
    manager.reset()
    
    # ウェーブ1開始
    manager.start_wave(0)
    
    # シミュレーション
    print("\nウェーブ1シミュレーション:")
    sim_time = 0.0
    for _ in range(20):
        result = manager.update(1.0)  # 1秒ごとに更新
        sim_time += 1.0
        
        if result:
            print(f"  {sim_time:.1f}秒: {result['type']} が出現")
        
        if not manager.wave_active and manager.enemy_index >= len(manager.enemies_in_wave):
            print(f"ウェーブ完了！ {sim_time:.1f}秒かかりました")
            break