#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly UPSTREAM="${FLOWLOG_BENCH_SOURCE:-"${ROOT_DIR}/flowlog-bench"}"
readonly FLOWLOG_DIR="${ROOT_DIR}/flowlog"
readonly FLOWLOG_COMPILER="${FLOWLOG_COMPILER:-"${FLOWLOG_DIR}/target/release/flowlog-compiler"}"
readonly FACT_DIR="${FACT_DIR:?set FACT_DIR to the populated flowlog-bench facts directory}"
readonly WORKERS="${WORKERS:-1}"

if (( $# )); then
    configs=("$@")
else
    configs=(default.txt doop_intensive.txt joinorder.txt)
fi

"${ROOT_DIR}/scripts/verify-flowlog-bench-inventory.sh"
cargo build \
    --manifest-path "${ROOT_DIR}/Cargo.toml" \
    --release \
    -p miniflow-flowlog-bench-corpus \
    --bins
cargo build \
    --manifest-path "${FLOWLOG_DIR}/Cargo.toml" \
    --release \
    -p flowlog-compiler

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-bench-crosscheck.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT
export FLOWLOG_RUNTIME_PATH="${FLOWLOG_DIR}/flowlog-runtime"

extract_sizes() {
    grep -E $'^[A-Za-z_][A-Za-z0-9_]*\t[0-9]+$' "$1" \
        | tr '[:upper:]' '[:lower:]' \
        | LC_ALL=C sort -u
}

pair_count=0
for config in "${configs[@]}"; do
    config_path="${config}"
    [[ "${config_path}" = /* ]] || config_path="${UPSTREAM}/config/${config_path}"
    test -f "${config_path}"
    while IFS=$'\t' read -r program dataset; do
        [[ -n "${program}" ]] || continue
        if [[ "${program}" == interactive-complex-* ]]; then
            echo "LDBC rows require scripts/crosscheck-flowlog-bench-ldbc.sh" >&2
            exit 1
        fi
        facts="${FACT_DIR}/${dataset}"
        test -d "${facts}" || {
            echo "missing configured dataset: ${facts}" >&2
            exit 1
        }
        semantic_source="$(
            awk -F '\t' -v p="${program}" '$1 == p {print $2}' \
                "${ROOT_DIR}/corpus/flowlog-bench/aliases.tsv"
        )"
        semantic_source="${semantic_source:-${program}}"
        oracle_source="${UPSTREAM}/programs/oracle/flowlog/${semantic_source}/default.dl"
        local_binary="${ROOT_DIR}/target/release/${program}"
        test -f "${oracle_source}"
        test -x "${local_binary}"

        pair="${program}-${dataset}"
        oracle_binary="${work_dir}/${pair}-flowlog"
        oracle_log="${work_dir}/${pair}-flowlog.log"
        local_log="${work_dir}/${pair}-miniflow.log"
        "${FLOWLOG_COMPILER}" \
            "${oracle_source}" \
            -F "${facts}" \
            -D - \
            -o "${oracle_binary}" \
            --mode datalog-batch \
            --str-intern \
            >/dev/null
        "${oracle_binary}" -w "${WORKERS}" >"${oracle_log}"
        WORKERS="${WORKERS}" "${local_binary}" "${facts}" >"${local_log}"
        extract_sizes "${oracle_log}" >"${work_dir}/${pair}-flowlog.sizes"
        extract_sizes "${local_log}" >"${work_dir}/${pair}-miniflow.sizes"
        diff -u \
            "${work_dir}/${pair}-flowlog.sizes" \
            "${work_dir}/${pair}-miniflow.sizes"
        pair_count=$((pair_count + 1))
        printf 'PASS %s + %s\n' "${program}" "${dataset}"
    done < <(
        awk -F= '
            /^[[:space:]]*#/ || !NF { next }
            {
                program = $1
                sub(/^.*\//, "", program)
                sub(/\.dl$/, "", program)
                dataset = $2
                sub(/[[:space:]]+\[.*$/, "", dataset)
                print program "\t" dataset
            }
        ' "${config_path}"
    )
done

printf 'FlowLog/MiniFlow benchmark size parity: %s configured rows passed\n' \
    "${pair_count}"
