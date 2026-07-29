#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

mapfile -t sources < <(
    cd "${ROOT_DIR}"
    rg -l '(^|[^[:alnum:]_])miniflow(::miniflow)?!' \
        --glob '*.rs' crates corpus parity examples 2>/dev/null |
        LC_ALL=C sort
)

test "${#sources[@]}" -gt 0

for source in "${sources[@]}"; do
    if rg -n '(<--|^[[:space:]]*relation[[:space:]])' "${ROOT_DIR}/${source}"; then
        echo "${source}: miniflow! must use FlowLog syntax (.decl and :-)" >&2
        exit 1
    fi
done

echo "MiniFlow syntax inventory: ${#sources[@]} Rust sources use FlowLog spelling"
