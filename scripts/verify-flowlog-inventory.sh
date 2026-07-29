#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly PARITY_DIR="${ROOT_DIR}/parity/flowlog"
readonly MANIFEST="${PARITY_DIR}/manifest.tsv"

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-inventory.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

cut -f1 "${MANIFEST}" | LC_ALL=C sort >"${work_dir}/manifest-fixtures"
find "${PARITY_DIR}" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' \
    | LC_ALL=C sort >"${work_dir}/fixture-directories"
diff -u "${work_dir}/fixture-directories" "${work_dir}/manifest-fixtures"

while IFS=$'\t' read -r fixture _output canonical_example runtime_example mode; do
    test -f "${PARITY_DIR}/${fixture}/program.dl"
    test -f \
        "${ROOT_DIR}/crates/miniflow-macro/examples/${canonical_example}.rs"
    test -f "${ROOT_DIR}/crates/miniflow-macro/examples/${runtime_example}.rs"
    [[ "${mode}" == "plain" || "${mode}" == "profile" ]]
done <"${MANIFEST}"

echo "FlowLog parity inventory: all fixtures and emitters accounted"
