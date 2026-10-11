#!/bin/sh
# Android 用 APK (arm64) をビルドして署名する。
#
#   使い方: scripts/build-android.sh
#   出力  : dist-android/rumble-<version>-arm64.apk
#
# 既定ではデバッグ鍵で署名する (手元の端末での動作確認用)。
# 配布用の鍵で署名するには、次の環境変数を渡す:
#   ANDROID_KEYSTORE           キーストアのパス
#   ANDROID_KEYSTORE_PASSWORD  キーストアのパスワード
#   ANDROID_KEY_ALIAS          鍵の別名
#   ANDROID_KEY_PASSWORD       鍵のパスワード (省略時はキーストアと同じ)
set -eu

: "${JAVA_HOME:=/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
: "${ANDROID_HOME:=$HOME/Library/Android/sdk}"
: "${NDK_HOME:=$(ls -d "$ANDROID_HOME"/ndk/* | sort | tail -1)}"
export JAVA_HOME ANDROID_HOME NDK_HOME

# 同梱する OpenSSL のビルドが NDK の ar / ranlib を見つけられるようにする
ndk_bin=$(ls -d "$NDK_HOME"/toolchains/llvm/prebuilt/*/bin | head -1)
export AR_aarch64_linux_android="$ndk_bin/llvm-ar"
export RANLIB_aarch64_linux_android="$ndk_bin/llvm-ranlib"

cd "$(dirname "$0")/.."
pnpm tauri android build --apk --target aarch64

build_tools=$(ls -d "$ANDROID_HOME"/build-tools/* | sort | tail -1)
if [ -n "${ANDROID_KEYSTORE:-}" ]; then
  keystore=$ANDROID_KEYSTORE
  : "${ANDROID_KEYSTORE_PASSWORD:?ANDROID_KEYSTORE_PASSWORD が必要です}"
  : "${ANDROID_KEY_ALIAS:?ANDROID_KEY_ALIAS が必要です}"
  [ -n "${ANDROID_KEY_PASSWORD:-}" ] || ANDROID_KEY_PASSWORD=$ANDROID_KEYSTORE_PASSWORD
else
  keystore="$HOME/.android/debug.keystore"
  ANDROID_KEYSTORE_PASSWORD=android
  ANDROID_KEY_ALIAS=androiddebugkey
  ANDROID_KEY_PASSWORD=android
  if [ ! -f "$keystore" ]; then
    mkdir -p "$(dirname "$keystore")"
    "$JAVA_HOME/bin/keytool" -genkeypair -keystore "$keystore" -storepass android -keypass android \
      -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
      -dname "CN=Android Debug,O=Android,C=US"
  fi
fi
# パスワードはコマンドラインに出さず、環境変数で apksigner へ渡す
export ANDROID_KEYSTORE_PASSWORD ANDROID_KEY_PASSWORD

version=$(sed -n 's/.*"version": "\(.*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)
unsigned=src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
output="dist-android/rumble-$version-arm64.apk"
mkdir -p dist-android
"$build_tools/zipalign" -f -p 4 "$unsigned" "$output.aligned"
"$build_tools/apksigner" sign --ks "$keystore" \
  --ks-pass env:ANDROID_KEYSTORE_PASSWORD --key-pass env:ANDROID_KEY_PASSWORD \
  --ks-key-alias "$ANDROID_KEY_ALIAS" --out "$output" "$output.aligned"
rm -f "$output.aligned" "$output.idsig"
"$build_tools/apksigner" verify "$output" 2>/dev/null
echo "APK: $output"
