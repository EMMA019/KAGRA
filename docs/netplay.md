# ローカル対戦と入力同期

スマートフォンを主対象にしたオンライン対戦の一歩。シミュレーションは Rust
（`kagra-shared`）にあり、Android / Windows / wasm が同じティックを共有する。
ソケットはこのクレートに入れない。リレーはホスト側の仕事。

## なぜ入力同期か

状態同期（毎フレームの World dump）はメッセージが大きく、float の再シリアライズで
ホスト間がずれやすい。対戦ラウンドは「初期 dump + 両者の入力 + 固定 dt」の純関数
なので、入力だけを届けて両方で同じ `FightSession.tick` を回せば状態は一致する。

予測とロールバックは入れていない。そのティックの全スロットが揃ってから
1 フレーム進む。遅延はそのまま待ち時間になる。同じコードパスで状態が割れない
ことを先に固定する。

## プレイヤー枠

`GenreSession::set_player_input(slot, input)` が共通の入口。スロット 0 は従来の
`set_input`。他ジャンルは既定実装のままスロット 1 以降を無視する。

`fight_hitstun` だけスロット 1 を実装した。`None` のあいだ相手は従来の AI。
スロット 1 を渡した瞬間から相手カプセルは人間入力（移動・攻撃・ガード）になる。
ローカル対戦の見本は `examples/versus_local.py`。

- キーボード: P1 は WASD / F 攻撃 / C ガード。P2 は矢印 / 1 攻撃 / 2 ガード。
- タッチ: 画面の左半分が P1、右半分が P2。スティックは外側、ATK / BLK は中央寄り。

## メッセージ

JSON。タグは `op`。スティックは -1000..1000 の整数（`f32` をそのまま載せない）。
`start` の dt は `dt_num` / `dt_den`（1 と 60）。ホストは `Lockstep.dt` で
`f32` に戻す。Python 側で割らない。

| op | 向き | 中身 |
|---|---|---|
| `hello` | client → relay | `proto` `room` `name` |
| `welcome` | relay → client | `slot` `room` `seed` `peers` |
| `start` | relay → both | `tick0` `dt_num` `dt_den` |
| `input` | client → relay → peer | `input.slot` `tick` `lx` `lz` `jump` `attack` `dodge` |
| `bye` | either | `reason` |

`seed` は将来の乱数用。今の人間同士の格闘は乱数を使わない。両者は同じ
fixture から始める。

Rust API: `Lockstep` / `NetMessage`（`kagra-shared/src/netplay.rs`）。
Python 橋: `kagra.netplay.RoomClient`。C ABI: `kagra_net_lockstep_new` ほか。
wasm（feature `wasm`）: `WasmLockstep` / `netHello` / `netDecode` / `netDt`。

## ローカルリレー

```bash
python server/netplay_relay.py --host 127.0.0.1 --port 8765
python examples/versus_online.py --port 8765 --name a --ticks 90 --out scratch/a.json
python examples/versus_online.py --port 8765 --name b --ticks 90 --out scratch/b.json
```

部屋はメモリ上だけで、2 人まで。3 人目は `room full`。入力の `slot` は
サーバが接続に割り当てた値で上書きする。

ヘッドレスで両者の dump が一致することは:

```bash
python examples/versus_online.py --match --ticks 90
```

## 無料で置くなら（このリポジトリからはデプロイしない）

リレーは部屋テーブルをメモリに持つだけのプロセス。ディスクは要らない。

- **Cloudflare Workers + Durable Objects**。部屋 1 つを Durable Object 1 つに
  対応させ、WebSocket Hibernation API でフレームを相手に渡す。無料枠の範囲で
  試せる。常時プロセスを持たないので、この Python リレーをそのままは動かさない。
  メッセージ JSON はそのまま使える。
- **Render の無料 Web Service**。`0.0.0.0:$PORT` で `server/netplay_relay.py`
  を置く形に近い。WebSocket を通せる。15 分操作がないとスリープする。
  ファイルシステムは揮発なので、部屋をディスクに保存しない今の形と合う。
  無料枠は期限とスリープがある。有料プランには上げない。

どちらもこの変更では作らない。ローカルのリレーとドキュメントだけ。
