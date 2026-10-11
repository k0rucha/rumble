#!/bin/sh
# Android 用 APK (arm64) をビルドし、手元の端末で試せるようデバッグ鍵で署名する。
#
#   使い方: scripts/build-android.sh
#   出力  : dist-android/rumble-<version>-arm64.apk
#
# デバッグ鍵での署名は動作確認用。配布する場合は自分の鍵で署名し直すこと。
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
keystore="$HOME/.android/debug.keystore"
if [ ! -f "$keystore" ]; then
  mkdir -p "$(dirname "$keystore")"
  "$JAVA_HOME/bin/keytool" -genkeypair -keystore "$keystore" -storepass android -keypass android \
    -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Android,C=US"
fi

version=$(sed -n 's/.*"version": "\(.*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)
unsigned=src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
output="dist-android/rumble-$version-arm64.apk"
mkdir -p dist-android
"$build_tools/zipalign" -f -p 4 "$unsigned" "$output.aligned"
"$build_tools/apksigner" sign --ks "$keystore" --ks-pass pass:android --key-pass pass:android \
  --ks-key-alias androiddebugkey --out "$output" "$output.aligned" 2>/dev/null
rm -f "$output.aligned" "$output.idsig"
"$build_tools/apksigner" verify "$output" 2>/dev/null
echo "APK: $output"
