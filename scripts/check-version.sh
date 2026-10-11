#!/bin/sh
# リリースのタグと、ファイルに書かれたバージョンが一致しているかを確かめる。
# バージョンは package.json・Cargo.toml・src-tauri/tauri.conf.json の 3 か所にある。
#
#   使い方: scripts/check-version.sh v1.2.3
set -eu

tag=${1:?"使い方: $0 <タグ>"}
expected=${tag#v}
cd "$(dirname "$0")/.."

json_version() { sed -n 's/.*"version": *"\(.*\)".*/\1/p' "$1" | head -1; }

status=0
check() {
  if [ "$2" != "$expected" ]; then
    echo "$1 のバージョンは $2 ですが、タグは $tag です" >&2
    status=1
  fi
}
check package.json "$(json_version package.json)"
check src-tauri/tauri.conf.json "$(json_version src-tauri/tauri.conf.json)"
check Cargo.toml "$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
exit $status
