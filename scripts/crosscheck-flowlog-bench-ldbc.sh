#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly UPSTREAM="${FLOWLOG_BENCH_SOURCE:-"${ROOT_DIR}/flowlog-bench"}"
readonly FLOWLOG_DIR="${ROOT_DIR}/flowlog"
readonly FLOWLOG_COMPILER="${FLOWLOG_COMPILER:-"${FLOWLOG_DIR}/target/release/flowlog-compiler"}"
readonly FACT_DIR="${FACT_DIR:?set FACT_DIR to the populated LDBC dataset directory}"
readonly CONFIG="${1:-"${UPSTREAM}/config/ldbc.txt"}"
readonly WORKERS="${WORKERS:-1}"
readonly MAX_PARAMS="${MAX_PARAMS:-0}"

"${ROOT_DIR}/scripts/verify-flowlog-bench-inventory.sh"
cargo build \
    --manifest-path "${ROOT_DIR}/Cargo.toml" \
    --release \
    -p miniflow-flowlog-bench-corpus \
    --bin interactive-complex-2 \
    --bin interactive-complex-13
cargo build \
    --manifest-path "${FLOWLOG_DIR}/Cargo.toml" \
    --release \
    -p flowlog-compiler

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-bench-ldbc.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT
export FLOWLOG_RUNTIME_PATH="${FLOWLOG_DIR}/flowlog-runtime"

compare_rows() {
    python3 - "$1" "$2" <<'PY'
import csv
import sys

def rows(path):
    with open(path, newline="") as source:
        return {
            tuple(field.rstrip() for field in row)
            for row in csv.reader(source, delimiter="|")
            if row
        }

flowlog = rows(sys.argv[1])
miniflow = rows(sys.argv[2])
if flowlog != miniflow:
    only_flowlog = sorted(flowlog - miniflow)[:5]
    only_miniflow = sorted(miniflow - flowlog)[:5]
    raise SystemExit(
        f"row mismatch: only FlowLog={only_flowlog}; only MiniFlow={only_miniflow}"
    )
PY
}

query_count=0
parameter_count=0
while IFS=$'\t' read -r query dataset; do
    [[ -n "${query}" ]] || continue
    source_facts="${FACT_DIR}/${dataset}"
    test -d "${source_facts}" || {
        echo "missing configured dataset: ${source_facts}" >&2
        exit 1
    }
    oracle_source="${UPSTREAM}/programs/ldbc/flowlog/${query}.dl"
    local_binary="${ROOT_DIR}/target/release/${query}"
    test -f "${oracle_source}"
    test -x "${local_binary}"
    parameter_file="$(
        grep 'filename=' "${oracle_source}" \
            | grep -i param \
            | head -1 \
            | sed 's/.*filename="\([^"]*\)".*/\1/'
    )"
    test -n "${parameter_file}"
    test -f "${source_facts}/${parameter_file}"

    query_work="${work_dir}/${query}"
    facts_view="${query_work}/facts"
    oracle_output="${query_work}/flowlog-output"
    mkdir -p "${facts_view}" "${oracle_output}"
    find "${source_facts}" -maxdepth 1 -type f ! -name "${parameter_file}" \
        -exec ln -s '{}' "${facts_view}" ';'
    header="$(head -1 "${source_facts}/${parameter_file}")"
    mapfile -t parameter_rows < <(
        tail -n +2 "${source_facts}/${parameter_file}" | grep -v '^$'
    )
    if (( MAX_PARAMS > 0 && MAX_PARAMS < ${#parameter_rows[@]} )); then
        parameter_rows=("${parameter_rows[@]:0:MAX_PARAMS}")
    fi

    oracle_binary="${query_work}/flowlog-program"
    "${FLOWLOG_COMPILER}" \
        "${oracle_source}" \
        -F "${facts_view}" \
        -D "${oracle_output}" \
        -o "${oracle_binary}" \
        --mode datalog-batch \
        --str-intern \
        >/dev/null

    index=0
    for parameter_row in "${parameter_rows[@]}"; do
        index=$((index + 1))
        printf '%s\n%s\n' "${header}" "${parameter_row}" \
            >"${facts_view}/${parameter_file}"
        find "${oracle_output}" -maxdepth 1 -type f -delete
        local_output="${query_work}/miniflow-output-${index}"
        "${oracle_binary}" -w "${WORKERS}" >/dev/null
        WORKERS="${WORKERS}" \
            "${local_binary}" "${facts_view}" "${local_output}" >/dev/null
        find "${oracle_output}" -maxdepth 1 -type f -exec cat '{}' + \
            >"${query_work}/flowlog-${index}.rows"
        find "${local_output}" -maxdepth 1 -type f -exec cat '{}' + \
            >"${query_work}/miniflow-${index}.rows"
        compare_rows \
            "${query_work}/flowlog-${index}.rows" \
            "${query_work}/miniflow-${index}.rows"
        parameter_count=$((parameter_count + 1))
    done
    query_count=$((query_count + 1))
    printf 'PASS %s + %s (%s parameters)\n' \
        "${query}" "${dataset}" "${#parameter_rows[@]}"
done < <(
    awk -F= '
        /^[[:space:]]*#/ || !NF { next }
        {
            dataset = $2
            sub(/[[:space:]]+\[.*$/, "", dataset)
            print $1 "\t" dataset
        }
    ' "${CONFIG}"
)

printf \
    'FlowLog/MiniFlow LDBC row-set parity: %s queries and %s parameter rows passed\n' \
    "${query_count}" "${parameter_count}"
