#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly FLOWLOG_DIR="${ROOT_DIR}/flowlog"
readonly EXPECTED_COMMIT="$(<"${ROOT_DIR}/parity/flowlog/UPSTREAM_COMMIT")"
readonly EXPECTED_PATCH="${ROOT_DIR}/parity/flowlog/oracle.patch"
readonly PATCHED_FILES=(
    "flowlog-build/src/codegen/flow/non_recursive.rs"
    "flowlog-build/src/codegen/flow/recursive.rs"
    "flowlog-build/src/planner/stratum_planner.rs"
)

actual_commit="$(git -C "${FLOWLOG_DIR}" rev-parse HEAD)"
if [[ "${actual_commit}" != "${EXPECTED_COMMIT}" ]]; then
    echo "FlowLog oracle revision mismatch" >&2
    echo "expected: ${EXPECTED_COMMIT}" >&2
    echo "actual:   ${actual_commit}" >&2
    exit 1
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-oracle.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

git -C "${FLOWLOG_DIR}" diff --no-ext-diff --binary -- "${PATCHED_FILES[@]}" \
    >"${work_dir}/actual.patch"
cmp "${EXPECTED_PATCH}" "${work_dir}/actual.patch"

git -C "${FLOWLOG_DIR}" status --short --untracked-files=all \
    >"${work_dir}/actual.status"
for patched_file in "${PATCHED_FILES[@]}"; do
    printf ' M %s\n' "${patched_file}"
done >"${work_dir}/expected.status"
cmp "${work_dir}/expected.status" "${work_dir}/actual.status"

echo "FlowLog oracle revision and deterministic-order patch: passed"
