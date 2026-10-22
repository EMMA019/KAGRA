# スマホで試す（ケーブル不要）

エージェントが「できた」と言ったあと、Emma が USB 無しで電話から遊ぶための道。
有料のホスティングも、リポジトリに置く秘密も使わない。GitHub Pages と、
デバッグ署名の APK だけ。

## ブラウザ

GitHub Actions の [Preview](../.github/workflows/preview.yml) が `games/*.json` を
wasm の WorldPlay に載せる。

| 置き場所 | URL |
|---|---|
| `master` | `https://emma019.github.io/KAGRA/` |
| pull request `N` | `https://emma019.github.io/KAGRA/pr/N/` |

先頭はジャンル一覧。`world` がある例はすべてリンクする。Box Hop は一番上。
`world` が無い例（チェス、飛行など）は一覧に名前だけ出て、ブラウザでは進まない。
ルールは Python 側のまま。

PR を開くたびに、プレビューの URL と QR 画像を PR コメントへ書く。同じコメントを
更新する。PR を閉じると `/pr/N/` を消す。

電話のブラウザで開く。操作は Android のパッケージと同じ。

- 左：移動（スティック）
- 右下：ジャンプ
- 右上：アクション
- タイトルは画面をタップ、または「はじめる」

縦でも横でも、画面の縦横に合わせてスティックとボタンが付け替わる。
ピンチズームはしない（ゲームのタップとぶつかるため）。

ローカルで同じページを見るとき:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127
./scripts/build_wasm.sh
python scripts/build_browser_preview.py --out dist/preview --require-pkg
python -m http.server -d dist/preview 8000
# http://localhost:8000/  と  http://localhost:8000/play.html?game=platformer
```

`crest.html`（Crest Isle）と運転デモは今までどおり `kagra-shared/www/`。

## KAGRA Dev（Android）

ゲームを APK に焼かない、デバッグ専用の別アプリ。`applicationId` は
`dev.kagra.devshell`。箱跳び（`dev.kagra.platformer`）と並べて入る。

Package Android の artifact **`kagra-dev-apk`** を一度だけ電話に入れる
（提供元不明のアプリを、そのファイルマネージャに許可する）。あとはケーブル不要。

起動画面:

1. Pages の URL（CI が焼いた値。変えられる）
2. **master** か **PR**（番号）
3. 一覧からゲームを選ぶか、slug を入れる（Box Hop は `platformer`）

読み込むものは、プレビューと同じ束。`bundle.json` の各ファイルは
`path`（展開先）と `url`（Pages 上の場所）を持つ。アセットはゲーム間で
共有し、世界が参照するものだけを置く。
遊びながら **再読込** を押すか、上のバーを下に引くと、同じ URL を取り直す。
エージェントが Pages を更新したあと、ケーブル無しで最新を見られる。

ネイティブの `.so` は今までどおり `scripts/build_android_native.sh` が
`app` モジュールへ置く。Dev はその `.so` をリンクするだけで、ビルドスクリプトは
触らない。JNI と `libc++_shared.so` はゲーム APK と同じ 16 KB ページ
（NDK r29、`max-page-size` と `common-page-size`）。Package Android は
両方の APK を `scripts/check_android_16k.py` で確認する。

## 何が公開されるか

Pages に出るのはステージした世界とアセットと wasm。keystore も API トークンも
置かない。デバッグ APK の署名は Android SDK の debug keystore。リリース署名の
秘密は [distribution.md](distribution.md) のまま、このプレビューでは使わない。
