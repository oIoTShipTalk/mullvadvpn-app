#!/usr/bin/env bash

# This script is used to build, and optionally sign the app.
# See `README.md` for further instructions.

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "Commit hash"
git --no-pager log -1 --format=%H

echo "Clean cargo"
cargo clean

echo "Starting build of wireguard-go-rs"
echo "Current go version"
go version
echo "Current rust version"
cargo version

cd wireguard-go-rs

echo "Clean"
# make clean

echo "Build"
cargo build --release --target aarch64-linux-android

echo "*******************"
echo "Commit hash"
git --no-pager log -1 --format=%H
go version
cargo version
md5sum ../target/aarch64-linux-android/release/build/wireguard-go-rs*/out/*
echo "*******************"
