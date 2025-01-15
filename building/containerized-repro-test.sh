#!/usr/bin/env bash

# Builds the Android or Linux app in the current build container.
# See the `container-run.sh` script for possible configuration.

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPO_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"
cd "$SCRIPT_DIR"

source "$REPO_DIR/scripts/utils/log"

echo "Dirty files"
git diff-files

build_command=("./build-repro-test.sh")

set -x
exec "$SCRIPT_DIR/container-run.sh" android "${build_command[@]}" "true" "$@"
