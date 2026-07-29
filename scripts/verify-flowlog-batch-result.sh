#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly FIXTURES="${ROOT_DIR}/flowlog/tests/fixtures/datalog-batch"
readonly MANIFEST="${ROOT_DIR}/corpus/flowlog-batch/manifest.tsv"
readonly RUNNER="${ROOT_DIR}/target/debug/miniflow-flowlog-batch-corpus"

cargo build \
    --manifest-path "${ROOT_DIR}/Cargo.toml" \
    -p miniflow-flowlog-batch-corpus \
    --bin miniflow-flowlog-batch-corpus

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-batch-result.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

completed=0
while IFS=$'\t' read -r fixture _family status; do
    [[ "${fixture}" == \#* || -z "${fixture}" || "${status}" == "pending" ]] && continue
    fixture_dir="${FIXTURES}/${fixture}"
    output_dir="${work_dir}/${fixture}/output"
    mkdir -p "${output_dir}"
    "${RUNNER}" "${fixture}" "${fixture_dir}" "${output_dir}"

    find "${fixture_dir}/expected" -maxdepth 1 -type f -printf '%f\n' \
        | LC_ALL=C sort >"${work_dir}/${fixture}.expected-files"
    find "${output_dir}" -maxdepth 1 -type f -printf '%f\n' \
        | LC_ALL=C sort >"${work_dir}/${fixture}.actual-files"
    diff -u \
        "${work_dir}/${fixture}.expected-files" \
        "${work_dir}/${fixture}.actual-files"

    while IFS= read -r output_file; do
        if [[ -f "${fixture_dir}/runtime_flags" ]] \
            && grep -q -- '-w' "${fixture_dir}/runtime_flags"; then
            # Match FlowLog's own fixture runner: timely worker scheduling does
            # not define row order, even for `-w 1`.
            diff -u \
                <(LC_ALL=C sort "${fixture_dir}/expected/${output_file}") \
                <(LC_ALL=C sort "${output_dir}/${output_file}")
        else
            cmp \
                "${fixture_dir}/expected/${output_file}" \
                "${output_dir}/${output_file}"
        fi
    done <"${work_dir}/${fixture}.expected-files"
    completed=$((completed + 1))
done <"${MANIFEST}"

printf 'FlowLog batch expected-output parity: %s completed fixtures passed\n' \
    "${completed}"
