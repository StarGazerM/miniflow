#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/cargo-env.sh"

cargo clean \
    --manifest-path "${ROOT_DIR}/Cargo.toml" \
    --target-dir "${CARGO_TARGET_DIR}"

editor_target="${TMPDIR:-/tmp}/miniflow-rust-analyzer-target"
if [[ -d "${editor_target}" ]]; then
    cargo clean \
        --manifest-path "${ROOT_DIR}/Cargo.toml" \
        --target-dir "${editor_target}"
fi

for legacy_target in "${ROOT_DIR}/target" "${ROOT_DIR}/flowlog/target"; do
    case "${legacy_target}" in
        "${ROOT_DIR}/target" | "${ROOT_DIR}/flowlog/target")
            rm -rf -- "${legacy_target}"
            ;;
        *)
            echo "refusing unexpected legacy target: ${legacy_target}" >&2
            exit 1
            ;;
    esac
done

printf 'Removed disposable and legacy MiniFlow build artifacts\n'
