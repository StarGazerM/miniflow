#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly FIXTURES="${ROOT_DIR}/flowlog/tests/fixtures/datalog-batch"
readonly CORPUS="${ROOT_DIR}/corpus/flowlog-batch"
readonly MANIFEST="${CORPUS}/manifest.tsv"
readonly ADAPTERS="${CORPUS}/adapters.tsv"

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-batch-inventory.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

find "${FIXTURES}" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' \
    | LC_ALL=C sort >"${work_dir}/upstream"
awk -F '\t' '!/^#/ && NF {print $1}' "${MANIFEST}" \
    | LC_ALL=C sort >"${work_dir}/manifest"
diff -u "${work_dir}/upstream" "${work_dir}/manifest"

awk -F '\t' '
    BEGIN { ok = 1 }
    /^#/ || !NF { next }
    NF != 3 {
        printf "manifest line %d must have three tab-separated fields\n", NR > "/dev/stderr"
        ok = 0
        next
    }
    $3 != "pending" && $3 != "strict" && $3 != "adapter" {
        printf "manifest line %d has invalid status %s\n", NR, $3 > "/dev/stderr"
        ok = 0
    }
    seen[$1]++ {
        printf "manifest line %d duplicates fixture %s\n", NR, $1 > "/dev/stderr"
        ok = 0
    }
    END { exit !ok }
' "${MANIFEST}"

awk -F '\t' '$3 == "adapter" {print $1}' "${MANIFEST}" | LC_ALL=C sort \
    >"${work_dir}/manifest-adapters"
awk -F '\t' '
    BEGIN { ok = 1 }
    /^#/ || !NF { next }
    NF != 2 || $2 != "str-intern-representation" || seen[$1]++ {
        printf "invalid adapter declaration on line %d\n", NR > "/dev/stderr"
        ok = 0
    }
    { print $1 }
    END { exit !ok }
' "${ADAPTERS}" | LC_ALL=C sort >"${work_dir}/declared-adapters"
diff -u "${work_dir}/manifest-adapters" "${work_dir}/declared-adapters"

while IFS=$'\t' read -r fixture _family status; do
    [[ "${fixture}" == \#* || -z "${fixture}" ]] && continue
    test -f "${FIXTURES}/${fixture}/program.dl"
    if [[ "${status}" != "pending" ]]; then
        test -f "${CORPUS}/src/${fixture}.rs"
    fi
done <"${MANIFEST}"

while IFS=$'\t' read -r fixture reason; do
    [[ "${fixture}" == \#* || -z "${fixture}" ]] && continue
    test "${reason}" = "str-intern-representation"
    test "$(sed '/^[[:space:]]*#/d; /^[[:space:]]*$/d' \
        "${FIXTURES}/${fixture}/compile_flags")" = "--str-intern"
done <"${ADAPTERS}"

total="$(awk -F '\t' '!/^#/ && NF {n++} END {print n}' "${MANIFEST}")"
pending="$(awk -F '\t' '$3 == "pending" {n++} END {print n + 0}' "${MANIFEST}")"
if [[ "${pending}" -ne 0 ]]; then
    printf 'FlowLog batch inventory has %s pending fixtures\n' "${pending}" >&2
    exit 1
fi
printf 'FlowLog batch inventory: all %s fixtures covered\n' "${total}"
