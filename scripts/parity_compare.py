"""Compares an upstream tool with `sudsy lint` on tests/parity/<tool>/ (run by scripts/parity.sh).

Usage: parity_compare.py <tool> <version> <sudsy-binary>
"""
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent


def catalog_entries(tool):
    catalog = yaml.safe_load((ROOT / "docs/parity" / f"{tool}.yaml").read_text(encoding="utf-8"))
    return [e for e in catalog["entries"] if "." not in e["id"]]


def rule_map(tool):
    """Upstream rule name -> Sudsy rule id, from the catalog's top-level entries."""
    return {
        e["id"]: e["sudsy"][len("rule: "):]
        for e in catalog_entries(tool)
        if e["sudsy"].startswith("rule: ")
    }


def formatter_ids(tool):
    """`(covered, partial)`: upstream rule names the formatter handles, from the catalog: status
    `formatter`, or `option: csharp.format.<name>` for an option the stylecop preset sets (the
    fixtures format with it). `partial` entries state a gap."""
    preset = yaml.safe_load((ROOT / "crates/lang-csharp/presets/stylecop.yaml").read_text(encoding="utf-8"))
    options = set(preset["csharp"]["format"])
    def ids(coverage):
        return {
            e["id"]
            for e in catalog_entries(tool)
            if e["coverage"] == coverage
            and (e["sudsy"] == "formatter" or e["sudsy"].removeprefix("option: csharp.format.") in options)
        }
    return ids("covered"), ids("partial")


def yamllint(fixture, cwd):
    """`{(upstream rule, line, column)}` of the fixture."""
    out = subprocess.run(
        [sys.executable, "-m", "yamllint", "-f", "parsable", "-c", ".yamllint", fixture.name],
        cwd=cwd, capture_output=True, text=True, check=False,
    ).stdout
    found = set()
    for line in out.splitlines():
        m = re.match(r"^.+?:(\d+):(\d+): \[\w+\] .*?(?: \(([\w-]+)\))?$", line)
        if m:
            found.add((m.group(3) or "syntax-error", int(m.group(1)), int(m.group(2))))
    return found


def sudsy_lint(sudsy, fixture, cwd, only=None):
    """`{(rule, line, column)}`; with `only`, the diagnostics of those rules."""
    out = subprocess.run(
        [sudsy, "lint", "--format", "json", fixture.name],
        cwd=cwd, capture_output=True, text=True, check=False,
    )
    report = json.loads(out.stdout)
    return {
        (d["rule"], d["range"]["start"]["line"], d["range"]["start"]["column"])
        for d in report["diagnostics"]
        if only is None or d["rule"] in only
    }


def compare(label, upstream, ours, tool):
    """Prints the differences between two `{(rule, line, column)}` sets; returns their count."""
    only_upstream, only_ours = sorted(upstream - ours), sorted(ours - upstream)
    status = "ok" if not (only_upstream or only_ours) else "MISMATCH"
    print(f"{label}: {len(upstream)} {tool}, {len(ours)} sudsy: {status}")
    for rule, line, col in only_upstream:
        print(f"  only {tool}: {rule} {line}:{col}")
    for rule, line, col in only_ours:
        print(f"  only sudsy:  {rule} {line}:{col}")
    return len(only_upstream) + len(only_ours)


def stylecop_build(directory, ids, version):
    """`{(file name, rule, line, column)}` of the StyleCop diagnostics `dotnet build` reports for the
    fixture project in `directory`, restricted to `ids` (each enabled as a warning; the other
    StyleCop rules are noise for the comparison)."""
    (directory / ".editorconfig").write_text(
        "root = true\n[*.cs]\n" + "".join(f"dotnet_diagnostic.{i}.severity = warning\n" for i in sorted(ids)),
        encoding="utf-8",
    )
    log = directory / "build.sarif"
    run = subprocess.run(
        ["dotnet", "build", "--no-incremental", "-nologo", "-v:q", f"-p:StyleCopVersion={version}",
         f"-p:ErrorLog={log}"],
        cwd=directory, capture_output=True, text=True, check=False,
    )
    if not log.exists():
        sys.exit(f"dotnet build produced no error log:\n{run.stdout}\n{run.stderr}")
    found = set()
    for result in json.loads(log.read_text(encoding="utf-8"))["runs"][0]["results"]:
        if result["ruleId"] not in ids:
            continue
        location = result["locations"][0]
        location = location.get("physicalLocation") or location.get("resultFile")
        uri = (location.get("artifactLocation") or location)["uri"]
        region = location["region"]
        name = Path(uri.replace("%20", " ")).name
        found.add((name, result["ruleId"], region["startLine"], region["startColumn"]))
    return found


def stylecop(sudsy, version):
    """Two builds of the fixtures. Plain: every `rule:` id StyleCop reports is reported by `sudsy
    lint` at the same place, and nothing else. After `sudsy fix` (which formats): no `formatter` id
    remains, and what StyleCop still reports is what `sudsy lint` still reports."""
    mapping = rule_map("stylecop")
    formatter, partial = formatter_ids("stylecop")
    mismatches = 0
    for name in ("plain", "fixed"):
        work = ROOT / "target/parity" / f"stylecop-{name}"
        shutil.rmtree(work, ignore_errors=True)
        shutil.copytree(ROOT / "tests/parity/stylecop", work)
        if name == "fixed":
            subprocess.run([sudsy, "fix", "--unsafe", "."], cwd=work, capture_output=True, text=True, check=False)
        built = stylecop_build(work, set(mapping) | formatter | partial, version)
        if name == "plain":
            idle = sorted(formatter - {r for _, r, _, _ in built})
            print(f"formatter ids no fixture violates: {' '.join(idle)}")
        for fixture in sorted(work.glob("*.cs")):
            upstream = {(mapping[r], l, c) for f, r, l, c in built if f == fixture.name and r in mapping}
            mismatches += compare(f"{fixture.name} ({name})", upstream, sudsy_lint(sudsy, fixture, work, set(mapping.values())), "stylecop")
        if name == "fixed":
            for f, r, l, c in sorted(built):
                # A `partial` entry's gap is only expected outside the `layout_*` fixtures.
                if r in formatter or (r in partial and f.startswith("layout_")):
                    print(f"  formatter id left after fix: {f}: {r} {l}:{c}")
                    mismatches += 1
    print(f"stylecop {version}: {mismatches} mismatch(es)")
    return 1 if mismatches else 0


def main():
    tool, version, sudsy = sys.argv[1:4]
    if tool == "stylecop":
        return stylecop(sudsy, version)
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    fixtures = sorted(p for p in folder.iterdir() if p.suffix in (".yaml", ".yml") and p.name != "sudsy.yaml")
    mismatches = 0
    for fixture in fixtures:
        # An upstream rule with no mapping can never match: report it.
        upstream = {(mapping.get(r, f"<unmapped {tool} rule {r}>"), l, c) for r, l, c in yamllint(fixture, folder)}
        mismatches += compare(fixture.name, upstream, sudsy_lint(sudsy, fixture, folder), tool)
    print(f"{len(fixtures)} fixtures, {mismatches} mismatch(es) ({tool} {version})")
    return 1 if mismatches else 0


sys.exit(main())
