"""Compares an upstream tool with `sudsy lint` on tests/parity/<tool>/ (run by scripts/parity.sh).

Usage: parity_compare.py <tool> <version> <sudsy-binary>
"""
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
PARITY = ROOT / os.environ.get("CARGO_TARGET_DIR", "target") / "parity"
CATALOGS = {"eslint-jsonc": "eslint-plugin-jsonc"}  # tool name -> catalog file, where they differ


def catalog_entries(tool, sub=False):
    catalog = yaml.safe_load((ROOT / "docs/parity" / f"{CATALOGS.get(tool, tool)}.yaml").read_text(encoding="utf-8"))
    return [e for e in catalog["entries"] if sub or "." not in e["id"]]


def rule_map(tool):
    """Upstream rule name (`anchors.forbid-unused-anchors` for an option that is a rule of its own) -> Sudsy
    rule id, from the catalog."""
    return {
        e["id"]: e["sudsy"][len("rule: "):]
        for e in catalog_entries(tool, sub=True)
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
        m = re.match(r"^.+?:(\d+):(\d+): \[\w+\] (.*?)(?: \(([\w-]+)\))?$", line)
        if m:
            rule, column = m.group(4) or "syntax-error", int(m.group(2))
            if rule == "anchors":  # one yamllint rule, three Sudsy rules: "found unused anchor ..."
                rule += {"undeclared": ".forbid-undeclared-aliases", "duplicated": ".forbid-duplicated-anchors", "unused": ".forbid-unused-anchors"}[m.group(3).split()[1]]
            if rule in ("braces", "brackets") and "forbidden" in m.group(3):  # the `forbid` option
                rule += ".forbid"
                column -= 1  # after the delimiter
            # yamllint reports these two after the token: Sudsy reports the token (the `:` or `-`,
            # the octal number).
            if rule == "empty-values":
                column -= 1
            elif rule == "octal-values":
                column -= len(re.search(r'"(.*)"$', m.group(3)).group(1))
            found.add((rule, int(m.group(1)), column))
    return found


def eslint_jsonc(version, folder):
    """`{fixture name: {(upstream rule, line, column)}}` of the `.json` fixtures. ESLint and the plugin
    are installed by parity.sh into target/parity/eslint-jsonc-<version>/; the fixtures and the flat
    config are copied below it so that the config finds the plugin."""
    install = PARITY / f"eslint-jsonc-{version}"
    work = install / "work"
    shutil.rmtree(work, ignore_errors=True)
    shutil.copytree(folder, work, ignore=shutil.ignore_patterns("sudsy.yaml"))
    run = subprocess.run(
        ["node", str(install / "node_modules/eslint/bin/eslint.js"), "-f", "json", "."],
        cwd=work, capture_output=True, text=True, check=False,
    )
    try:
        results = json.loads(run.stdout)
    except json.JSONDecodeError:
        sys.exit(f"eslint produced no report:\n{run.stdout}\n{run.stderr}")
    return {
        Path(r["filePath"]).name: {(m["ruleId"] or "syntax-error", m["line"], m["column"]) for m in r["messages"]}
        for r in results
    }


def markdownlint(version, folder):
    """`{fixture name: {(upstream rule, line, 0)}}` of the `.md` fixtures (markdownlint reports a column
    for some rules only, so the column is left out of the comparison; Sudsy's is zeroed alike).
    markdownlint-cli2 is installed by parity.sh into target/parity/markdownlint-<version>/; the
    fixtures and their `.markdownlint.jsonc` are copied below it."""
    install = PARITY / f"markdownlint-{version}"
    work = install / "work"
    shutil.rmtree(work, ignore_errors=True)
    shutil.copytree(folder, work, ignore=shutil.ignore_patterns("sudsy.yaml"))
    run = subprocess.run(
        ["node", str(install / "node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs"), "*.md"],
        cwd=work, capture_output=True, text=True, check=False,
    )
    found = {p.name: set() for p in work.glob("*.md")}
    for line in (run.stdout + run.stderr).splitlines():
        m = re.match(r"^(\S+\.md):(\d+)(?::\d+)? \S+ (MD\d+)", line)
        if m:
            found[m.group(1)].add((m.group(3), int(m.group(2)), 0))
    return found


def rule_fixtures(tool, sudsy, version, exe):
    """Runs the tool (hadolint, shellcheck) and `sudsy lint` on the rule fixtures of its language
    (tests/rules/<lang>/<folder>/*<suffix>, annotations removed) and compares `(rule, line)` of the
    catalog's rules: upstream codes the catalog does not map are left out.
    `tests/parity/<tool>/expected-diff.txt` lists known differences as
    `<folder>/<name> <tool|sudsy>:<rule> <line>: <reason>`; one that no longer differs fails."""
    lang, suffix, command, parse = {
        "hadolint": ("dockerfile", ".dockerfile", [exe, "--no-fail", "--no-color", "-f", "json"],
                     lambda d: (d["code"], d["line"])),
        "shellcheck": ("shell", ".sh", [exe, "-s", "bash", "-o", "all", "-f", "json"],
                       lambda d: (f"SC{d['code']}", d["line"])),
    }[tool]
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    work = PARITY / f"{tool}-{version}" / "work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    expected = {}
    for line in (folder / "expected-diff.txt").read_text(encoding="utf-8").splitlines():
        if line.strip() and not line.startswith("#"):
            key, _, reason = line.partition(": ")
            expected[key] = reason
    used, mismatches, total = set(), 0, 0
    for source in sorted((ROOT / "tests/rules" / lang).glob(f"*/*{suffix}")):
        text = "".join(
            l for l in source.read_text(encoding="utf-8").splitlines(keepends=True) if not re.match(r"\s*#?\s*\^", l)
        )
        label = f"{source.parent.name}/{source.stem}"
        fixture = work / f"{source.parent.name}-{source.stem}{suffix}"
        fixture.write_text(text, encoding="utf-8", newline="")
        out = subprocess.run(command + [fixture.name], cwd=work, capture_output=True, text=True, check=False).stdout
        upstream = {(mapping[c], l, 0) for c, l in map(parse, json.loads(out)) if c in mapping}
        ours = {(r, l, 0) for r, l, _ in sudsy_lint(sudsy, fixture, work, set(mapping.values()))}
        total += len(upstream)
        problems = []
        for side, rows in ((tool, upstream - ours), ("sudsy", ours - upstream)):
            for rule, line, _ in sorted(rows):
                key = f"{label} {side}:{rule} {line}"
                if key in expected:
                    used.add(key)
                else:
                    problems.append(f"  only {side}: {rule} {line}")
        print(f"{label}: {len(upstream)} {tool}, {len(ours)} sudsy: {'MISMATCH' if problems else 'ok'}")
        print("\n".join(problems))
        mismatches += len(problems)
    for key in sorted(set(expected) - used):
        print(f"  stale expected difference: {key}")
        mismatches += 1
    print(f"{total} {tool} findings, {mismatches} mismatch(es) ({tool} {version}, {len(used)} known)")
    return 1 if mismatches else 0


def sudsy_lint(sudsy, fixture, cwd, only=None):
    """`{(rule, line, column)}`; with `only`, the diagnostics of those rules."""
    out = subprocess.run(
        [sudsy, "lint", "--format", "json", fixture.name],
        cwd=cwd, capture_output=True, text=True, check=False,
    )
    report = json.loads(out.stdout)
    return {
        (d["rule"], d["range"]["start"]["line"], 0 if fixture.suffix == ".md" else d["range"]["start"]["column"])
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
        work = PARITY / f"stylecop-{name}"
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
    if tool in ("hadolint", "shellcheck"):
        return rule_fixtures(tool, sudsy, version, sys.argv[4])
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    suffixes = {"eslint-jsonc": (".json",), "markdownlint": (".md",)}.get(tool, (".yaml", ".yml"))
    fixtures = sorted(p for p in folder.iterdir() if p.suffix in suffixes and p.name != "sudsy.yaml")
    eslint = {"eslint-jsonc": eslint_jsonc, "markdownlint": markdownlint}[tool](version, folder) if tool in ("eslint-jsonc", "markdownlint") else None
    mismatches = 0
    for fixture in fixtures:
        found = eslint.get(fixture.name, set()) if eslint is not None else yamllint(fixture, folder)
        # An upstream rule with no mapping can never match: report it.
        upstream = {(mapping.get(r, f"<unmapped {tool} rule {r}>"), l, c) for r, l, c in found}
        mismatches += compare(fixture.name, upstream, sudsy_lint(sudsy, fixture, folder), tool)
    print(f"{len(fixtures)} fixtures, {mismatches} mismatch(es) ({tool} {version})")
    return 1 if mismatches else 0


sys.exit(main())
