#!/usr/bin/env bash

# This script is used to build, and optionally sign the app.
# See `README.md` for further instructions.

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

IS_USING_CONTAINER_SCRIPT=${1:-"false"}

commit_hash="$(git --no-pager log -1 --format=%H)"
repo_state="$(test -z "$(git status --porcelain)" && echo "CLEAN" || echo "DIRTY")"

echo "Commit hash: $commit_hash ($repo_state)"
echo "OS info: $OSTYPE (container=$IS_USING_CONTAINER_SCRIPT)"

echo "Clean cargo"
cargo clean

echo "Starting build of wireguard-go-rs"
echo "Current go version"
go version
echo "Current rust version"
cargo version

echo "Clean"
# make clean

echo "Build"
mkdir -p /build/build/libdest
pushd wireguard-go-rs/libwg/wireguard-go/maybenot
export RUSTFLAGS="-C metadata=maybenot-ffi --remap-path-prefix /root/.cargo=/CARGO_HOME --remap-path-prefix /root/.rustup=/RUSTUP_HOME --remap-path-prefix /build=/SOURCE_DIR" 
cargo build --target-dir /cargo-target/target --release --target aarch64-linux-android
cp /cargo-target/target/aarch64-linux-android/release/libmaybenot_ffi.a /build/build/libdest/libmaybenot.a
popd

echo "*******************"
echo "Build using: build-maybenot-ffi.sh"
echo "Commit hash: $commit_hash ($repo_state)"
echo "OS info: $OSTYPE (container=$IS_USING_CONTAINER_SCRIPT)"
go version
cargo version
echo "OUTPUT:"
md5sum /build/build/libdest/*
echo "*******************"
