# Staged compiler refactor measurements

Measured on 2026-07-29 against baseline commit `86d5735` and the final verified
staged-refactor tree. Both builds used isolated target directories with:

```text
CARGO_INCREMENTAL=0
CARGO_PROFILE_RELEASE_DEBUG=0
CARGO_PROFILE_RELEASE_STRIP=symbols
cargo build --release -p miniflow-macro
```

The full clean build was run in both orders to reduce warm-cache bias.
Compiler-crates-only time was measured after cleaning only `miniflow-core` and
`miniflow-macro` from otherwise populated release targets.

| Measure | Baseline | Staged | Change |
|---|---:|---:|---:|
| Full clean release wall time | 3.62 s baseline mean | 3.79 s | +4.70% |
| Full clean release peak RSS | 383,800 KiB baseline mean | 385,592 KiB | +0.47% |
| Compiler-crates-only wall time | 1.46 s | 1.55 s | +6.16% |
| Compiler-crates-only peak RSS | 339,572 KiB | 365,692 KiB | +7.69% |
| `miniflow-core` release rlib | 12,774,548 B | 13,821,062 B | +8.19% |
| `miniflow-macro` stripped release `.so` | 4,527,576 B | 4,689,352 B | +3.57% |
| Isolated release target | 63,224,342 B | 64,800,313 B | +2.49% |

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
