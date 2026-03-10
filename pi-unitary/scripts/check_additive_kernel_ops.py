#!/usr/bin/env python3
"""Fail if additive kernel files contain forbidden arithmetic operators.

This guard enforces the additive-only contract for designated hot-path modules.
"""

from __future__ import annotations

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]

# Files where hot-path arithmetic must remain additive-only.
TARGETS = [
    ROOT / "crates" / "pi-sim" / "src" / "recursive.rs",
    ROOT / "crates" / "pi-sim" / "src" / "choke_additive.rs",
]

FORBIDDEN = {"*", "/", "%"}


def strip_comments_and_strings(src: str) -> str:
    out: list[str] = []
    i = 0
    n = len(src)
    state = "code"
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""

        if state == "code":
            if ch == "/" and nxt == "/":
                state = "line_comment"
                out.append("  ")
                i += 2
                continue
            if ch == "/" and nxt == "*":
                state = "block_comment"
                out.append("  ")
                i += 2
                continue
            if ch == '"':
                state = "string"
                out.append(" ")
                i += 1
                continue
            if ch == "'":
                state = "char"
                out.append(" ")
                i += 1
                continue
            out.append(ch)
            i += 1
            continue

        if state == "line_comment":
            if ch == "\n":
                state = "code"
                out.append("\n")
            else:
                out.append(" ")
            i += 1
            continue

        if state == "block_comment":
            if ch == "*" and nxt == "/":
                state = "code"
                out.append("  ")
                i += 2
            else:
                out.append("\n" if ch == "\n" else " ")
                i += 1
            continue

        if state == "string":
            if ch == "\\" and i + 1 < n:
                out.append("  ")
                i += 2
                continue
            if ch == '"':
                state = "code"
                out.append(" ")
            else:
                out.append("\n" if ch == "\n" else " ")
            i += 1
            continue

        if state == "char":
            if ch == "\\" and i + 1 < n:
                out.append("  ")
                i += 2
                continue
            if ch == "'":
                state = "code"
                out.append(" ")
            else:
                out.append("\n" if ch == "\n" else " ")
            i += 1
            continue

    return "".join(out)


def find_forbidden_ops(src: str) -> list[tuple[int, int, str]]:
    clean = strip_comments_and_strings(src)
    hits: list[tuple[int, int, str]] = []

    def prev_nonspace(idx: int) -> str:
        j = idx - 1
        while j >= 0 and clean[j].isspace():
            j -= 1
        return clean[j] if j >= 0 else ""

    def next_nonspace(idx: int) -> str:
        j = idx + 1
        while j < n and clean[j].isspace():
            j += 1
        return clean[j] if j < n else ""

    unary_prefixes = set("=([{:+-*/%&|!?,;<>")

    line = 1
    col = 0
    i = 0
    n = len(clean)
    while i < n:
        ch = clean[i]
        col += 1
        if ch == "\n":
            line += 1
            col = 0
            i += 1
            continue

        if ch in FORBIDDEN:
            prev = prev_nonspace(i)
            nxt = next_nonspace(i)

            # Allow unary dereference (*value) while still rejecting multiplication.
            if ch == "*" and (prev == "" or prev in unary_prefixes) and (
                nxt.isalpha() or nxt == "_"
            ):
                i += 1
                continue

            # Allow glob import/export patterns like `use super::*;`.
            if ch == "*" and nxt in ";,}" and prev in {":", "{"}:
                i += 1
                continue

            hits.append((line, col, ch))
        i += 1
    return hits


def main() -> int:
    failures: list[str] = []
    for path in TARGETS:
        if not path.exists():
            failures.append(f"Missing target file: {path}")
            continue
        src = path.read_text(encoding="utf-8")
        hits = find_forbidden_ops(src)
        for line, col, op in hits:
            rel = path.relative_to(ROOT)
            failures.append(f"{rel}:{line}:{col} forbidden operator '{op}'")

    if failures:
        print("Additive-only kernel check failed:")
        for msg in failures:
            print(f"  - {msg}")
        return 1

    print("Additive-only kernel check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
