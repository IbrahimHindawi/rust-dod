"""Build every program, run it, and compare what it prints with its .expected file.

    python tools/check.py               every program, Debug
    python tools/check.py archetype     only programs whose path contains "archetype"
    python tools/check.py --release     optimised, without debug assertions
    python tools/check.py --bless       rewrite the .expected files from this run
    python tools/check.py --clippy      also run cargo clippy over everything
    python tools/check.py --miri        run the programs that use `unsafe` under Miri

The dojo's blocks are checked too (`python tools/check.py dojo` for only them):
each block's solution.rs, a whole program with the harness as a module, must
build and print its harness.expected; level1.rs must build and fail, since it
has a gap; level2.rs must build. The playgrounds are [[bin]]s, so they are checked like the rest.

A program passes only if every step is clean:

  1. rustc reports no warnings (-D warnings),
  2. the program exits with 0,
  3. its output matches the .expected file beside it, byte for byte.

Every program is a single file built with plain `rustc`, the way you would
build it by hand. The programs are the [[bin]] list in Cargo.toml, so there is
one list to keep, and `cargo build` builds the same files.

rustc is RUSTC or `rustc` on PATH. Miri needs a nightly toolchain with the miri
component (`rustup toolchain install nightly --component miri`).
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "build" / "check"


def programs_from_cargo_toml() -> list[tuple[str, Path]]:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    found = re.findall(r'\[\[bin\]\]\s*name\s*=\s*"([^"]+)"\s*path\s*=\s*"([^"]+)"', text)
    if not found:
        sys.exit("check: no [[bin]] entries in Cargo.toml")
    return [(name, ROOT / path) for name, path in found]


def run(cmd: list[str], cwd: Path = ROOT, env: dict | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, timeout=900, env=env)


def compare(actual: str, expected_path: Path, bless: bool) -> tuple[bool, str]:
    actual = actual.replace("\r\n", "\n")
    if bless:
        expected_path.write_text(actual, encoding="utf-8", newline="\n")
        return True, "blessed"
    if not expected_path.exists():
        return False, f"no {expected_path.name}; run with --bless after reading the output:\n{actual}"
    expected = expected_path.read_text(encoding="utf-8").replace("\r\n", "\n")
    if actual != expected:
        got, want = actual.splitlines(), expected.splitlines()
        for i in range(max(len(got), len(want))):
            g = got[i] if i < len(got) else "<missing>"
            w = want[i] if i < len(want) else "<missing>"
            if g != w:
                return False, f"line {i + 1}\n  got:  {g}\n  want: {w}"
    return True, ""


def check(rustc: str, name: str, source: Path, release: bool, bless: bool) -> tuple[bool, str]:
    work = OUT / ("release" if release else "debug")
    work.mkdir(parents=True, exist_ok=True)
    exe = work / (f"{name}.exe" if os.name == "nt" else name)
    flags = ["--edition", "2024", "-g", "-D", "warnings", "--crate-name", name]
    if release:
        flags += ["-C", "opt-level=3", "-C", "debug-assertions=off"]
    build = run([rustc, *flags, str(source), "-o", str(exe)])
    if build.returncode != 0 or build.stdout.strip():
        return False, "rustc:\n" + build.stdout
    ran = run([str(exe)])
    if ran.returncode != 0:
        return False, f"exit {ran.returncode}:\n{ran.stdout}"
    return compare(ran.stdout, source.with_suffix(".expected"), bless)


def with_modules(source: Path) -> str:
    """A program's text, and the text of each module it includes by path."""
    text = source.read_text(encoding="utf-8")
    for path in re.findall(r'^#\[path = "([^"]+)"\]', text, re.M):
        text += (source.parent / path).read_text(encoding="utf-8")
    return text


def check_miri(name: str, source: Path) -> tuple[bool, str]:
    env = dict(os.environ, MIRIFLAGS="-Zmiri-strict-provenance")
    ran = run(["cargo", "+nightly", "miri", "run", "-q", "--bin", name], env=env)
    if ran.returncode != 0:
        return False, f"miri, exit {ran.returncode}:\n{ran.stdout}"
    return compare(ran.stdout, source.with_suffix(".expected"), False)


# ---------------------------------------------------------------- the dojo

DOJO = ROOT / "dojo"
CLIPPY_DOJO = ["-A", "clippy::new_without_default", "-A", "clippy::len_without_is_empty"]


def dojo_blocks() -> list[Path]:
    return sorted((p for p in DOJO.iterdir() if p.is_dir() and re.match(r"\d+\.", p.name)),
                  key=lambda p: int(p.name.split(".")[0]))


# A kata file is a whole program: it brings in dojo.rs and the harness as
# modules, so each of the solution and the levels builds where it is.
def dojo_build(rustc: str, block: Path, kata: Path, where: str, release: bool,
               flags: list[str], driver: str | None = None) -> tuple[Path | None, str]:
    folder = OUT / ("release" if release else "debug") / "dojo" / block.name
    folder.mkdir(parents=True, exist_ok=True)
    exe = folder / (f"{where}.exe" if os.name == "nt" else where)
    opt = ["-C", "opt-level=3", "-C", "debug-assertions=off"] if release else []
    built = run([driver or rustc, "--edition", "2024", "-g", *opt, *flags, "--crate-name", where,
                 str(kata), "-o", str(exe)])
    if built.returncode != 0 or built.stdout.strip():
        return None, built.stdout
    return exe, ""


def check_dojo(rustc: str, block: Path, release: bool, bless: bool) -> tuple[bool, str]:
    exe, err = dojo_build(rustc, block, block / "solution.rs", "solution", release, ["-D", "warnings"])
    if not exe:
        return False, "rustc, harness with the solution:\n" + err
    ran = run([str(exe)])
    if ran.returncode != 0:
        return False, f"the solution fails its harness, exit {ran.returncode}:\n{ran.stdout}"
    ok, detail = compare(ran.stdout, block / "harness.expected", bless)
    if not ok:
        return False, detail
    for level in (1, 2):
        exe, err = dojo_build(rustc, block, block / f"level{level}.rs", f"level{level}", release,
                              ["-D", "warnings", "-A", "unused"])
        if not exe:
            return False, f"rustc, harness with level {level}:\n" + err
        if level == 1 and run([str(exe)]).returncode == 0:
            return False, "level 1 passes the harness as given; its gap is not a gap"
    return True, detail


def clippy_dojo(rustc: str, block: Path) -> tuple[bool, str]:
    driver = shutil.which("clippy-driver")
    if not driver:
        return False, "clippy-driver not found"
    exe, err = dojo_build(rustc, block, block / "solution.rs", "clippy", False,
                          ["-D", "warnings", *CLIPPY_DOJO], driver)
    return exe is not None, err


def miri_dojo(block: Path) -> tuple[bool, str]:
    pkg = OUT / "miri" / block.name
    pkg.mkdir(parents=True, exist_ok=True)
    (pkg / "Cargo.toml").write_text(
        f'[package]\nname = "dojo-check"\nversion = "0.1.0"\nedition = "2024"\nautobins = false\n\n'
        f'[[bin]]\nname = "harness"\npath = "{(block / "solution.rs").as_posix()}"\n', encoding="utf-8")
    env = dict(os.environ, MIRIFLAGS="-Zmiri-strict-provenance")
    ran = run(["cargo", "+nightly", "miri", "run", "-q", "--", "--ops", "300"], cwd=pkg, env=env)
    if ran.returncode != 0 or "PASS" not in ran.stdout.splitlines():
        return False, f"miri, exit {ran.returncode}:\n{ran.stdout}"
    return True, ""


def main() -> int:
    args = sys.argv[1:]
    release = "--release" in args
    bless = "--bless" in args
    filters = [a for a in args if not a.startswith("--")]

    rustc = os.environ.get("RUSTC") or shutil.which("rustc")
    if not rustc:
        sys.exit("check: rustc not found; set RUSTC or put rustc on PATH")
    programs = [(n, p) for n, p in programs_from_cargo_toml()
                if not filters or any(f in p.as_posix() for f in filters)]

    failed = 0
    for name, source in programs:
        ok, detail = check(rustc, name, source, release, bless)
        rel = source.relative_to(ROOT).as_posix()
        print(f"{'ok  ' if ok else 'FAIL'} {rel}" + (f"  ({detail})" if ok and detail else ""))
        if not ok:
            failed += 1
            print("     " + detail.replace("\n", "\n     "))
    print(f"{len(programs) - failed} of {len(programs)} passed")

    blocks = [b for b in dojo_blocks()
              if not filters or any(f in b.relative_to(ROOT).as_posix() for f in filters)]
    dojo_failed = 0
    for block in blocks:
        ok, detail = check_dojo(rustc, block, release, bless)
        print(f"{'ok  ' if ok else 'FAIL'} {block.relative_to(ROOT).as_posix()}, the solution and the levels"
              + (f"  ({detail})" if ok and detail else ""))
        if not ok:
            dojo_failed += 1
            print("     " + detail.replace("\n", "\n     "))
    if blocks:
        print(f"{len(blocks) - dojo_failed} of {len(blocks)} dojo blocks passed")
    failed += dojo_failed

    if "--clippy" in args:
        clippy = run(["cargo", "clippy", "-q", "--all-targets", "--", "-D", "warnings"])
        print("clippy: " + ("clean" if clippy.returncode == 0 and not clippy.stdout.strip() else "FAIL"))
        if clippy.returncode != 0 or clippy.stdout.strip():
            failed += 1
            print("     " + clippy.stdout.replace("\n", "\n     "))
        for block in blocks:
            ok, detail = clippy_dojo(rustc, block)
            print(f"clippy: {'clean' if ok else 'FAIL'} {block.relative_to(ROOT).as_posix()}, the solution")
            if not ok:
                failed += 1
                print("     " + detail.replace("\n", "\n     "))

    if "--miri" in args:
        # Only the programs with an `unsafe` block: Miri is slow, and safe Rust
        # has nothing for it to find.
        unsafe_ones = [(n, p) for n, p in programs if "unsafe {" in with_modules(p)]
        for name, source in unsafe_ones:
            ok, detail = check_miri(name, source)
            print(f"{'ok  ' if ok else 'FAIL'} miri {source.relative_to(ROOT).as_posix()}")
            if not ok:
                failed += 1
                print("     " + detail.replace("\n", "\n     "))
        # The dojo's solutions with `unsafe`, through their harnesses, with a
        # short fuzz run.
        for block in blocks:
            if "unsafe {" in (block / "solution.rs").read_text(encoding="utf-8"):
                ok, detail = miri_dojo(block)
                print(f"{'ok  ' if ok else 'FAIL'} miri {block.relative_to(ROOT).as_posix()}, the solution")
                if not ok:
                    failed += 1
                    print("     " + detail.replace("\n", "\n     "))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
