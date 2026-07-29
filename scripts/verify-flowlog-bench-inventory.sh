#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly UPSTREAM="${FLOWLOG_BENCH_SOURCE:-"${ROOT_DIR}/flowlog-bench"}"
readonly CORPUS="${ROOT_DIR}/corpus/flowlog-bench"
readonly PROGRAMS="${CORPUS}/programs.tsv"
readonly CONFIGS="${CORPUS}/configs.tsv"
readonly VARIANTS="${CORPUS}/variants.tsv"
readonly ALIASES="${CORPUS}/aliases.tsv"
readonly PINNED_COMMIT="$(<"${CORPUS}/UPSTREAM_COMMIT")"

if [[ ! -d "${UPSTREAM}/.git" ]]; then
    echo "flowlog-bench checkout not found at ${UPSTREAM}" >&2
    exit 1
fi

actual_commit="$(git -C "${UPSTREAM}" rev-parse HEAD)"
if [[ "${actual_commit}" != "${PINNED_COMMIT}" ]]; then
    echo "flowlog-bench revision mismatch" >&2
    echo "expected: ${PINNED_COMMIT}" >&2
    echo "actual:   ${actual_commit}" >&2
    exit 1
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-flowlog-bench-inventory.XXXXXX")"
trap 'rm -rf "${work_dir}"' EXIT

awk -F '\t' '
    BEGIN { ok = 1 }
    /^#/ || !NF { next }
    NF != 3 {
        printf "program manifest line %d must have three fields\n", NR > "/dev/stderr"
        ok = 0
        next
    }
    $2 != "canonical" && $2 != "configured-alias" && $2 != "ldbc" {
        printf "program manifest line %d has invalid upstream kind %s\n", NR, $2 > "/dev/stderr"
        ok = 0
    }
    $3 != "ascent-include" &&
        $3 != "local-recursive-aggregate" &&
        $3 != "local-ldbc" {
        printf "program manifest line %d has invalid implementation %s\n", NR, $3 > "/dev/stderr"
        ok = 0
    }
    seen[$1]++ {
        printf "program manifest line %d duplicates %s\n", NR, $1 > "/dev/stderr"
        ok = 0
    }
    { print $1 }
    END { exit !ok }
' "${PROGRAMS}" | LC_ALL=C sort >"${work_dir}/declared-programs"

find "${CORPUS}/src/bin" -maxdepth 1 -type f -name '*.rs' -printf '%f\n' \
    | sed 's/\.rs$//' | LC_ALL=C sort >"${work_dir}/local-programs"
diff -u "${work_dir}/declared-programs" "${work_dir}/local-programs"

find "${UPSTREAM}/programs/oracle/flowlog" \
    -mindepth 2 -maxdepth 2 -type f -name default.dl -printf '%h\n' \
    | sed 's#.*/##' | LC_ALL=C sort >"${work_dir}/upstream-canonical"
awk -F '\t' '$2 == "canonical" {print $1}' "${PROGRAMS}" \
    | LC_ALL=C sort >"${work_dir}/declared-canonical"
diff -u "${work_dir}/upstream-canonical" "${work_dir}/declared-canonical"

find "${UPSTREAM}/programs/oracle/ascent" \
    -mindepth 2 -maxdepth 2 -type f -name Cargo.toml -printf '%h\n' \
    | sed 's#.*/##' | grep -vx harness | LC_ALL=C sort \
    >"${work_dir}/upstream-ascent"
awk -F '\t' '$2 == "canonical" || $2 == "configured-alias" {print $1}' \
    "${PROGRAMS}" | LC_ALL=C sort >"${work_dir}/declared-ascent"
diff -u "${work_dir}/upstream-ascent" "${work_dir}/declared-ascent"

find "${UPSTREAM}/programs/ldbc/flowlog" \
    -maxdepth 1 -type f -name '*.dl' -printf '%f\n' \
    | sed 's/\.dl$//' | LC_ALL=C sort >"${work_dir}/upstream-ldbc"
awk -F '\t' '$2 == "ldbc" {print $1}' "${PROGRAMS}" \
    | LC_ALL=C sort >"${work_dir}/declared-ldbc"
diff -u "${work_dir}/upstream-ldbc" "${work_dir}/declared-ldbc"

while IFS=$'\t' read -r program upstream_kind implementation; do
    [[ "${program}" == \#* || -z "${program}" ]] && continue
    local_source="${CORPUS}/src/bin/${program}.rs"
    test -f "${local_source}"
    grep -Fq '#![flowlog_batch]' "${local_source}"
    case "${implementation}" in
        ascent-include)
            test -f "${UPSTREAM}/programs/oracle/ascent/${program}/src/main.rs"
            grep -Fq \
                "flowlog-bench/programs/oracle/ascent/${program}/src/main.rs" \
                "${local_source}"
            ;;
        local-recursive-aggregate)
            test "${program}" = cc -o "${program}" = sssp
            test -f "${UPSTREAM}/programs/oracle/flowlog/${program}/default.dl"
            ;;
        local-ldbc)
            test "${upstream_kind}" = ldbc
            test -f "${UPSTREAM}/programs/ldbc/flowlog/${program}.dl"
            ;;
    esac
done <"${PROGRAMS}"

awk -F '\t' '
    BEGIN { ok = 1 }
    /^#/ || !NF { next }
    NF != 3 || seen[$1]++ {
        printf "invalid alias declaration on line %d\n", NR > "/dev/stderr"
        ok = 0
    }
    { print $1 "\t" $2 "\t" $3 }
    END { exit !ok }
' "${ALIASES}" >"${work_dir}/aliases"
while IFS=$'\t' read -r alias semantic_source reason; do
    test "${reason}" = "upstream-default-config-has-no-flowlog-borrow-directory"
    test ! -e "${UPSTREAM}/programs/oracle/flowlog/${alias}/default.dl"
    test -f "${UPSTREAM}/programs/oracle/ascent/${alias}/src/main.rs"
    test -f "${UPSTREAM}/programs/oracle/flowlog/${semantic_source}/default.dl"
    test "$(awk -F '\t' -v p="${alias}" '$1 == p {print $2}' "${PROGRAMS}")" \
        = configured-alias
done <"${work_dir}/aliases"

# The upstream borrow translation intentionally differs from polonius_int only
# in its first provenance comment. Prove that the configured alias has no
# hidden semantic delta.
tail -n +2 "${UPSTREAM}/programs/oracle/ascent/borrow/src/main.rs" \
    >"${work_dir}/borrow-body"
tail -n +2 "${UPSTREAM}/programs/oracle/ascent/polonius_int/src/main.rs" \
    >"${work_dir}/polonius-int-body"
cmp "${work_dir}/borrow-body" "${work_dir}/polonius-int-body"

awk -F '\t' '
    BEGIN { ok = 1 }
    /^#/ || !NF { next }
    NF != 2 || $2 !~ /^[0-9]+$/ || seen[$1]++ {
        printf "invalid config declaration on line %d\n", NR > "/dev/stderr"
        ok = 0
    }
    END { exit !ok }
' "${CONFIGS}"

configured_pairs=0
while IFS=$'\t' read -r config expected_pairs; do
    [[ "${config}" == \#* || -z "${config}" ]] && continue
    config_path="${UPSTREAM}/config/${config}"
    test -f "${config_path}"
    awk -F= '
        /^[[:space:]]*#/ || !NF { next }
        {
            program = $1
            sub(/^.*\//, "", program)
            sub(/\.dl$/, "", program)
            dataset = $2
            sub(/[[:space:]]+\[.*$/, "", dataset)
            if (!length(program) || !length(dataset)) {
                printf "invalid active config row on line %d\n", NR > "/dev/stderr"
                exit 1
            }
            key = program SUBSEP dataset
            if (seen[key]++) {
                printf "duplicate configured pair %s=%s\n", program, dataset > "/dev/stderr"
                exit 1
            }
            print program "\t" dataset
        }
    ' "${config_path}" >"${work_dir}/${config}.pairs"
    actual_pairs="$(wc -l <"${work_dir}/${config}.pairs")"
    test "${actual_pairs}" -eq "${expected_pairs}"
    configured_pairs=$((configured_pairs + actual_pairs))
    while IFS=$'\t' read -r program _dataset; do
        grep -Fqx "${program}" "${work_dir}/declared-programs"
    done <"${work_dir}/${config}.pairs"
done <"${CONFIGS}"

awk -F '\t' '
    BEGIN { ok = 1 }
    /^#/ || !NF { next }
    NF != 2 || $2 !~ /^[0-9]+$/ || seen[$1]++ {
        printf "invalid variant declaration on line %d\n", NR > "/dev/stderr"
        ok = 0
    }
    { print $1 }
    END { exit !ok }
' "${VARIANTS}" | LC_ALL=C sort >"${work_dir}/declared-variant-programs"
diff -u "${work_dir}/upstream-canonical" "${work_dir}/declared-variant-programs"

variant_total=0
while IFS=$'\t' read -r program expected_count; do
    [[ "${program}" == \#* || -z "${program}" ]] && continue
    program_dir="${UPSTREAM}/programs/oracle/flowlog/${program}"
    manifest="${program_dir}/manifest.csv"
    find "${program_dir}" -maxdepth 1 -type f -name '*.dl' -printf '%f\n' \
        | LC_ALL=C sort >"${work_dir}/${program}.dl-files"
    if [[ -f "${manifest}" ]]; then
        tail -n +2 "${manifest}" | cut -d, -f1 | LC_ALL=C sort \
            >"${work_dir}/${program}.manifest-files"
        diff -u \
            "${work_dir}/${program}.dl-files" \
            "${work_dir}/${program}.manifest-files"
    else
        # Upstream has one legacy default-only directory without the generated
        # join-order manifest. Do not generalize this exception.
        test "${program}" = polonius_int
        test "${expected_count}" -eq 1
    fi
    actual_count="$(wc -l <"${work_dir}/${program}.dl-files")"
    test "${actual_count}" -eq "${expected_count}"
    grep -Fqx default.dl "${work_dir}/${program}.dl-files"
    variant_total=$((variant_total + actual_count))
done <"${VARIANTS}"
test "${variant_total}" -eq 878
"${ROOT_DIR}/scripts/verify-flowlog-bench-variants.py" "${UPSTREAM}"

printf \
    'flowlog-bench inventory: 19 canonical + 1 configured alias + 2 LDBC programs; %s configured pairs; %s join-order files accounted\n' \
    "${configured_pairs}" "${variant_total}"
