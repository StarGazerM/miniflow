# MiniFlow

MiniFlow is embedded batch Datalog for Rust. It compiles declarative rules to
Timely and Differential Dataflow through a shared relational compiler core.

The workspace provides two independent frontends:

- **MiniFlow** uses FlowLog-style declarations and rules: `.decl`, `:-`,
  `.output`, and head aggregates.
- **AscentFlow** uses Ascent-style syntax: `relation`, `<--`, and
  body-clause expressions.

Both frontends lower to the same frontend-neutral AST, relational HIR, SCC
planner, and canonical dataflow emitter. Their normal dependency graphs remain
isolated; neither public frontend depends on the other.

## Quick start

MiniFlow requires a Rust toolchain with Edition 2024 support.

```console
git clone https://github.com/StarGazerM/miniflow.git
cd miniflow
cargo test --workspace
```

Declare relations and rules with `miniflow!`, initialize the input relation
fields, and call `run`. Derived relations are written back to their fields.

```rust
use miniflow::miniflow;

miniflow! {
    pub struct Reach;

    .decl source(id: int32)
    .decl arc(source: int32, target: int32)
    .decl reach(id: int32)

    reach(x) :- source(x).
    reach(y) :- reach(x), arc(x, y).
}

fn main() {
    let mut program = Reach {
        source: vec![(1,)],
        arc: vec![(1, 2), (2, 3)],
        ..Reach::default()
    };

    program.run();
    assert_eq!(program.reach, vec![(1,), (2,), (3,)]);
}
```

Run the checked-in CSV-backed example:

```console
cd parity/flowlog/reach
cargo run --quiet -p miniflow --example parity_reach
```

It prints:

```text
1
2
3
```

## Ascent-style frontend

AscentFlow provides the same compiler and runtime behind an independent
Ascent-shaped syntax frontend.

```rust
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

## Architecture

| Layer | Crates | Responsibility |
| --- | --- | --- |
| MiniFlow frontend | `miniflow`, `miniflow-macro`, `miniflow-syntax` | FlowLog-style embedded syntax |
| AscentFlow frontend | `ascent-flow`, `ascent-flow-macro`, `ascent-flow-syntax` | Ascent-style embedded syntax |
| Compiler | `miniflow-core` | AST, relational HIR, SCC planning, and Rust emission |
| Runtime | `miniflow-runtime` | Timely and Differential Dataflow execution |

The compiler intentionally focuses on an embedded batch-Datalog core. It does
not provide incremental transactions, a standalone CLI, a Tokio executor, or a
separate arithmetic and typechecking language.

## Compatibility

MiniFlow checks two compatibility boundaries:

1. Programs in the supported FlowLog overlap must produce identical canonical
   dataflow and results.
2. Every tracked Ascent test, example, and benchmark must have a MiniFlow
   counterpart.

The FlowLog oracle is pinned to `flowlog-compiler-v0.5.0` at commit
`6c111b729e4bf8bffb5037b85b894031786140cc`. The `flowlog-bench` inventory is
pinned separately at commit
`2db7c2eab9f64852242a1691b51707f3fb3454ff`.

See [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) for the supported language
surface, parity contract, and benchmark accounting.

### Source footprint

The size gate compares production Rust only, excluding tests, examples, and
benchmark corpora.

<!-- BEGIN TOKEI COMPARISON -->
| Production Rust | Files | Lines | Code | Comments | Blanks |
|---|---:|---:|---:|---:|---:|
| MiniFlow + AscentFlow | 16 | 6905 | 6446 | 105 | 354 |
| FlowLog batch stack | 151 | 39910 | 29303 | 6402 | 4205 |
<!-- END TOKEI COMPARISON -->

## Verification

The standard workspace checks do not require the upstream oracle checkouts:

```console
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The complete parity gate uses local, pinned FlowLog and `flowlog-bench`
checkouts. Prepare them once:

```console
git clone https://github.com/flowlog-rs/flowlog.git flowlog
git -C flowlog checkout 6c111b729e4bf8bffb5037b85b894031786140cc
git -C flowlog apply ../parity/flowlog/oracle.patch

git clone https://github.com/flowlog-rs/flowlog-bench.git flowlog-bench
git -C flowlog-bench checkout 2db7c2eab9f64852242a1691b51707f3fb3454ff

scripts/verify.sh
```

The complete gate checks formatting, tests, Clippy, frontend isolation,
inventory completeness, result parity, canonical expansion parity, and the
production source-size bound. Cargo artifacts are redirected outside the
checkout and the complete gate uses a disposable target directory.

Dataset-backed benchmark comparisons are available separately:

```console
FACT_DIR=/path/to/flowlog-bench/facts scripts/crosscheck-flowlog-bench.sh
FACT_DIR=/path/to/ldbc/facts scripts/crosscheck-flowlog-bench-ldbc.sh
```

## License

MiniFlow is licensed under the [Apache License 2.0](LICENSE).
