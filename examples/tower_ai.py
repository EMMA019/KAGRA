"""
tower_ai.py - タワーAIシステム（Python実装）

エンジンに不足しているAIターゲット選択機能をPython側で実装します。
TODO: エンジン側にAIコンポーネントシステムを追加推奨
"""

import math
from typing import List, Dict, Any, Optional


class TowerAI:
    """タワーAIクラス - ターゲット選択ロジック"""
    
    # TODO: エンジン側にAIコンポーネントシステムを実装
    # 提案API: kagra.AITargetingSystem(tower, strategy="nearest")
    
    def __init__(self, tower_position: tuple, tower_range: float, 
                 tower_type: str = "arrow"):
        """
        初期化
        
        Args:
            tower_position: タワー位置 (x, y)
            tower_range: タワーの射程範囲
            tower_type: タワータイプ ("arrow", "magic", "cannon")
        """
        self.x, self.y = tower_position
        self.range = tower_range
        self.type = tower_type
        self.current_target = None
        self.target_history = []
        
    def select_target(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """
        最適なターゲットを選択
        
        Args:
            enemies: 敵のリスト（各敵は辞書形式）
            
        Returns:
            選択された敵、またはNone
        """
        if not enemies:
            return None
        
        # 射程内の敵をフィルタリング
        in_range_enemies = []
        for enemy in enemies:
            if not enemy.get("active", True):
                continue
                
            # 射程内かチェック
            if self.is_in_range(enemy["x"], enemy["y"]):
                in_range_enemies.append(enemy)
        
        if not in_range_enemies:
            return None
        
        # タワータイプに応じた戦略でターゲットを選択
        if self.type == "arrow":
            return self._select_target_arrow(in_range_enemies)
        elif self.type == "magic":
            return self._select_target_magic(in_range_enemies)
        elif self.type == "cannon":
            return self._select_target_cannon(in_range_enemies)
        else:
            return self._select_target_default(in_range_enemies)
    
    def is_in_range(self, target_x: float, target_y: float) -> bool:
        """ターゲットが射程内かチェック"""
        dx = target_x - self.x
        dy = target_y - self.y
        distance = math.sqrt(dx*dx + dy*dy)
        return distance <= self.range
    
    def _select_target_arrow(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """
        矢塔用ターゲット選択戦略
        
        戦略: ベースに最も近い敵を優先（基本的な戦略）
              → 早期に脅威を排除
        """
        # ベースまでの距離でソート（簡易実装）
        # 実際のゲームではベース位置が必要
        if not enemies:
            return None
        
        # 現在の位置から最も近い敵を選択（簡易版）
        closest_enemy = None
        closest_distance = float('inf')
        
        for enemy in enemies:
            dx = enemy["x"] - self.x
            dy = enemy["y"] - self.y
            distance = math.sqrt(dx*dx + dy*dy)
            
            if distance < closest_distance:
                closest_distance = distance
                closest_enemy = enemy
        
        return closest_enemy
    
    def _select_target_magic(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """
        魔法塔用ターゲット選択戦略
        
        戦略: 敵が密集している場所の中心に近い敵を優先
              → 範囲攻撃の効果を最大化
        """
        if len(enemies) < 3:
            # 敵が少ない場合は最もHPが低い敵を優先
            return self._select_lowest_hp(enemies)
        
        # 敵の平均位置を計算
        avg_x = sum(e["x"] for e in enemies) / len(enemies)
        avg_y = sum(e["y"] for e in enemies) / len(enemies)
        
        # 平均位置に最も近い敵を選択
        closest_to_center = None
        closest_distance = float('inf')
        
        for enemy in enemies:
            dx = enemy["x"] - avg_x
            dy = enemy["y"] - avg_y
            distance = math.sqrt(dx*dx + dy*dy)
            
            if distance < closest_distance:
                closest_distance = distance
                closest_to_center = enemy
        
        return closest_to_center
    
    def _select_target_cannon(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """
        砲塔用ターゲット選択戦略
        
        戦略: HPが高い敵を優先（減速効果で長く足止めできる）
              → 高HP敵を遅くして他のタワーに倒させる
        """
        return self._select_highest_hp(enemies)
    
    def _select_target_default(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """デフォルト戦略: 最もHPが低い敵を優先"""
        return self._select_lowest_hp(enemies)
    
    def _select_lowest_hp(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """最もHPが低い敵を選択"""
        if not enemies:
            return None
        
        lowest_hp_enemy = min(enemies, key=lambda e: e.get("hp", float('inf')))
        return lowest_hp_enemy
    
    def _select_highest_hp(self, enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """最もHPが高い敵を選択"""
        if not enemies:
            return None
        
        highest_hp_enemy = max(enemies, key=lambda e: e.get("hp", 0))
        return highest_hp_enemy
    
    def _select_nearest_to_base(self, enemies: List[Dict[str, Any]], 
                               base_position: tuple) -> Optional[Dict[str, Any]]:
        """ベースに最も近い敵を選択"""
        if not enemies:
            return None
        
        base_x, base_y = base_position
        nearest_enemy = None
        nearest_distance = float('inf')
        
        for enemy in enemies:
            dx = enemy["x"] - base_x
            dy = enemy["y"] - base_y
            distance = math.sqrt(dx*dx + dy*dy)
            
            if distance < nearest_distance:
                nearest_distance = distance
                nearest_enemy = enemy
        
        return nearest_enemy
    
    def update_target_priority(self, target: Dict[str, Any], 
                              new_priority: float = 1.0):
        """ターゲットの優先度を更新（学習的要素）"""
        # ターゲット履歴に追加
        self.target_history.append({
            "target": target.get("type", "unknown"),
            "priority": new_priority,
            "timestamp": len(self.target_history)
        })
        
        # 履歴が長すぎたら古いものを削除
        if len(self.target_history) > 100:
            self.target_history = self.target_history[-50:]
    
    def get_targeting_stats(self) -> Dict[str, Any]:
        """AIのターゲティング統計を取得"""
        if not self.target_history:
            return {
                "total_targets": 0,
                "average_priority": 0.0,
                "most_targeted": "none"
            }
        
        # 最もターゲットにされた敵タイプ
        target_counts = {}
        for entry in self.target_history:
            enemy_type = entry["target"]
            target_counts[enemy_type] = target_counts.get(enemy_type, 0) + 1
        
        most_targeted = max(target_counts.items(), key=lambda x: x[1])[0] if target_counts else "none"
        
        return {
            "total_targets": len(self.target_history),
            "average_priority": sum(e["priority"] for e in self.target_history) / len(self.target_history),
            "most_targeted": most_targeted,
            "target_distribution": target_counts
        }


class AITargetingManager:
    """複数タワーのAIターゲティングを管理するマネージャークラス"""
    
    # TODO: エンジン側にAIマネージャーシステムを統合推奨
    
    def __init__(self):
        self.tower_ais = {}  # tower_id -> TowerAI
        self.global_strategy = "balanced"
        self.performance_stats = {
            "total_targets_selected": 0,
            "successful_targets": 0,
            "average_selection_time": 0.0,
        }
    
    def register_tower(self, tower_id: str, tower_position: tuple, 
                      tower_range: float, tower_type: str = "arrow"):
        """タワーをAIシステムに登録"""
        ai = TowerAI(tower_position, tower_range, tower_type)
        self.tower_ais[tower_id] = ai
        return ai
    
    def unregister_tower(self, tower_id: str):
        """タワーをAIシステムから削除"""
        if tower_id in self.tower_ais:
            del self.tower_ais[tower_id]
    
    def update_tower_position(self, tower_id: str, new_position: tuple):
        """タワーの位置を更新"""
        if tower_id in self.tower_ais:
            self.tower_ais[tower_id].x, self.tower_ais[tower_id].y = new_position
    
    def select_target_for_tower(self, tower_id: str, 
                               enemies: List[Dict[str, Any]]) -> Optional[Dict[str, Any]]:
        """特定のタワーのターゲットを選択"""
        if tower_id not in self.tower_ais:
            return None
        
        ai = self.tower_ais[tower_id]
        target = ai.select_target(enemies)
        
        if target:
            self.performance_stats["total_targets_selected"] += 1
            
            # 簡易的な成功判定（ターゲットが射程内か）
            if ai.is_in_range(target["x"], target["y"]):
                self.performance_stats["successful_targets"] += 1
        
        return target
    
    def set_global_strategy(self, strategy: str):
        """グローバル戦略を設定"""
        valid_strategies = ["balanced", "aggressive", "defensive", "efficient"]
        if strategy in valid_strategies:
            self.global_strategy = strategy
            
            # 戦略に応じたAIパラメータ調整（簡易実装）
            if strategy == "aggressive":
                # 攻撃的: 最も近い敵を常に優先
                pass
            elif strategy == "defensive":
                # 防御的: ベースに近い敵を優先
                pass
            elif strategy == "efficient":
                # 効率的: HPが低い敵を優先（効率的なリソース使用）
                pass
    
    def get_performance_report(self) -> Dict[str, Any]:
        """パフォーマンスレポートを取得"""
        total_towers = len(self.tower_ais)
        success_rate = 0.0
        if self.performance_stats["total_targets_selected"] > 0:
            success_rate = (self.performance_stats["successful_targets"] / 
                          self.performance_stats["total_targets_selected"]) * 100
        
        return {
            "total_towers": total_towers,
            "total_targets_selected": self.performance_stats["total_targets_selected"],
            "successful_targets": self.performance_stats["successful_targets"],
            "success_rate_percent": success_rate,
            "global_strategy": self.global_strategy,
            "average_selection_time": self.performance_stats["average_selection_time"]
        }
    
    def reset_stats(self):
        """統計をリセット"""
        self.performance_stats = {
            "total_targets_selected": 0,
            "successful_targets": 0,
            "average_selection_time": 0.0,
        }


# 簡易テスト
if __name__ == "__main__":
    print("TowerAI テスト実行...")
    
    # テストデータ作成
    test_enemies = [
        {"x": 100, "y": 100, "hp": 30, "type": "slime", "active": True},
        {"x": 150, "y": 150, "hp": 50, "type": "goblin", "active": True},
        {"x": 200, "y": 200, "hp": 100, "type": "orc", "active": True},
        {"x": 250, "y": 250, "hp": 20, "type": "slime", "active": True},
    ]
    
    # TowerAIインスタンス作成
    tower_position = (120, 120)
    ai = TowerAI(tower_position, tower_range=150, tower_type="arrow")
    
    # ターゲット選択テスト
    target = ai.select_target(test_enemies)
    
    if target:
        print(f"選択されたターゲット: {target['type']} (HP: {target['hp']})")
        print(f"位置: ({target['x']}, {target['y']})")
    else:
        print("ターゲットが見つかりませんでした")
    
    # AIマネージャーテスト
    print("\nAITargetingManager テスト:")
    manager = AITargetingManager()
    
    # タワー登録
    tower1_id = "tower_1"
    manager.register_tower(tower1_id, (100, 100), 120, "magic")
    
    # ターゲット選択
    target = manager.select_target_for_tower(tower1_id, test_enemies)
    if target:
        print(f"マネージャー経由で選択: {target['type']}")
    
    # パフォーマンスレポート
    report = manager.get_performance_report()
    print(f"\nパフォーマンスレポート:")
    print(f"  タワー数: {report['total_towers']}")
    print(f"  選択数: {report['total_targets_selected']}")
    print(f"  成功率: {report['success_rate_percent']:.1f}%")