# 配布（Windows と Android）

ジャンル例を、人が起動できる形にする。公開はしない。GitHub Actions の
artifact を手元に落として、itch.io にはブラウザから手動で上げる。有料の
ストアや署名サービスは使わない。

Windows と Android は同じマニフェストを読む。

## マニフェスト

`games/<例のファイル名>.json`。スキーマは
[`docs/schemas/game_manifest.json`](schemas/game_manifest.json)。

```json
{
  "title": "Box Hop",
  "icon": "games/icons/platformer.png",
  "version": "0.1.0",
  "entry": "examples/platformer_minimal.py",
  "world": "kagra-shared/tests/fixtures/platformer_world.json"
}
```

| キー | 必須 | 意味 |
|---|---|---|
| `title` | はい | ランチャー名。UTF-8。Android のラベルになる |
| `icon` | はい | リポジトリ相対の PNG |
| `version` | はい | `major.minor.patch`。`versionCode` は `major*10000+minor*100+patch` |
| `entry` | はい | 起動する Python 例 |
| `world` | いいえ | Android の WorldPlay が読む dump |
| `application_id` | いいえ | 省略時は `dev.kagra.<slug>` |
| `bundle_assets` | いいえ | 追加で同梱するパス。`["assets"]` で `assets/` 全体 |

`--game` には例のパスを渡す。`games/platformer_minimal.json` のように JSON を
直接渡してもよい。

```bash
python scripts/package_windows.py --game examples/platformer_minimal.py
python scripts/package_android.py --game examples/platformer_minimal.py
```

`world` がある例は、電話では Rust の `WorldPlay` がその dump を進める
（タイトルでタップすると開始。左スティックが移動、右下がジャンプ、右上が
アクション）。Python 側の HUD や独自ルールは Windows の exe がそのまま実行する。
`world` が無い例（チェス、バー、飛行など）の APK は名前とアイコンは付くが、
画面に「ルールは Python 側」と出る。遊べる本体は Windows の exe。

参照アセット（`knight` や `crate.glb` など）は dump と例のソースから集めて
同梱する。リポジトリに無いファイル（gitignore された jpg など）は入らない。

## Windows

成果物はフォルダと zip。中に `<slug>.exe` がある。PyInstaller の onedir で、
Python と `kagra.kagra_shared`（maturin の wheel）とステージした例・dump・
アセットを一緒に入れる。

exe を作れるのは Windows だけ。他の OS ではレイアウト確認まで:

```bash
python scripts/package_windows.py --game examples/platformer_minimal.py --stage-only
```

Windows での手順:

```bat
maturin build --release --locked --out dist\wheel
pip install dist\wheel\*.whl
pip install "pyinstaller>=6"
python scripts\package_windows.py --game examples\platformer_minimal.py
```

zip は `dist/windows/<slug>-windows.zip`。展開して exe を起動する。
コンソールが残るのは、足りないモジュールのトレースを見るため。

描画は **Vulkan**。共有コアは dx12 を切ってある（wgpu-hal 30 と
gpu-allocator の組み合わせが Windows でコンパイルできないため）。
Vulkan ドライバの入った PC で試す。アダプタが無いと `draw_world` は
単色の枠に落ちる。

GitHub Actions（無料の `windows-latest`）:
[`.github/workflows/package-windows.yml`](../.github/workflows/package-windows.yml)。
pull request では `examples/platformer_minimal.py` を包む。別の例は
Actions の **Run workflow** で `game` にパスを入れる。zip は artifact。
リリースとして公開はしない。

実機確認:

1. artifact の zip を Windows PC に落とす
2. 展開する（`_internal` を exe から離さない）
3. exe をダブルクリックする
4. タイトルで Space または Enter、WASD で遊ぶ
5. 窓が真っ黒なときは GPU の Vulkan を確認する。コンソールに `no GPU adapter` と出る

## Android

既存のシェル（`mobile/android`、JNI、`scripts/build_android_native.sh`）を
`--game` で切り替える。デバッグ APK は Android SDK の debug keystore で
署名される。自分で keystore を作らなくても実機に入る。

```bash
# cargo-ndk と ANDROID_NDK_HOME（NDK r29 / 29.0.14206865、16 KB ページ）
python scripts/package_android.py --game examples/platformer_minimal.py
```

中でやること:

1. `mobile/android/kagra.game.properties` に title / version / applicationId / icon
2. `app/src/main/assets/kagra_root/` に例・dump・アセット
3. `drawable-mdpi/ic_launcher.png`
4. `libkagra_shared.so`（arm64-v8a と x86_64、`--features render`、`--locked`）
5. `gradle :app:assembleDebug`

`.so` が無いとスクリプトは失敗する。ネイティブを飛ばして設定だけ見るときは
`--skip-native --stage-only`。マニフェストの無い素の `assembleDebug` は
今までどおり Crest Isle（CI の `android-shell` ジョブ）。

APK は `dist/android/<slug>-debug.apk`。

GitHub Actions（無料の `ubuntu-latest`）:
[`.github/workflows/package-android.yml`](../.github/workflows/package-android.yml)。
NDK r29・cargo-ndk・Gradle 8.7・AGP 8.5.2・platform 35 を入れて debug APK を artifact にする。

16 KB ページ（Android 15 以降、Pixel 10 など）では、各 `.so` の ELF LOAD の
`p_align` が 16384 以上で、APK 内では非圧縮かつ 16 KB zip アラインである必要がある。
Rust は `.cargo/config.toml` の Android ターゲットへ
`-Wl,-z,max-page-size=16384` と `common-page-size` を渡す。JNI も同じリンカ引数。
`libc++_shared.so` は NDK r28+ のプリビルドを使う（`c++_shared`）。
パッケージは `useLegacyPackaging = false` で、AGP 8.5.1 以降が `zipalign -P 16` する。
CI は `llvm-readelf -l` と `zipalign -c -P 16 -v 4` で落とす。手元では:

```bash
python scripts/check_android_16k.py dist/android/platformer-debug.apk
```

ネイティブのロードに失敗したときは、画面に `UnsatisfiedLinkError` の全文が出る。

実機確認:

1. 電話で **提供元不明のアプリ** を、この APK を入れたファイルマネージャに対して許可する
2. artifact の APK を電話にコピーして開く
3. 初回は `kagra_root` をアプリ領域へ展開する（アセットの量だけ待つ）
4. 画面をタップしてタイトルを抜け、左で歩き、右下でジャンプする
5. 画面がビルド手順のテキストだけなら、その APK に `.so` が入っていない

`applicationId` が例ごとに違うので、箱跳びとレースは並べてインストールできる。

### リリース署名（秘密は作らない）

デバッグ鍵の APK は配布の試しには足りる。itch.io に長く置くなら自分の
keystore で `assembleRelease` する。鍵はリポジトリに置かない。

ローカルで鍵を作る（パスワードは自分で決める。コマンド履歴に残したくなければ
対話の `keytool` を使う）:

```bash
keytool -genkeypair -v \
  -keystore release.keystore -alias kagra \
  -keyalg RSA -keysize 2048 -validity 10000
```

このファイルとパスワードを GitHub の **Settings → Secrets and variables → Actions**
に自分で入れる。ワークフローは秘密を作らない。

| secret | 中身 |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | `base64 -w0 release.keystore` の1行 |
| `ANDROID_KEYSTORE_PASSWORD` | store のパスワード |
| `ANDROID_KEY_ALIAS` | 上の例なら `kagra` |
| `ANDROID_KEY_PASSWORD` | 鍵のパスワード（store と同じでよい） |

Actions の **Package Android → Run workflow** で `sign_release` をオンにする。
`assembleRelease` が走り、`dist/android/<slug>-release.apk` が artifact に加わる。
秘密が空だとスクリプトは署名せずに終了する。

ローカルで同じことをするとき:

```bash
export ANDROID_KEYSTORE_BASE64="$(base64 -w0 release.keystore)"
export ANDROID_KEYSTORE_PASSWORD="..."
export ANDROID_KEY_ALIAS="kagra"
export ANDROID_KEY_PASSWORD="..."
python scripts/package_android.py --game examples/platformer_minimal.py --release
```

`ANDROID_KEYSTORE_FILE` に鍵のパスを直接渡しても Gradle は署名する。
base64 は Actions 用。

## 直してここ

デバッグ APK、ブラウザプレビュー、Windows の zip では、プレイ中に
3 本指の長押し（タッチ）か F12（キーボード）で一時停止する。メモ（日本語）
と直近約 10 秒の状態から、トークンなしで GitHub の playtest issue 下書きを
開く。スクリーンショットはギャラリーかダウンロードに保存し、issue に貼る。
リリース用のストアビルド（`KAGRA_PLAYTEST=0`、Android release）では出ない。

再現は `python scripts/repro_issue.py <issue 番号またはブロック>`。
直す前と直した後のスクリーンショットを残す。

## itch.io に上げる（手動）

アカウントは無料。自動アップロードはしない（butler の API キーも置かない）。

1. itch.io でプロジェクトを作る。Kind は HTML ではなく **Downloadable**
2. Windows の zip を **windows** としてアップロードする。このページでは
   「これは zip のまま実行する」ではなく、落として展開して exe を起動する、
   と説明文に書く
3. Android の APK を **android** としてアップロードする。説明に
   「提供元不明のアプリを許可してインストール」と書く
4. Pricing は No payments のままでよい。**Public** にする前に、下の実機確認を終える
5. このリポジトリの Actions からは公開ボタンを押さない

## タイトル・セーブ・音

既定のパッケージ（`examples/platformer_minimal.py` / Box Hop）は、起動すると
タイトルで待つ。Space / Enter / タップで始まり、P または右上の PAUSE で
止まり、ゴールか落下のあと RETRY / TITLE に戻る。音量とミュートはポーズから
変える。ハイスコアは JSON で残る。

| OS | パス |
|---|---|
| Windows | `%APPDATA%\kagra\box_hop.json` |
| Android | アプリ内部ストレージ `filesDir/kagra/box_hop.json` |
| その他 | `$XDG_DATA_HOME/kagra` か `~/.local/share/kagra` |

効果音と BGM は `assets/audio/` の CC0 合成音（`LICENSE.txt`）。デバイスが
無い CI や lavapipe ではミキサーが無音のままフレームを止めない。Android は
同じキューを SoundPool / MediaPlayer で鳴らす。

## 何が配布物に入るか

- Windows: 例の Python、`kagra` パッケージ、ネイティブモジュール、dump、
  参照された glTF、マニフェスト、短い README
- Android: 同じステージを `assets/kagra_root` に入れた debug APK（または
  上の秘密で署名した release APK）と、`libkagra_shared.so`

入らないもの: PyPI への publish、itch.io への自動投稿、keystore 本体、
iOS / ブラウザ用のこのフェーズのパッケージ（既存の wasm 手順は
[`mobile/README.md`](../mobile/README.md) のまま）。
