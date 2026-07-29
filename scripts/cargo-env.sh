#!/usr/bin/env bash

if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/miniflow-target"
elif [[ "${CARGO_TARGET_DIR}" != /* ]]; then
    export CARGO_TARGET_DIR="$(pwd)/${CARGO_TARGET_DIR}"
fi

export CARGO_INCREMENTAL=0
export CARGO_CACHE_RUSTC_INFO=0
unset RUSTC_WRAPPER CARGO_BUILD_RUSTC_WRAPPER
