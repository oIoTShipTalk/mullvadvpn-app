#!/usr/bin/env bash

# This script is used to build, and optionally sign the app.
# See `README.md` for further instructions.

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

IS_USING_CONTAINER_SCRIPT=${1:-"false"}

commit_hash="$(git --no-pager log -1 --format=%H || echo gitless)"
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
cargo clean

echo "Build"
PATH_REMAPS=$(cargo run -q --bin remap-path-prefix)
mkdir -p $SCRIPT_DIR/build/libdest
pushd wireguard-go-rs/libwg/wireguard-go/maybenot
export RUSTFLAGS="-C metadata=maybenot-ffi $PATH_REMAPS"
if [ "$IS_USING_CONTAINER_SCRIPT" = "false" ]; then
    TARGET_DIR="$SCRIPT_DIR/target"
else
    TARGET_DIR="$SCRIPT_DIR/cargo-target/target"
fi
cargo build --target-dir $TARGET_DIR --release --target aarch64-linux-android --locked -j 1
cp $TARGET_DIR/aarch64-linux-android/release/libmaybenot_ffi.a $SCRIPT_DIR/build/libdest/libmaybenot.a
popd

echo "*******************"
echo "Build using: build-maybenot-ffi.sh"
echo "Commit hash: $commit_hash ($repo_state)"
echo "OS info: $OSTYPE (container=$IS_USING_CONTAINER_SCRIPT)"
go version
cargo version
echo "OUTPUT:"
md5sum $SCRIPT_DIR/build/libdest/*
echo "*******************"
