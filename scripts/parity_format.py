"""Compares `sudsy format` with Prettier or Biome on tests/parity/<tool>/ (run by `scripts/parity.sh format`).

Usage: parity_format.py <prettier|biome> <version> <sudsy-binary> <npm-prefix>

Every fixture is formatted by the tool and by Sudsy (with the fixture folder's sudsy.yaml, which
extends the matching preset); the outputs must be equal. A fixture listed in the folder's
`expected-diff.txt` (`<fixture>: <reason>` per line) may differ; it is reported as known, and as
stale when the outputs are in fact equal.
"""
import difflib
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SUFFIXES = {".json", ".jsonc", ".yaml", ".yml"}


def run(cmd, cwd, stdin=None):
    data = stdin.encode("utf-8") if stdin is not None else None
    r = subprocess.run(cmd, cwd=cwd, input=data, capture_output=True, check=False)
    if r.returncode != 0:
        raise SystemExit(f"{' '.join(map(str, cmd))} failed:\n{r.stderr.decode()}{r.stdout.decode()}")
    return r.stdout.decode("utf-8")


def prettier(prefix, fixture, work):
    script = prefix / "node_modules/prettier/bin/prettier.cjs"
    return run(["node", script, "--no-config", "--no-editorconfig", fixture.name], work)


def biome(prefix, fixture, work):
    script = prefix / "node_modules/@biomejs/biome/bin/biome"
    text = fixture.read_bytes().decode("utf-8")
    return run(["node", script, "format", f"--stdin-file-path={fixture.name}"], work, text)


def sudsy_format(sudsy, fixture, work):
    run([sudsy, "format", fixture.name], work)
    return fixture.read_bytes().decode("utf-8")


def expected_diffs(folder):
    path = folder / "expected-diff.txt"
    out = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip() and not line.startswith("#"):
                name, _, reason = line.partition(":")
                out[name.strip()] = reason.strip()
    return out


def main():
    tool, version, sudsy, prefix = sys.argv[1:5]
    prefix = Path(prefix).resolve()
    folder = ROOT / "tests/parity" / tool
    work = ROOT / "target/parity" / f"{tool}-work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    upstream_format = {"prettier": prettier, "biome": biome}[tool]
    known = expected_diffs(folder)
    fixtures = sorted(p for p in folder.iterdir() if p.suffix in SUFFIXES and p.name != "sudsy.yaml")
    bad = 0
    for fixture in fixtures:
        copy = work / fixture.name
        shutil.copy(fixture, copy)
        theirs = upstream_format(prefix, copy, work)
        ours = sudsy_format(sudsy, copy, work)
        same = theirs == ours
        if same and fixture.name in known:
            print(f"{fixture.name}: STALE expected-diff entry (outputs are equal)")
            bad += 1
        elif same:
            print(f"{fixture.name}: ok")
        elif fixture.name in known:
            print(f"{fixture.name}: known difference ({known[fixture.name]})")
        else:
            print(f"{fixture.name}: DIFFERENT")
            sys.stdout.writelines(
                difflib.unified_diff(theirs.splitlines(True), ours.splitlines(True), tool, "sudsy")
            )
            bad += 1
    print(f"{len(fixtures)} fixtures, {bad} mismatch(es) ({tool} {version})")
    return 1 if bad else 0


sys.exit(main())
