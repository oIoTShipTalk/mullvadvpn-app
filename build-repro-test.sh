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

cd repro-test

echo "Clean"
# make clean

echo "Build"
cargo build --locked --release --target aarch64-linux-android

echo "*******************"
echo "Commit hash: $commit_hash ($repo_state)"
echo "OS info: $OSTYPE (container=$IS_USING_CONTAINER_SCRIPT)"
go version
cargo version
md5sum ../build/target/aarch64-linux-android*/release/*
echo "*******************"
