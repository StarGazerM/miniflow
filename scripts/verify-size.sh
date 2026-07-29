#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v tokei >/dev/null; then
    echo "tokei is required for the production-source comparison" >&2
    exit 1
fi

tokei_stats() {
    LC_ALL=C tokei --compact --types Rust "$@" |
        awk '
            $1 == "Total" {
                print $2, $3, $4, $5, $6
                found = 1
            }
            END {
                if (!found) {
                    exit 1
                }
            }
        '
}

read -r miniflow_files miniflow_lines miniflow_code \
    miniflow_comments miniflow_blanks <<<"$(tokei_stats \
    "${ROOT_DIR}/crates/ascent-flow/src" \
    "${ROOT_DIR}/crates/miniflow-core/src" \
    "${ROOT_DIR}/crates/miniflow-macro/src" \
    "${ROOT_DIR}/crates/miniflow/src")"
read -r flowlog_files flowlog_lines flowlog_code \
    flowlog_comments flowlog_blanks <<<"$(tokei_stats \
    "${ROOT_DIR}/flowlog/flowlog-build/src" \
    "${ROOT_DIR}/flowlog/flowlog-parser/src" \
    "${ROOT_DIR}/flowlog/flowlog-runtime/src" \
    "${ROOT_DIR}/flowlog/flowlog-compiler/src")"

test "${miniflow_lines}" -le 6500
test "$((miniflow_lines * 5))" -lt "${flowlog_lines}"
test "${miniflow_code}" -le 6200
test "$((miniflow_code * 4))" -lt "${flowlog_code}"

comparison="$(printf '%s\n' \
    '<!-- BEGIN TOKEI COMPARISON -->' \
    '| Production Rust | Files | Lines | Code | Comments | Blanks |' \
    '|---|---:|---:|---:|---:|---:|' \
    "| MiniFlow + AscentFlow | ${miniflow_files} | ${miniflow_lines} | ${miniflow_code} | ${miniflow_comments} | ${miniflow_blanks} |" \
    "| FlowLog batch stack | ${flowlog_files} | ${flowlog_lines} | ${flowlog_code} | ${flowlog_comments} | ${flowlog_blanks} |" \
    '<!-- END TOKEI COMPARISON -->')"
documented="$(sed -n \
    '/<!-- BEGIN TOKEI COMPARISON -->/,/<!-- END TOKEI COMPARISON -->/p' \
    "${ROOT_DIR}/README.md")"

if [[ "${documented}" != "${comparison}" ]]; then
    echo "README Tokei comparison is stale; expected:" >&2
    printf '%s\n' "${comparison}" >&2
    exit 1
fi

printf '%s\n' "${comparison}"
