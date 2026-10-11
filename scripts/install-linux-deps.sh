#!/bin/sh
# Linux (Debian / Ubuntu) で rumble をビルドするのに必要なパッケージを入れる。
# CI と、手元の Linux での開発の両方で使う。
#
#   使い方: scripts/install-linux-deps.sh
set -eu

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential \
  file \
  libayatana-appindicator3-dev \
  libdbus-1-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  patchelf \
  pkg-config
