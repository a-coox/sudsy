"""Compares an upstream tool with `sudsy lint` on tests/parity/<tool>/ (run by scripts/parity.sh).

Usage: parity_compare.py <tool> <version> <sudsy-binary>
"""
import json
import re
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent


def rule_map(tool):
    """Upstream rule name -> Sudsy rule id, from the catalog's top-level entries."""
    catalog = yaml.safe_load((ROOT / "docs/parity" / f"{tool}.yaml").read_text(encoding="utf-8"))
    return {
        e["id"]: e["sudsy"][len("rule: "):]
        for e in catalog["entries"]
        if e["sudsy"].startswith("rule: ") and "." not in e["id"]
    }


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


def sudsy_lint(sudsy, fixture, cwd):
    out = subprocess.run(
        [sudsy, "lint", "--format", "json", fixture.name],
        cwd=cwd, capture_output=True, text=True, check=False,
    )
    report = json.loads(out.stdout)
    return {
        (d["rule"], d["range"]["start"]["line"], d["range"]["start"]["column"])
        for d in report["diagnostics"]
    }


TOOLS = {"yamllint": yamllint}


def main():
    tool, version, sudsy = sys.argv[1:4]
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    fixtures = sorted(p for p in folder.iterdir() if p.suffix in (".yaml", ".yml") and p.name != "sudsy.yaml")
    mismatches = 0
    for fixture in fixtures:
        upstream = set()
        for rule, line, col in TOOLS[tool](fixture, folder):
            # An upstream rule with no mapping can never match: report it.
            upstream.add((mapping.get(rule, f"<unmapped {tool} rule {rule}>"), line, col))
        ours = sudsy_lint(sudsy, fixture, folder)
        only_upstream, only_ours = sorted(upstream - ours), sorted(ours - upstream)
        status = "ok" if not (only_upstream or only_ours) else "MISMATCH"
        print(f"{fixture.name}: {len(upstream)} {tool}, {len(ours)} sudsy: {status}")
        for rule, line, col in only_upstream:
            print(f"  only {tool}: {rule} {line}:{col}")
        for rule, line, col in only_ours:
            print(f"  only sudsy:  {rule} {line}:{col}")
        mismatches += len(only_upstream) + len(only_ours)
    print(f"{len(fixtures)} fixtures, {mismatches} mismatch(es) ({tool} {version})")
    return 1 if mismatches else 0


sys.exit(main())
