#!/usr/bin/env python3
"""Prove every generated benchmark file is only a join-order permutation."""

from __future__ import annotations

import argparse
import collections
import csv
import importlib.util
from pathlib import Path


def load_generator(upstream: Path):
    source = upstream / "scripts/joinorder/gen_joinorder_variants.py"
    spec = importlib.util.spec_from_file_location("flowlog_joinorder", source)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_statement(program: str, variant: str, index: int, base: dict, actual: dict) -> None:
    if base["kind"] != actual["kind"]:
        raise ValueError(f"{program}/{variant}: statement {index} changed kind")
    if base["kind"] != "rule":
        if base["raw"] != actual["raw"]:
            raise ValueError(f"{program}/{variant}: statement {index} changed")
        return
    if base["head"] != actual["head"]:
        raise ValueError(f"{program}/{variant}: rule {index} changed head")
    if base["other_atoms"] != actual["other_atoms"]:
        raise ValueError(f"{program}/{variant}: rule {index} changed filters or negation")
    if collections.Counter(base["pos_atoms"]) != collections.Counter(actual["pos_atoms"]):
        raise ValueError(f"{program}/{variant}: rule {index} changed relational atoms")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("upstream", type=Path)
    arguments = parser.parse_args()
    generator = load_generator(arguments.upstream)
    root = arguments.upstream / "programs/oracle/flowlog"
    checked = 0

    for directory in sorted(path for path in root.iterdir() if path.is_dir()):
        default = directory / "default.dl"
        if not default.is_file():
            continue
        baseline = generator.parse_program(default.read_text())
        manifest = directory / "manifest.csv"
        if manifest.is_file():
            with manifest.open(newline="") as source:
                rows = list(csv.DictReader(source))
            for row in rows:
                if generator.short_id(row["rule_perms"]) != row["signature"]:
                    raise ValueError(
                        f"{directory.name}/{row['variant']}: bad manifest signature"
                    )

        for variant_path in sorted(directory.glob("*.dl")):
            variant = generator.parse_program(variant_path.read_text())
            if len(baseline) != len(variant):
                raise ValueError(
                    f"{directory.name}/{variant_path.name}: statement count changed"
                )
            for index, (base, actual) in enumerate(zip(baseline, variant, strict=True)):
                check_statement(
                    directory.name, variant_path.name, index, base, actual
                )
            checked += 1

    if checked != 878:
        raise ValueError(f"expected 878 variants, checked {checked}")
    print(f"flowlog-bench variants: {checked} files are pure join-order permutations")


if __name__ == "__main__":
    main()
