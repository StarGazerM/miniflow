#!/usr/bin/env bash
set -euo pipefail

readonly ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${ROOT_DIR}"

miniflow_tree="$(cargo tree -p miniflow --edges normal)"
ascent_flow_tree="$(cargo tree -p ascent-flow --edges normal)"

if grep -Eq '(^|[[:space:]])ascent-flow(-macro|-syntax)? v' <<<"${miniflow_tree}"; then
    echo "miniflow's normal dependency graph contains an AscentFlow frontend crate" >&2
    exit 1
fi

if grep -Eq '(^|[[:space:]])miniflow(-macro|-syntax)? v' <<<"${ascent_flow_tree}"; then
    echo "ascent-flow's normal dependency graph contains the MiniFlow frontend" >&2
    exit 1
fi

grep -Fq 'miniflow-core v' <<<"${miniflow_tree}"
grep -Fq 'miniflow-runtime v' <<<"${miniflow_tree}"
grep -Fq 'miniflow-core v' <<<"${ascent_flow_tree}"
grep -Fq 'miniflow-runtime v' <<<"${ascent_flow_tree}"

echo "Frontend dependency isolation: MiniFlow and AscentFlow share only core/runtime"
