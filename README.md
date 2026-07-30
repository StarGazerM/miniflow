# MiniFlow

MiniFlow is a clean batch-Datalog core derived from the semantics of FlowLog
and the embedded compiler shape of Ascent.

The implementation has three production crates:

- `miniflow-core`: the default MiniFlow syntax, public source model, relational
  HIR, typed compiler pipeline, planning, and canonical Rust emission.
- `miniflow-macro`: the thin built-in procedural-macro entry point.
- `miniflow`: the runtime used by generated programs. It has no compiler,
  macro, or syntax dependency.

The project ships one surface macro and one rule arrow: `miniflow!` accepts
MiniFlow rules written with `:-`. There is no Ascent-facing macro, parser mode,
feature, or facade crate.

The uncoupled dependency form selects the proc macro explicitly:

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

`CompilerPipeline` exposes four typed, replaceable boundaries:

```text
TokenStream -> source::Program -> HirProgram -> ProgramPlan -> TokenStream
                reader          lowerer       planner        renderer
```

Each stage can be replaced with a function of the same type, or extended by
inserting a carrier-preserving function after it. Fine-grained `PlanRule` and
`PlanScc` layers remain available inside the planning stage. This is a
direct-style pipeline; stage outputs are ordinary typed values, not a CPS code
stream.

The default parser and composition are ordinary public functions in
`miniflow-core`, so a downstream proc-macro can start from
`default_pipeline()`, insert or replace stages, and then call `expand`. The
shipped `miniflow-macro` does only the final rustc proc-macro wrapping:

```text
application --> custom macro or miniflow-macro --> miniflow-core
generated Rust ----------------------------------> miniflow runtime
```

The external-layer integration test includes a distinct `graph Program;`
surface and proves that it produces the identical expansion without rewriting
to MiniFlow tokens. A separate proc-macro fixture additionally inserts an HIR
pass and replaces the planner. A genuinely different surface can construct
`CompilerPipeline::new(reader)` directly; a MiniFlow-compatible macro can
reuse `miniflow_core::default_pipeline()`. Neither requires editing this
repository.

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

### Inline Rust computation

A rule-body `let` accepts a Rust pattern and any Rust expression. Because a
block is an expression, computation can contain nested local bindings,
conditionals, matches, calls, and closures:

```rust
miniflow! {
    struct Adjusted;
    relation input(i32);
    relation output(i32);

    output(adjusted) :-
        input(value),
        let adjusted = {
            let doubled = *value * 2;
            if doubled > 10 {
                doubled + 1
            } else {
                doubled
            }
        };
}
```

The block runs inside the generated dataflow operator for each matching row.
A rule must still begin with a positive relational atom, existing rule
variables are references, and the `let` pattern must be irrefutable. Use
`if let` for a refutable pattern. Side effects should be avoided because rows
may be evaluated in parallel and without a deterministic execution order.

The pinned FlowLog frontend cannot embed an equivalent Rust block or `let`
clause directly in a `.dl` rule. Its rule bodies contain relational atoms,
comparisons, string constraints, negation, and disjunction. FlowLog can still
call Rust through a declared `.extern fn`, but its implementation must live in
a separate Rust UDF file supplied to the compiler with `--udf`. MiniFlow's
difference is direct host-Rust embedding inside the rule, not an exclusive
ability to invoke Rust.


## The same dataflow as FlowLog

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
| MiniFlow | 23 | 9595 | 8565 | 409 | 621 |
| FlowLog batch stack | 151 | 39910 | 29303 | 6402 | 4205 |
<!-- END TOKEI COMPARISON -->

MiniFlow is 24.0% of the FlowLog stack by total source lines and 29.2% by Tokei
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
