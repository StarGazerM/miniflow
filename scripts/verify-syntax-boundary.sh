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
    echo "miniflow-core unexpectedly depends on its proc-macro wrapper" >&2
    exit 1
fi
if ! rg -q 'impl Parse for|custom_keyword!' \
    "${ROOT_DIR}/crates/miniflow-core/src/syntax.rs"; then
    echo "miniflow-core no longer owns the default syntax parser" >&2
    exit 1
fi
if rg -q 'impl Parse for|custom_keyword!' \
    "${ROOT_DIR}/crates/miniflow-macro/src"; then
    echo "miniflow-macro unexpectedly contains a second syntax parser" >&2
    exit 1
fi

runtime_tree="$(cargo tree -p miniflow -e normal)"
if grep -Eq 'miniflow-core|miniflow-macro' <<<"${runtime_tree}"; then
    echo "miniflow runtime unexpectedly selects a compiler or syntax" >&2
    exit 1
fi

macro_tree="$(cargo tree -p miniflow-macro -e normal)"
grep -q 'miniflow-core' <<<"${macro_tree}"

cargo check -p miniflow
echo "Runtime, compiler-core/default-frontend, and macro boundaries: passed"
