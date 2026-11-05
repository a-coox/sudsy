"""Compares `sudsy format` with Prettier, Biome, taplo or `dotnet format whitespace` on tests/parity/<tool>/ (run by `scripts/parity.sh format`).

Usage: parity_format.py <prettier|biome|taplo|dotnet-format|csharpier|csharpier-xml> <version> <sudsy-binary> <tool-prefix>

Every fixture is formatted by the tool and by Sudsy (with the fixture folder's sudsy.yaml, which
extends the matching preset); the outputs must be equal. A fixture listed in the folder's
`expected-diff.txt` (`<fixture>: <reason>` per line) may differ; it is reported as known, and as
stale when the outputs are in fact equal.
"""
import difflib
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PARITY = ROOT / os.environ.get("CARGO_TARGET_DIR", "target") / "parity"
SUFFIXES = {".json", ".jsonc", ".yaml", ".yml", ".cs", ".toml"}


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


def taplo(prefix, fixture, work):
    exe = prefix / "taplo.exe"
    text = fixture.read_bytes().decode("utf-8")
    try:
        return run([exe if exe.exists() else prefix / "taplo", "fmt", "--no-auto-config", "-"], work, text)
    except SystemExit:
        return None  # taplo rejects the file (it is valid TOML for Sudsy)


def taplo_fixtures(folder):
    """The folder's own fixtures, the TOML files of tests/fixtures/toml and the toml-test valid
    files of the 1.0 list (when fetched), as (name, path); names are the keys of expected-diff.txt."""
    out = [(p.name, p) for p in sorted(folder.glob("*.toml"))]
    out += [(f"fixtures/{p.name}", p) for p in sorted((ROOT / "tests/fixtures/toml").glob("*.toml"))]
    tests = ROOT / "tests/conformance/corpus/toml-test/tests"
    if (tests / "files-toml-1.0.0").exists():
        for rel in (tests / "files-toml-1.0.0").read_text().splitlines():
            path = tests / rel
            if rel.startswith("valid/") and rel.endswith(".toml"):
                try:
                    path.read_bytes().decode("utf-8")
                except UnicodeDecodeError:
                    continue
                out.append((rel, path))
    else:
        print("toml-test corpus not fetched (tests/conformance/fetch-toml-corpus.sh): corpus files skipped")
    return out


def csharpier(prefix, fixture, work):
    exe = prefix / "csharpier.exe"
    # CSharpier formats in place, so it gets a copy: Sudsy formats the original.
    theirs = work / "csharpier" / fixture.name
    theirs.parent.mkdir(exist_ok=True)
    shutil.copy(fixture, theirs)
    try:
        run([exe if exe.exists() else prefix / "csharpier", "format", theirs.name], theirs.parent)
    except SystemExit:
        return None  # CSharpier rejects the file
    return theirs.read_bytes().decode("utf-8")


XML_SUFFIXES = {".csproj", ".props", ".targets", ".config", ".resx", ".xml", ".slnx", ".xaml"}


def xml_fixtures(folder):
    """The folder's own files, those of tests/fixtures/xml and the fetched project files (named
    `corpus/<path>`), as (name, path)."""
    out = [(p.name, p) for p in sorted(folder.iterdir()) if p.suffix in XML_SUFFIXES]
    out += [(f"fixtures/{p.name}", p) for p in sorted((ROOT / "tests/fixtures/xml").iterdir()) if p.suffix in XML_SUFFIXES]
    corpus = ROOT / "tests/conformance/corpus/xml-projects"
    out += [
        (f"corpus/{p.relative_to(corpus).as_posix()}", p)
        for p in sorted(corpus.rglob("*"))
        if p.suffix in XML_SUFFIXES and ".git" not in p.parts
    ]
    return out


def sudsy_format(sudsy, fixture, work):
    try:
        run([sudsy, "format", fixture.name], work)
    except SystemExit:
        return None  # Sudsy rejects the file
    return fixture.read_bytes().decode("utf-8")


def dotnet_format(case, work):
    """`dotnet format whitespace` on a case folder (an `.editorconfig` and `input.cs`)."""
    run(["dotnet", "format", "whitespace", str(case), "--folder"], work)
    return (case / "input.cs").read_bytes().decode("utf-8")


def dotnet_format_main(version, sudsy):
    """One case per `<case>.editorconfig`: `<case>.cs`, else the shared `input.cs`, formatted under it."""
    folder = ROOT / "tests/parity/dotnet-format"
    known = expected_diffs(folder)
    cases = sorted(p.stem for p in folder.glob("*.editorconfig"))
    bad = 0
    for name in cases:
        outputs = []
        for side in ("dotnet-format", "sudsy"):
            case = PARITY / f"dotnet-format-work/{side}/{name}"
            shutil.rmtree(case, ignore_errors=True)
            case.mkdir(parents=True)
            shutil.copy(folder / f"{name}.editorconfig", case / ".editorconfig")
            own = folder / f"{name}.cs"
            shutil.copy(own if own.exists() else folder / "input.cs", case / "input.cs")
            shutil.copy(folder / "sudsy.yaml", case / "sudsy.yaml")
            if side == "sudsy":
                outputs.append(sudsy_format(sudsy, case / "input.cs", case))
            else:
                outputs.append(dotnet_format(case, case))
        theirs, ours = outputs
        same = theirs == ours
        if same and name in known:
            print(f"{name}: STALE expected-diff entry (outputs are equal)")
            bad += 1
        elif same:
            print(f"{name}: ok")
        elif name in known:
            print(f"{name}: known difference ({known[name]})")
        else:
            print(f"{name}: DIFFERENT")
            sys.stdout.writelines(
                difflib.unified_diff(theirs.splitlines(True), ours.splitlines(True), "dotnet-format", "sudsy")
            )
            bad += 1
    print(f"{len(cases)} cases, {bad} mismatch(es) (dotnet format {version})")
    return 1 if bad else 0


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
    sys.stdout.reconfigure(encoding="utf-8")
    tool, version, sudsy, prefix = sys.argv[1:5]
    if tool == "dotnet-format":
        return dotnet_format_main(version, sudsy)
    prefix = Path(prefix).resolve()
    folder = ROOT / "tests/parity" / tool
    work = PARITY / f"{tool}-work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    upstream_format = {"prettier": prettier, "biome": biome, "csharpier": csharpier, "csharpier-xml": csharpier, "taplo": taplo}[tool]
    known = expected_diffs(folder)
    if tool == "taplo":
        fixtures = taplo_fixtures(folder)
    elif tool == "csharpier-xml":
        fixtures = xml_fixtures(folder)
    else:
        fixtures = sorted(
            (p.name, p) for p in folder.iterdir() if p.suffix in SUFFIXES and p.name != "sudsy.yaml"
        )
    bad = 0
    for name, fixture in fixtures:
        copy = work / name.replace("/", "__")
        shutil.copy(fixture, copy)
        theirs = upstream_format(prefix, copy, work)
        ours = sudsy_format(sudsy, copy, work)
        same = theirs == ours
        if theirs is None and name in known:
            print(f"{name}: known difference ({known[name]})")
        elif theirs is None:
            print(f"{name}: DIFFERENT (the tool rejects the file)")
            bad += 1
        elif same and name in known:
            print(f"{name}: STALE expected-diff entry (outputs are equal)")
            bad += 1
        elif same:
            print(f"{name}: ok")
        elif name in known:
            print(f"{name}: known difference ({known[name]})")
        else:
            print(f"{name}: DIFFERENT")
            sys.stdout.writelines(
                difflib.unified_diff((theirs or "").splitlines(True), (ours or "").splitlines(True), tool, "sudsy")
            )
            bad += 1
    print(f"{len(fixtures)} fixtures, {bad} mismatch(es) ({tool} {version})")
    return 1 if bad else 0


sys.exit(main())
