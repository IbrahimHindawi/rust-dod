"""Check that the code and output samples in the guides match the programs they
come from.

    python tools/check_snippets.py

Each guide in ecs/docs and docs names its program's source on a line of its
own, in backticks. Then:

  * every ```rust block must appear in that source exactly, as consecutive
    lines, indentation included;
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


def main() -> int:
    failures = 0
    checked = 0
    for docs in DOCS:
        for doc in sorted(docs.glob("*.md")):
            text = doc.read_text(encoding="utf-8")
            m = re.search(r"^`((?:ecs/)?src/[^`]+\.rs)`", text, re.M)
            if not m:
                continue
            source_path = ROOT / m.group(1)
            source = source_path.read_text(encoding="utf-8")
            expected_path = source_path.with_suffix(".expected")
            expected = expected_path.read_text(encoding="utf-8").split("\n") if expected_path.exists() else []
            for info, body, line in blocks(text):
                if info == "rust":
                    checked += 1
                    if "\n".join(body) not in source:
                        failures += 1
                        print(f"{doc.relative_to(ROOT)}:{line}: rust sample not found in {m.group(1)}")
                        for b in body[:3]:
                            print(f"      {b}")
                elif info == "" and body and body[0] in expected:
                    checked += 1
                    if not in_order(body, expected):
                        failures += 1
                        print(f"{doc.relative_to(ROOT)}:{line}: output sample does not match "
                              f"{expected_path.relative_to(ROOT)}")
    print(f"{checked - failures} of {checked} samples match")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
