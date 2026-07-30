# Staged compiler refactor measurements

Measured on 2026-07-29 against baseline commit `86d5735` and the verified
planning-refactor checkpoint before syntax extraction. Both builds used
isolated target directories with:

```text
CARGO_INCREMENTAL=0
CARGO_PROFILE_RELEASE_DEBUG=0
CARGO_PROFILE_RELEASE_STRIP=symbols
cargo build --release -p miniflow-macro
```

The baseline and staged clean builds were run in both orders to reduce
warm-cache bias. Compiler-crates-only time was measured after cleaning only
`miniflow-core` and `miniflow-macro` from otherwise populated release targets.

| Measure | Baseline | Staged | Change |
|---|---:|---:|---:|
| Full clean release wall time | 3.62 s baseline mean | 3.79 s | +4.70% |
| Full clean release peak RSS | 383,800 KiB baseline mean | 385,592 KiB | +0.47% |
| Compiler-crates-only wall time | 1.46 s | 1.55 s | +6.16% |
| Compiler-crates-only peak RSS | 339,572 KiB | 365,692 KiB | +7.69% |
| `miniflow-core` release rlib | 12,774,548 B | 13,821,062 B | +8.19% |
| `miniflow-macro` stripped release `.so` | 4,527,576 B | 4,689,352 B | +3.57% |
| Isolated release target | 63,224,342 B | 64,800,313 B | +2.49% |

After deleting the facade and syntax packages, a final isolated build under
`/data` measured:

| Stable measure | Pre-removal staged | Final | Change |
|---|---:|---:|---:|
| Production Rust lines | 9,564 | 9,506 | -0.61% |
| `miniflow-core` release rlib | 13,821,062 B | 13,548,388 B | -1.97% |
| `miniflow-macro` stripped release `.so` | 4,689,352 B | 4,868,080 B | +3.81% |
| Isolated release target | 64,800,313 B | 64,681,514 B | -0.18% |

At this measured checkpoint the parser belonged to the procedural-macro
artifact, so the `.so` increase was an ownership transfer rather than an added
syntax implementation. The whole isolated target, which counted that code
once, became 118,799 bytes smaller. The final single clean run took 4.76
seconds with 381,564 KiB peak RSS; timings remain host-noise-sensitive.

Timing and RSS are host-noise-sensitive; artifact and target byte counts are
the stable regression signals. The size gate separately requires:

- at most 9,600 production Rust lines and 8,600 code lines;
- more than a 3x source-size advantage over the pinned FlowLog batch stack;
- no production Rust file larger than 2,000 lines.

Canonical expansion byte counts remained unchanged:

| Fixture | Bytes |
|---|---:|
| facts | 5,014 |
| projection | 5,068 |
| join | 6,939 |
| reach | 8,373 |
| negation | 7,927 |

## Runtime and macro dependency inversion

The rejected strict-Ascent component was removed because it only rejected one
arrow spelling before delegating to the shared grammar. The facade and the
separate syntax crate were removed as well. There is now one public macro,
`miniflow!`, and its parser accepts only `:-`.

At this checkpoint `miniflow-core` contained no token parser, and the
`miniflow` runtime contained no compiler, macro, or syntax dependency. The
parser now lives in `miniflow-core`, which already owns both compiler
infrastructure and the default planner/renderer. This lets downstream
procedural macros reuse the default syntax while swapping or inserting typed
compiler stages without an otherwise empty driver crate. `miniflow-macro`
remains only the built-in proc-macro entry point.
