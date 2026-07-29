#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Keep generated Timely/Differential programs compact throughout the nested
# FlowLog verification scripts. These environment overrides also reach Cargo
# invocations whose generated manifests live outside this workspace.
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_PROFILE_RELEASE_STRIP=symbols
export CARGO_PROFILE_BENCH_DEBUG=0
export CARGO_PROFILE_BENCH_STRIP=symbols

cd "${ROOT_DIR}"
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
scripts/verify-syntax-boundary.sh
scripts/verify-ascent-inventory.sh
scripts/verify-flowlog-inventory.sh
scripts/verify-flowlog-batch-inventory.sh
scripts/verify-flowlog-bench-inventory.sh
scripts/verify-size.sh
scripts/verify-flowlog-batch-result.sh
scripts/verify-flowlog-batch-expansion.sh
scripts/verify-flowlog-result.sh
scripts/verify-flowlog-expansion.sh
