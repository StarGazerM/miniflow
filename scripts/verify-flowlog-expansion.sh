#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/cargo-env.sh"
readonly FLOWLOG_DIR="${ROOT_DIR}/flowlog"
readonly MANIFEST="${ROOT_DIR}/parity/flowlog/manifest.tsv"
readonly COMPILER="${CARGO_TARGET_DIR}/debug/flowlog-compiler"

"${ROOT_DIR}/scripts/verify-flowlog-oracle.sh"

cargo build \
    --manifest-path "${FLOWLOG_DIR}/Cargo.toml" \
    -p flowlog-compiler

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-expansion.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

export FLOWLOG_RUNTIME_PATH="${FLOWLOG_DIR}/flowlog-runtime"
while IFS=$'\t' read -r fixture _output canonical_example _runtime_example mode; do
    fixture_dir="${ROOT_DIR}/parity/flowlog/${fixture}"
    fixture_work="${work_dir}/${fixture}"
    mkdir -p "${fixture_work}"
    compiler_flags=()
    if [[ "${mode}" == "profile" ]]; then
        compiler_flags+=(--profile)
    elif [[ "${mode}" != "plain" ]]; then
        echo "unknown FlowLog parity mode: ${mode}" >&2
        exit 1
    fi

    CARGO_TARGET_DIR="${work_dir}/flowlog-target" "${COMPILER}" \
        "${fixture_dir}/program.dl" \
        "${compiler_flags[@]}" \
        --check \
        -B "${fixture_work}/flowlog-build" \
        -D output
    CARGO_TARGET_DIR="${work_dir}/flowlog-target" "${COMPILER}" \
        "${fixture_dir}/program.dl" \
        "${compiler_flags[@]}" \
        --check \
        -B "${fixture_work}/flowlog-build-repeat" \
        -D output

    cargo run \
        --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example "${canonical_example}" \
        >"${fixture_work}/miniflow.rs"

    cargo run \
        --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example extract_dataflow_core \
        -- \
        "${fixture_work}/flowlog-build/src/main.rs" \
        >"${fixture_work}/flowlog-core.rs"
    cargo run \
        --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example extract_dataflow_core \
        -- \
        "${fixture_work}/flowlog-build-repeat/src/main.rs" \
        >"${fixture_work}/flowlog-core-repeat.rs"
    cargo run \
        --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example extract_dataflow_core \
        -- \
        "${fixture_work}/miniflow.rs" \
        >"${fixture_work}/miniflow-core.rs"

    cmp \
        "${fixture_work}/flowlog-core.rs" \
        "${fixture_work}/flowlog-core-repeat.rs"
    cmp \
        "${fixture_work}/flowlog-core.rs" \
        "${fixture_work}/miniflow-core.rs"
done <"${MANIFEST}"

echo "FlowLog/MiniFlow canonical dataflow-core parity: all fixtures passed"
