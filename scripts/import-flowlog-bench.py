#!/usr/bin/env python3
"""Import pinned FlowLog-bench programs into the FlowLog-syntax frontend.

The upstream Rust programs are already semantics-preserving Ascent
translations with the intended join order.  This importer changes only the
embedded surface spelling and keeps each host benchmark harness verbatim.
"""

from __future__ import annotations

import re
import subprocess
import sys
from difflib import unified_diff
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
UPSTREAM = ROOT / "flowlog-bench" / "programs" / "oracle" / "ascent"
DESTINATION = ROOT / "corpus" / "flowlog-bench" / "src" / "bin"
PROGRAMS = (
    "andersen",
    "bipartite",
    "borrow",
    "crdt",
    "crdt_slow",
    "csda",
    "cspa",
    "cvc5",
    "doop",
    "dyck",
    "galen",
    "pointsto",
    "polonius_int",
    "polonius_str",
    "reach",
    "sg",
    "tc",
    "z3",
)


def split_types(source: str) -> list[str]:
    fields: list[str] = []
    start = 0
    depth = 0
    for index, char in enumerate(source):
        if char in "<([{":
            depth += 1
        elif char in ">)]}":
            depth -= 1
        elif char == "," and depth == 0:
            fields.append(source[start:index].strip())
            start = index + 1
    tail = source[start:].strip()
    if tail:
        fields.append(tail)
    return fields


def outputs(local_source: str) -> list[str]:
    attribute = re.search(r"#!\[output\((.*?)\)\]", local_source, re.DOTALL)
    if attribute:
        return [
            name
            for name in re.split(r"[\s,]+", attribute.group(1))
            if name
        ]
    directives = re.findall(r"^\s*\.output\s+([A-Za-z_]\w*)", local_source, re.MULTILINE)
    if directives:
        return directives
    raise ValueError("local benchmark has no explicit output inventory")


def macro_bounds(source: str) -> tuple[int, int]:
    start = source.index("ascent_par! {")
    body_start = start + len("ascent_par! {")
    depth = 1
    for index in range(body_start, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return start, index + 1
    raise ValueError("unterminated ascent_par! invocation")


def translate_body(body: str) -> str:
    translated: list[str] = []
    in_rule = False
    relation = re.compile(r"^(\s*)relation\s+([A-Za-z_]\w*)\((.*)\);$")
    for line in body.splitlines():
        declaration = relation.match(line)
        if declaration:
            indent, name, raw_types = declaration.groups()
            columns = ", ".join(
                f"c{index}: {ty}" for index, ty in enumerate(split_types(raw_types))
            )
            translated.append(f"{indent}.decl {name}({columns})")
            continue

        line = line.replace("<--", ":-")
        line = line.replace(", if ", ", ")
        line = re.sub(r"^(\s*)if\s+", r"\1", line)
        if ":-" in line:
            in_rule = True
        if in_rule and line.rstrip().endswith(";"):
            line = line.rstrip()[:-1] + "."
            in_rule = False
        translated.append(line)
    return "\n".join(translated)


def rustfmt(source: str) -> str:
    result = subprocess.run(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"],
        input=source,
        text=True,
        check=True,
        capture_output=True,
    )
    return result.stdout


def render_program(program: str) -> tuple[Path, str]:
    destination = DESTINATION / f"{program}.rs"
    selected_outputs = outputs(destination.read_text())
    upstream = (UPSTREAM / program / "src" / "main.rs").read_text()
    start, end = macro_bounds(upstream)
    macro = upstream[start:end]
    body = macro[len("ascent_par! {") : -1]
    flowlog = translate_body(body)
    output_directives = "\n".join(f"    .output {name}" for name in selected_outputs)
    replacement = (
        "miniflow! {\n"
        "    #![flowlog_batch]\n"
        f"{flowlog.rstrip()}\n\n"
        f"{output_directives}\n"
        "}"
    )
    rendered = upstream[:start] + replacement + upstream[end:]
    rendered = rendered.replace("use ascent::ascent_par;", "use miniflow::miniflow;")
    rendered = rendered.replace(
        "Ascent translation of", "MiniFlow FlowLog-syntax translation of"
    )
    if "use harness::*;" in rendered:
        rendered = (
            "#![allow(clippy::unreadable_literal, clippy::wildcard_imports)]\n\n"
            + rendered
        )
    return destination, rustfmt(rendered)


def main() -> None:
    check = sys.argv[1:] == ["--check"]
    if sys.argv[1:] and not check:
        raise SystemExit("usage: import-flowlog-bench.py [--check]")
    failed = False
    for program in PROGRAMS:
        destination, rendered = render_program(program)
        if check:
            actual = destination.read_text()
            if actual != rendered:
                failed = True
                sys.stderr.writelines(
                    unified_diff(
                        actual.splitlines(keepends=True),
                        rendered.splitlines(keepends=True),
                        fromfile=str(destination),
                        tofile=f"{destination} (pinned import)",
                    )
                )
        else:
            destination.write_text(rendered)
    if failed:
        raise SystemExit("FlowLog benchmark sources drifted from the pinned import")


if __name__ == "__main__":
    main()
