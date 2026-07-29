#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly VERIFY_TARGET="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-verify-target.XXXXXX")"
trap 'rm -rf "${VERIFY_TARGET}"' EXIT
export CARGO_TARGET_DIR="${VERIFY_TARGET}"
source "${ROOT_DIR}/scripts/cargo-env.sh"

cd "${ROOT_DIR}"
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
scripts/verify-ascent-inventory.sh
scripts/verify-flowlog-inventory.sh
scripts/verify-flowlog-batch-inventory.sh
scripts/verify-flowlog-bench-inventory.sh
scripts/verify-miniflow-syntax.sh
scripts/verify-frontend-isolation.sh
scripts/verify-size.sh
scripts/verify-flowlog-batch-result.sh
scripts/verify-flowlog-batch-expansion.sh
scripts/verify-flowlog-result.sh
scripts/verify-flowlog-expansion.sh
