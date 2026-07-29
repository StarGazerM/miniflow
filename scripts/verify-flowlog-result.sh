#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly FLOWLOG_DIR="${ROOT_DIR}/flowlog"
readonly MANIFEST="${ROOT_DIR}/parity/flowlog/manifest.tsv"
readonly COMPILER="${FLOWLOG_DIR}/target/debug/flowlog-compiler"

"${ROOT_DIR}/scripts/verify-flowlog-oracle.sh"

cargo build \
    --manifest-path "${FLOWLOG_DIR}/Cargo.toml" \
    -p flowlog-compiler

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-parity.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

export FLOWLOG_RUNTIME_PATH="${FLOWLOG_DIR}/flowlog-runtime"
while IFS=$'\t' read -r fixture output_file _canonical_example runtime_example mode; do
    fixture_dir="${ROOT_DIR}/parity/flowlog/${fixture}"
    fixture_work="${work_dir}/${fixture}"
    flowlog_run="${fixture_work}/flowlog-run"
    miniflow_run="${fixture_work}/miniflow-run"
    mkdir -p "${flowlog_run}/output" "${miniflow_run}"

    compiler_flags=()
    if [[ "${mode}" == "profile" ]]; then
        compiler_flags+=(--profile)
    elif [[ "${mode}" != "plain" ]]; then
        echo "unknown FlowLog parity mode: ${mode}" >&2
        exit 1
    fi
    "${COMPILER}" \
        "${compiler_flags[@]}" \
        -D output \
        "${fixture_dir}/program.dl" \
        -o "${fixture_work}/flowlog-program"

    find "${fixture_dir}" -maxdepth 1 -type f -name '*.csv' \
        -exec cp '{}' "${flowlog_run}" ';'
    find "${fixture_dir}" -maxdepth 1 -type f -name '*.csv' \
        -exec cp '{}' "${miniflow_run}" ';'

    (
        cd "${flowlog_run}"
        "${fixture_work}/flowlog-program" >/dev/null
    )
    (
        cd "${miniflow_run}"
        cargo run \
            --quiet \
            --manifest-path "${ROOT_DIR}/Cargo.toml" \
            -p miniflow-macro \
            --example "${runtime_example}"
    ) >"${miniflow_run}/output.csv"

    LC_ALL=C sort \
        "${flowlog_run}/output/${output_file}" \
        >"${flowlog_run}/output.sorted.csv"
    LC_ALL=C sort \
        "${miniflow_run}/output.csv" \
        >"${miniflow_run}/output.sorted.csv"
    diff -u \
        "${flowlog_run}/output.sorted.csv" \
        "${miniflow_run}/output.sorted.csv"

    if [[ "${mode}" == "profile" ]]; then
        flowlog_metrics="${flowlog_run}/program_log/metrics/operators_worker_t0_0.log"
        miniflow_metrics="${miniflow_run}/program_log/metrics/operators_worker_t0_0.log"
        test -s "${flowlog_run}/program_log/ops.json"
        test -s "${miniflow_run}/program_log/ops.json"
        test "$(wc -l <"${flowlog_metrics}")" -gt 1
        test "$(wc -l <"${miniflow_metrics}")" -gt 1
        head -n 1 "${flowlog_metrics}" >"${fixture_work}/flowlog-metrics-header"
        head -n 1 "${miniflow_metrics}" >"${fixture_work}/miniflow-metrics-header"
        cmp \
            "${fixture_work}/flowlog-metrics-header" \
            "${fixture_work}/miniflow-metrics-header"
    fi
done <"${MANIFEST}"

echo "FlowLog/MiniFlow result and profiling parity: all fixtures passed"
