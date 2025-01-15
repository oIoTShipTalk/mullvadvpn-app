#!/bin/bash
# From: https://github.com/rust-lang/cargo/issues/8140#issuecomment-639381024
set -e

file=
for arg in "$@"; do
    if [[ ${arg} == ${CARGO_HOME}/* && -z ${file} ]]; then
        file=${arg##${CARGO_HOME}}
    fi
done

args=()
for arg in "$@"; do
    if [[ ${arg} == metadata=* ]]; then
        args+=(metadata="${file}")
    else
        args+=("${arg}")
    fi
done

exec "${args[@]}"
