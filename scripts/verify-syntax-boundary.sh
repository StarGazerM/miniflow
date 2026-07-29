#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if rg -q '<--|ascent[_-]flow|ascent_par|mod ascent[[:space:]]*\{' \
    "${ROOT_DIR}/crates" \
    "${ROOT_DIR}/corpus" \
    --glob '*.rs' \
    --glob 'Cargo.toml'; then
    echo "MiniFlow workspace unexpectedly contains an Ascent-facing syntax path" >&2
    exit 1
fi

core_tree="$(cargo tree -p miniflow-core -e normal)"
if grep -q 'miniflow-macro' <<<"${core_tree}"; then
    echo "miniflow-core unexpectedly depends on the macro driver" >&2
    exit 1
fi
if rg -q 'impl Parse for|custom_keyword!' \
    "${ROOT_DIR}/crates/miniflow-core/src/source.rs"; then
    echo "miniflow-core source model unexpectedly contains a token parser" >&2
    exit 1
fi
if rg -q 'Compiler::new|MiniFlowSyntax|pub fn parse\(' \
    "${ROOT_DIR}/crates/miniflow-core/src/lib.rs" \
    "${ROOT_DIR}/crates/miniflow-core/src/pipeline.rs"; then
    echo "miniflow-core unexpectedly selects a default grammar" >&2
    exit 1
fi

runtime_tree="$(cargo tree -p miniflow -e normal)"
if grep -Eq 'miniflow-core|miniflow-macro' <<<"${runtime_tree}"; then
    echo "miniflow runtime unexpectedly selects a compiler or syntax" >&2
    exit 1
fi

driver_tree="$(cargo tree -p miniflow-macro -e features)"
grep -q 'miniflow-core' <<<"${driver_tree}"

cargo check -p miniflow
echo "Runtime, compiler-kernel, and macro-driver boundaries: passed"
