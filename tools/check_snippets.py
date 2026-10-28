"""Check that the code and output samples in the guides match the programs they
come from.

    python tools/check_snippets.py

Each guide in ecs/docs and docs names its program's source on a line of its
own, in backticks; each dojo block's README is checked against every file of
its block. Then:

  * every ```rust block must appear in that source, or in a module it brings
    in with #[path], exactly, as consecutive lines, indentation included;
  * every plain ``` block whose first line is a line of the program's
    .expected is an output sample: its lines, except "...", must appear in the
    .expected in the same order.

Blocks fenced ```rust ignore are code that is not in the program on purpose,
usually because it does not compile, and are not checked; nor are ```rin
blocks, which quote the rin-dod version, or plain blocks that do not start
with an output line, such as diagrams and compiler errors. A guide that passes
can be copied from with confidence.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = [ROOT / "ecs" / "docs", ROOT / "docs"]


def blocks(text: str) -> list[tuple[str, list[str], int]]:
    """(info string, lines, line number) for every fenced block."""
    found = []
    lines = text.split("\n")
    i = 0
    while i < len(lines):
        m = re.match(r"^```([\w ,]*)\s*$", lines[i])
        if m:
            info, start, body = m.group(1).strip(), i + 1, []
            i += 1
            while i < len(lines) and lines[i].strip() != "```":
                body.append(lines[i])
                i += 1
            found.append((info, body, start))
        i += 1
    return found


def with_modules(source_path: Path) -> list[str]:
    """The program's text, and the text of each module it includes by path."""
    source = source_path.read_text(encoding="utf-8")
    texts = [source]
    for path in re.findall(r'^#\[path = "([^"]+)"\]', source, re.M):
        texts.append((source_path.parent / path).read_text(encoding="utf-8"))
    return texts


def in_order(sample: list[str], expected: list[str]) -> bool:
    at = 0
    for line in sample:
        if line.strip() == "...":
            continue
        try:
            at = expected.index(line, at) + 1
        except ValueError:
            return False
    return True


# A dojo block's README quotes its playground, its harness, the shared dojo.rs
# and the level files, so a sample may come from any of them, and an output
# sample from the playground's or the harness's expected output.
def dojo_sources(block: Path) -> tuple[list[str], list[str], list[Path]]:
    files = sorted(block.rglob("*.rs")) + [block.parent / "dojo.rs"]
    texts = [f.read_text(encoding="utf-8") for f in files if f.name != "kata.rs" or f.parent.name == "solution"]
    outputs = sorted(block.glob("*.expected"))
    expected = [line for o in outputs if o.exists() for line in o.read_text(encoding="utf-8").split("\n")]
    return texts, expected, outputs


def guides() -> list[tuple[Path, list[str], list[str], str, str]]:
    """(guide, source texts, expected lines, what the code is, what the output is)"""
    found = []
    for docs in DOCS:
        for doc in sorted(docs.glob("*.md")):
            m = re.search(r"^`((?:ecs/)?src/[^`]+\.rs)`", doc.read_text(encoding="utf-8"), re.M)
            if not m:
                continue
            source_path = ROOT / m.group(1)
            expected_path = source_path.with_suffix(".expected")
            expected = expected_path.read_text(encoding="utf-8").split("\n") if expected_path.exists() else []
            found.append((doc, with_modules(source_path), expected, m.group(1),
                          expected_path.relative_to(ROOT).as_posix()))
    for block in sorted(p for p in (ROOT / "dojo").glob("*.*") if (p / "README.md").exists()):
        texts, expected, _ = dojo_sources(block)
        name = block.relative_to(ROOT).as_posix()
        found.append((block / "README.md", texts, expected, name, name + "'s expected output"))
    return found


def main() -> int:
    failures = 0
    checked = 0
    for doc, sources, expected, code_name, output_name in guides():
        text = doc.read_text(encoding="utf-8")
        for info, body, line in blocks(text):
            if info == "rust":
                checked += 1
                if not any("\n".join(body) in text for text in sources):
                    failures += 1
                    print(f"{doc.relative_to(ROOT)}:{line}: rust sample not found in {code_name}")
                    for b in body[:3]:
                        print(f"      {b}")
            elif info == "" and body and body[0] in expected:
                checked += 1
                if not in_order(body, expected):
                    failures += 1
                    print(f"{doc.relative_to(ROOT)}:{line}: output sample does not match {output_name}")
    print(f"{checked - failures} of {checked} samples match")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
