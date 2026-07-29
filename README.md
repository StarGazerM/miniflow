# MiniFlow

MiniFlow is a clean batch-Datalog core derived from the semantics of FlowLog
and the embedded compiler shape of Ascent.

The implementation has three production crates:

- `miniflow-core`: the syntax-neutral source model, relational HIR, dependency
  SCCs, planning, and canonical Rust emission.
- `miniflow-macro`: the private MiniFlow token parser and procedural-macro
  driver that installs the parser into `miniflow-core`.
- `miniflow`: the runtime used by generated programs. It has no compiler,
  macro, or syntax dependency.

There is one surface macro and one rule arrow: `miniflow!` accepts MiniFlow
rules written with `:-`. There is no Ascent-facing macro, parser mode, feature,
or facade crate.

The uncoupled dependency form selects the driver explicitly:

```toml
[dependencies]
miniflow = "0.1"
miniflow-macro = "0.1"
```

```rust
use miniflow_macro::miniflow;

miniflow! {
    struct Reach;

    relation source(i32);
    relation arc(i32, i32);
    relation reach(i32);

    reach(x) :- source(x);
    reach(y) :- reach(x), arc(x, y);
}
```

### Compiler and syntax boundary

`ReadSource` is an open compiler operation from Rust tokens to the public
`source::Program` model. `Compiler::base()` installs no reader and there is no
grammar-selecting constructor in core. The `miniflow-macro` driver installs
its private parser, then reuses name and arity resolution, SCC construction,
physical planning, and DD rendering:

```text
application --> miniflow-macro --> miniflow-core
generated Rust --------------------> miniflow runtime
```

The structural token parser is a private module of `miniflow-macro`, not a
reusable syntax crate and not part of `miniflow-core`. The public `miniflow`
runtime does not depend on either compiler crate.

The external-layer integration test includes a distinct `graph Program;`
surface and proves that it produces the identical expansion without rewriting
to MiniFlow tokens. A genuinely different surface can publish its own driver,
implement `ReadSource`, and depend on `miniflow-core` plus the runtime without
editing either. The dependency gate verifies that neither `miniflow-core` nor
`miniflow` selects a syntax.

## Usage

Declare relations and rules with `miniflow!`, initialize the input relation
fields, and call `run`. Derived relations are written back to their fields:

```rust
use miniflow_macro::miniflow;

miniflow! {
    struct Reach;

    relation source(i32);
    relation arc(i32, i32);
    relation reach(i32);

    reach(x) :- source(x);
    reach(y) :- reach(x), arc(x, y);
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

The checked-in CSV-backed version can be run directly:

```console
$ (cd parity/flowlog/reach && cargo run --quiet -p miniflow-macro --example parity_reach)
1
2
3
```

## The same dataflow as FlowLog

The MiniFlow rules above correspond to this FlowLog program:

```datalog
.decl Source(id: int32)
.input Source(IO="file", filename="Source.csv", delimiter=",")
.decl Arc(x: int32, y: int32)
.input Arc(IO="file", filename="Arc.csv", delimiter=",")

.decl Reach(id: int32)
Reach(y) :- Source(y).
Reach(y) :- Reach(x), Arc(x,y).
.output Reach
```

The two front ends have different host shells: FlowLog generates a standalone
CSV-driven executable, while MiniFlow generates an embedded Rust type.
Inside that shell, both compile the reachability rules to the same Timely and
Differential Dataflow program:

```text
MiniFlow Rust tokens -> MiniFlow HIR and SCC plan --+
                                                     +-> identical canonical
FlowLog .dl         -> FlowLog batch compiler -------+   dataflow closure
```

This is byte equality, not just equivalent output. The parity check extracts
the unique generated `dataflow` closure from each compiler, removes only the
host-specific output sink and FlowLog's returned input handles, formats both
with the same Rust syntax pipeline, and compares the resulting bytes. Input
collections, joins, arrangements, recursive variables, thresholds, and
profiling operators remain in the comparison.

Run the proof against the pinned FlowLog compiler:

```console
$ scripts/verify-flowlog-expansion.sh
FlowLog/MiniFlow canonical dataflow-core parity: all fixtures passed
```

That command checks reachability and every other strict fixture in
[`parity/flowlog/manifest.tsv`](parity/flowlog/manifest.tsv). Result parity is
checked independently by `scripts/verify-flowlog-result.sh`.

There is deliberately no independent arithmetic language, I/O language,
transaction engine, incremental mode, component system, CLI, or Tokio
executor in the core. Rust provides types, expressions, functions, modules,
and program lifecycle. Timely and Differential Dataflow provide the batch
dataflow runtime.

Correctness has two independent gates:

1. Every program in the declared FlowLog/MiniFlow overlap matrix must produce
   byte-identical canonical generated Rust and identical relation output.
2. Every tracked external semantic test, example, and benchmark program must
   have a strict `:-` MiniFlow counterpart. Corpus-coverage tests reject
   missing or silently ignored upstream files.

The pinned
[`flowlog-rs/flowlog-bench`](https://github.com/flowlog-rs/flowlog-bench)
suite is available as 22 executable programs under
[`corpus/flowlog-bench`](corpus/flowlog-bench):

- 19 canonical FlowLog programs;
- the configured `borrow` alias, whose upstream Ascent body is identical to
  `polonius_int`;
- both active LDBC programs.

Eighteen wrappers compile checked-in `:-` MiniFlow fixtures. The inventory gate
reconstructs each fixture from its pinned external oracle using only the
recorded arrow and macro-name translation, then requires byte equality.
`cc`, `sssp`, and the two LDBC queries are explicit embedded translations
because they use recursive minima or have no corresponding Rust oracle.
`WORKERS=N` controls both fact ingestion and Timely/DD workers.

The inventory gate also checks all 85 active configuration rows and proves
that every one of the 878 generated `.dl` files is a pure body-atom
permutation of its canonical program. Generated join-order variants are
therefore accounted as plans, not duplicated as 878 hand-maintained semantic
programs.

The production comparison uses Tokei's `Total` row over Rust source
directories only; tests, examples, and both benchmark corpora are excluded.
The size gate also caps every production Rust file at 2,000 lines so compiler
features cannot rebuild a monolithic planning/code-generation module.

<!-- BEGIN TOKEI COMPARISON -->
| Production Rust | Files | Lines | Code | Comments | Blanks |
|---|---:|---:|---:|---:|---:|
| MiniFlow | 24 | 9506 | 8509 | 390 | 607 |
| FlowLog batch stack | 151 | 39910 | 29303 | 6402 | 4205 |
<!-- END TOKEI COMPARISON -->

MiniFlow is 23.8% of the FlowLog stack by total source lines and 29.1% by Tokei
code lines. Equivalently, FlowLog contains about 3.4 times as much production
code. `scripts/verify-size.sh` runs Tokei, enforces both size bounds, and
rejects a stale README table.

See [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) for the exact contract.

Useful benchmark checks:

```sh
scripts/verify-flowlog-bench-inventory.sh
cargo test -p miniflow-flowlog-bench-corpus --test contracts

# Full configured-pair checks require the upstream datasets.
FACT_DIR=/path/to/flowlog-bench/facts \
  scripts/crosscheck-flowlog-bench.sh
FACT_DIR=/path/to/ldbc/facts \
  scripts/crosscheck-flowlog-bench-ldbc.sh
```
