# Compiler compatibility contract

## Scope

MiniFlow implements batch evaluation of finite, typed relational rules.
Recursive evaluation is inferred from the rule dependency graph. Each
recursive strongly connected component is evaluated to a fixed point.

The compiler surface is embedded in Rust:

```rust,ignore
miniflow! {
    pub struct Reach;
    .decl source(id: int32)
    .decl arc(source: int32, target: int32)
    .decl reach(id: int32)

    reach(x) :- source(x).
    reach(y) :- reach(x), arc(x, y).
}
```

The Ascent-shaped surface is exported independently by `ascent-flow`:

```rust,ignore
use ascent_flow::ascent_flow;

ascent_flow! {
    pub struct Reach;

    relation source(i32);
    relation arc(i32, i32);
    relation reach(i32);

    reach(x) <-- source(x);
    reach(y) <-- reach(x), arc(x, y);
}
```

`miniflow-syntax` and `ascent-flow-syntax` are separate parsers. Both produce
the same frontend-neutral AST in `miniflow-core`; the HIR, SCC planner,
optimizer, dataflow emitter, and `miniflow-runtime` are shared. The normal
dependency graph of either public facade excludes the other parser and macro.
Only the absolute runtime-facade path in emitted Rust differs after lowering.

Relation column types and expressions are Rust syntax trees. MiniFlow does not
define an arithmetic AST, numeric coercions, casts, built-in string functions,
or a second type checker.

The shared compiler and evaluator contain:

- typed relation declarations;
- facts and positive relational atoms;
- repeated-variable equality and `_` wildcards;
- multiple rules per relation;
- inferred dependency SCCs and recursive fixed points;
- stratified negation;
- relational `count`, `min`, `max`, `sum`, and `mean` aggregates.

The FlowLog frontend accepts `.decl`, facts, `:-`, comparison predicates,
negated atoms, `.input`, `.output`, `.printsize`, and FlowLog head aggregates.
`.input` is embedded-boundary metadata; host Rust supplies relation contents.
`.output` and `.printsize` select emitted output relations without adding
standalone file/CLI scaffolding.

The Ascent frontend additionally recognizes its native body clauses:
conditions, `if let`, `let`, generators, and body aggregates. These features
cover the pinned Ascent test corpus. All retained examples and benchmarks use
FlowLog-syntax `miniflow!`; syntax-specific compatibility tests use
`ascent_flow!`. Lattice and source-macro examples have relational/Rust
encodings rather than a copied Ascent macro-expansion language.

The following FlowLog product features are outside the core:

- incremental transactions, retractions, and epoch APIs;
- explicit `loop`, `fixpoint`, and `.iterative` surface constructs;
- `.comp`, `.init`, inheritance, and override;
- standalone `.dl` I/O implementations and executable scaffolding;
- a Tokio executor.

Tokio may later wrap a generated engine as an optional host integration, but
Timely owns dataflow worker execution.

Generated program types expose both `run()` and `run_with_workers(usize)`.
`run()` uses the process-local worker count, which defaults to one; the
benchmark harness maps its existing `WORKERS` setting to this count. Inputs
are partitioned exactly once across Timely workers, and output buffers are
shared only at the embedded host boundary.

## One compiler path

The frontend stacks join at one semantic compiler path:

```text
FlowLog tokens -> miniflow-syntax -----\
                                        -> shared AST
Ascent tokens  -> ascent-flow-syntax --/
  -> desugared relational program
  -> HIR
  -> dependency SCCs and strata
  -> DD plan
  -> canonical Rust token stream
```

Each proc macro calls its syntax crate and then this same compiler library.
Expansion tests follow the production FlowLog parser and compiler path. There
is no test-only renderer or separately maintained build-script compiler.

## Exact expansion parity

The existing checkout under `flowlog/` is an oracle, never a production
dependency.

The exact overlap matrix is
[`parity/flowlog/manifest.tsv`](../parity/flowlog/manifest.tsv). It currently
pins these compiler shapes:

- facts, identity copy, projection with `_`, and multi-rule union;
- repeated-variable and `int32` constant selections;
- a two-column less-than condition;
- a two-input equijoin with one shared key;
- unary stratified negation;
- unary transitive reachability through a binary edge relation;
- the same reachability plan with profiling enabled.

Other MiniFlow programs still use the same HIR, SCC scheduler, and DD emitter,
and are covered by result tests. They are not claimed byte-identical to
arbitrary FlowLog programs until a manifest fixture extends the overlap
contract.
The live FlowLog batch corpus has a second, exhaustive matrix at
[`corpus/flowlog-batch/manifest.tsv`](../corpus/flowlog-batch/manifest.tsv).
It accounts for every directory currently present under
`flowlog/tests/fixtures/datalog-batch`:

- all 97 fixtures have a checked-in embedded MiniFlow counterpart;
- all 97 execute against the upstream input files and must reproduce the
  upstream expected file inventory and relation contents; output bytes are
  compared directly except where FlowLog's own fixture runner sorts rows for
  timely `-w` executions;
- 91 `strict` fixtures compare canonical generated DD bytes after compiling
  FlowLog twice from fresh build directories;
- the remaining six `adapter` fixtures are exactly the upstream
  `--str-intern` cases. They retain runtime parity, but the embedded boundary
  deliberately uses Rust `String` rather than FlowLog's internal interner
  representation. [`adapters.tsv`](../corpus/flowlog-batch/adapters.tsv)
  records this one-to-one boundary.

The inventory verifier recomputes the upstream directory set, rejects stale or
duplicate rows, requires a counterpart source file for every row, and fails if
even one row returns to `pending`. It also proves that every `adapter` row has
one declaration and that its upstream compile flags are exactly
`--str-intern`. There is no generic runtime-only escape class.

For each overlap fixture, the parity harness:

1. renders the corresponding `.dl` fixture through the pinned FlowLog batch
   compiler;
2. renders the MiniFlow program through `miniflow-core`;
3. locates the unique `dataflow` closure in each generated Rust syntax tree;
4. removes only host-adapter output sinks (`inspect`/`probe_with`) and the
   standalone closure's final input-handle return expression;
5. canonicalizes the retained closure statements with the same `syn` plus
   `prettyplease` pipeline;
6. compares bytes, without token normalization, snapshots that can be
   overwritten automatically, or wildcard regions;
7. independently executes all 97 embedded counterparts against the upstream
   data and expected-output contract.

Any intentional code-generation change requires changing the pinned oracle
revision or the compatibility contract. It cannot be accepted by refreshing a
golden file.

The oracle is pinned to the `flowlog-compiler-v0.5.0` release tag in
[`parity/flowlog/UPSTREAM_TAG`](../parity/flowlog/UPSTREAM_TAG) and its exact
commit in
[`parity/flowlog/UPSTREAM_COMMIT`](../parity/flowlog/UPSTREAM_COMMIT). The
verifier proves that the tag resolves to the commit. One audited patch gives
statement-producing planner structures canonical fingerprint order:
transformation dependencies, per-IDB unions, recursive metadata and enters,
and feedback assignments. Without it, FlowLog can emit byte-distinct
statement orders from the same input in separate processes. The verifier
checks the exact patch bytes and rejects every other dirty or untracked oracle
file.

The generated input declarations, logical dataflow body, recursive scopes,
arrangements, thresholds, and any in-dataflow profiling operators are inside
the byte comparison. Standalone CSV/CLI plumbing and embedded result drains
are host adapters and are verified through runtime result and profiler-output
tests.

## Complete Ascent corpus accounting

The tracked corpus consists of the 32 Rust sources selected from:

- `ascent/ascent_tests/src/**/*.rs`;
- `ascent/ascent_tests/benches/**/*.rs`;
- `ascent/ascent/examples/**/*.rs`.

Every upstream file has one manifest entry and one local counterpart. Entries
are one of:

- `ascent-result`: relies on host Rust or an Ascent feature that FlowLog cannot
  parse, but must produce the same result under MiniFlow;
- `host-benchmark`: contains benchmark programs and support code retained in
  the corresponding MiniFlow benchmark file;
- `host-support`: registry or experiment support whose associated programs are
  compiled and checked by the corresponding local test module.

These classes select an additional oracle; none disables compilation or
execution. CI recomputes the upstream file set and fails on missing, duplicate,
or stale manifest entries.

## Complete `flowlog-bench` accounting

The benchmark oracle is pinned independently in
[`corpus/flowlog-bench/UPSTREAM_COMMIT`](../corpus/flowlog-bench/UPSTREAM_COMMIT).
Its executable matrix contains:

- all 19 canonical `programs/oracle/flowlog/*/default.dl` programs;
- all 20 upstream Ascent program crates, including the configured `borrow`
  alias;
- both active `programs/ldbc/flowlog` queries;
- 55 `default.txt`, 20 `doop_intensive.txt`, 8 `joinorder.txt`, and 2
  `ldbc.txt` active rows.

All 22 local executables are direct FlowLog-syntax `miniflow!` programs with
checked-in `.decl`, `:-`, and `.output` forms. No local executable invokes
`ascent_par!` or textually includes an upstream macro body.
`scripts/import-flowlog-bench.py --check` deterministically regenerates 18
translations from the pinned upstream Ascent programs and rejects drift. The
four handwritten semantic translations are recursive-min `cc`/`sssp` and
LDBC Q2/Q13. Four-worker row-level tests cover those exceptional
translations, including reachable and unreachable Q13 results.

Join-order files are generated plans rather than distinct Datalog
denotations. The inventory gate compares each program's `.dl` files against
its upstream `manifest.csv`, checks the recorded counts (878 total), validates
each manifest signature, parses every file with the upstream generator's own
scanner, and rejects any difference other than permutation of positive body
atoms. This is structural accounting, not a filename-only count.

Dataset-dependent gates are deliberately strict:

- `scripts/crosscheck-flowlog-bench.sh` runs every requested ordinary
  configured row and compares every printed relation size to a freshly
  compiled FlowLog executable;
- `scripts/crosscheck-flowlog-bench-ldbc.sh` runs every selected parameter row
  and compares complete normalized output row sets.

They fail on a missing dataset, program, output, or mismatch; there is no skip
or accepted-partial mode.

## Profiling

Profiling is a compile-time program attribute and uses the same compiler path:

```rust,ignore
miniflow! {
    #![profile]
    // ...
}
```

The attribute controls generated Timely/Differential instrumentation. It does
not select a second emitter. Profile-disabled and profile-enabled expansions
both have exact oracle fixtures.

## Build policy

Release builds use the same `opt-level = 2` and `panic = "abort"` policy
emitted by FlowLog compiler 0.5.0. Unlike FlowLog's explicitly retained build
directories, MiniFlow keeps incremental compilation disabled in development,
test, and release profiles.

Repository Cargo commands place ordinary artifacts in
`/tmp/miniflow-target`; workspace rust-analyzer checks use
`/tmp/miniflow-rust-analyzer-target`. Verification overrides this with a fresh
temporary target and deletes it on exit. Generated standalone FlowLog crates
retain their own temporary targets because the FlowLog compiler locates the
resulting executable there; each surrounding parity script deletes that
build directory on exit.

`scripts/clean-build-artifacts.sh` removes both disposable targets plus legacy
`target` directories inside the checkout.

## One non-negotiable verification command

```text
scripts/verify.sh
```

This checks formatting, all workspace tests, clippy with warnings denied, the
live pinned Ascent and `flowlog-bench` inventories, FlowLog spelling for every
`miniflow!` source, dependency isolation of the two frontends, all 97 live
FlowLog batch fixture counterparts, all local benchmark contract tests, the
source size budget, runtime and profiler parity, two-build FlowLog
determinism, and canonical generated-Rust byte equality. There is no
snapshot-update mode and no accepted pending fixture. Full real-dataset
benchmark crosschecks use the two explicit commands above because the
upstream datasets are not stored in this repository.
