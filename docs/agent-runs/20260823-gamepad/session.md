# Session — 2026-08-23 Gamepad

部屋トラック最後の「README のまだ無い」。

## 判断

- 公開面は `axis` / `pad` / `inject_pad`。新しい Controller クラスは作らない。
- `Walk` が左スティックで歩き、右スティックで見る。キーとマウスは今まで通り。
- テストとスモークは `inject_pad`。Prop Garden のスモークは今まで通り `inject_key("W")`。
- 実機 gilrs は Cargo 1.83 が edition2024 クレートで落ちるのでこの PR では入れない。
  `poll_pad()` はエンジンにメソッドがあれば読む。

## Verify

`pytest tests -m "not golden"`。GPU シナリオは未実行。
