#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ASCENT_DIR="${ASCENT_SOURCE:-"${ROOT_DIR}/../ascent"}"
readonly PINNED_COMMIT="$(<"${ROOT_DIR}/corpus/ascent/UPSTREAM_COMMIT")"

if [[ ! -d "${ASCENT_DIR}/.git" ]]; then
    echo "Ascent checkout not found at ${ASCENT_DIR}; set ASCENT_SOURCE" >&2
    exit 1
fi

actual_commit="$(git -C "${ASCENT_DIR}" rev-parse HEAD)"
if [[ "${actual_commit}" != "${PINNED_COMMIT}" ]]; then
    echo "Ascent oracle revision mismatch" >&2
    echo "expected: ${PINNED_COMMIT}" >&2
    echo "actual:   ${actual_commit}" >&2
    exit 1
fi

temporary="$(mktemp -d "${TMPDIR:-/tmp}/miniflow-ascent-inventory.XXXXXX")"
trap 'rm -rf "${temporary}"' EXIT

git -C "${ASCENT_DIR}" ls-files \
    'ascent/examples/*.rs' \
    'ascent_tests/benches/*.rs' \
    'ascent_tests/src/*.rs' \
    'ascent_tests/src/**/*.rs' \
    | LC_ALL=C sort >"${temporary}/upstream"

awk -F '\t' '!/^#/ && NF {print $1}' \
    "${ROOT_DIR}/corpus/ascent/manifest.tsv" \
    | LC_ALL=C sort >"${temporary}/manifest"

diff -u "${temporary}/upstream" "${temporary}/manifest"
echo "Ascent corpus inventory parity: 32 paths passed"
