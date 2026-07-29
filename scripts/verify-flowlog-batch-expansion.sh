#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly FLOWLOG_DIR="${ROOT_DIR}/flowlog"
readonly FIXTURES="${FLOWLOG_DIR}/tests/fixtures/datalog-batch"
readonly MANIFEST="${ROOT_DIR}/corpus/flowlog-batch/manifest.tsv"
readonly COMPILER="${FLOWLOG_DIR}/target/debug/flowlog-compiler"

"${ROOT_DIR}/scripts/verify-flowlog-oracle.sh"
cargo build \
    --manifest-path "${FLOWLOG_DIR}/Cargo.toml" \
    -p flowlog-compiler
cargo build \
    --manifest-path "${ROOT_DIR}/Cargo.toml" \
    -p miniflow-flowlog-batch-corpus \
    --bin canonical

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-batch-expansion.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT
export FLOWLOG_RUNTIME_PATH="${FLOWLOG_DIR}/flowlog-runtime"

strict=0
while IFS=$'\t' read -r fixture _family status; do
    [[ "${fixture}" == \#* || -z "${fixture}" || "${status}" != "strict" ]] && continue
    fixture_dir="${FIXTURES}/${fixture}"
    fixture_work="${work_dir}/${fixture}"
    mkdir -p "${fixture_work}"

    compiler_flags=()
    if [[ -f "${fixture_dir}/udf.rs" ]]; then
        compiler_flags+=(--udf-file "${fixture_dir}/udf.rs")
    fi
    if [[ -f "${fixture_dir}/compile_flags" ]]; then
        while IFS= read -r line || [[ -n "${line}" ]]; do
            [[ -z "${line}" || "${line}" =~ ^[[:space:]]*# ]] && continue
            read -ra line_flags <<<"${line}"
            compiler_flags+=("${line_flags[@]}")
        done <"${fixture_dir}/compile_flags"
    fi
    if [[ -f "${fixture_dir}/include_dirs" ]]; then
        while IFS= read -r include_dir || [[ -n "${include_dir}" ]]; do
            [[ -z "${include_dir}" ]] && continue
            compiler_flags+=(-I "${fixture_dir}/${include_dir}")
        done <"${fixture_dir}/include_dirs"
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
    "${ROOT_DIR}/target/debug/canonical" "${fixture}" \
        >"${fixture_work}/miniflow.rs"

    cargo run --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example extract_dataflow_core \
        -- "${fixture_work}/flowlog-build/src/main.rs" \
        >"${fixture_work}/flowlog-core.rs"
    cargo run --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example extract_dataflow_core \
        -- "${fixture_work}/flowlog-build-repeat/src/main.rs" \
        >"${fixture_work}/flowlog-core-repeat.rs"
    cargo run --quiet \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        -p miniflow-core \
        --example extract_dataflow_core \
        -- "${fixture_work}/miniflow.rs" \
        >"${fixture_work}/miniflow-core.rs"

    cmp "${fixture_work}/flowlog-core.rs" "${fixture_work}/flowlog-core-repeat.rs"
    cmp "${fixture_work}/flowlog-core.rs" "${fixture_work}/miniflow-core.rs"
    strict=$((strict + 1))
done <"${MANIFEST}"

printf 'FlowLog batch canonical-core parity: %s strict fixtures passed\n' \
    "${strict}"
