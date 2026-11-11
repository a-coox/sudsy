"""Compares an upstream tool with `sudsy lint` on tests/parity/<tool>/ (run by scripts/parity.sh).

Usage: parity_lint.py <tool> <sudsy-binary> <target-dir>

The tool's version, install and corpora come from its entry in parity_tools.json (parity_format.prepare).
"""
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import yaml

import parity_format

ROOT = Path(__file__).resolve().parent.parent
PARITY = ROOT / os.environ.get("CARGO_TARGET_DIR", "target") / "parity"


def catalog_entries(tool, sub=False):
    catalog = yaml.safe_load((ROOT / "docs/parity" / f"{parity_format.tools()['lint'][tool]['catalog']}.yaml").read_text(encoding="utf-8"))
    out = []
    for id_, value in catalog["entries"].items():  # `<status> [| <gap>]`, see docs/parity/README.md
        status, _, gap = value.partition(" | ")
        kind, _, arg = status.partition(" ")
        coverage = kind if kind in ("sdk", "wontfix", "deferred") else "missing" if kind == "planned" else "partial" if gap else "covered"
        out.append({"id": id_, "sudsy": f"{kind}: {arg}" if arg else kind, "coverage": coverage})
    for group in catalog.get("deferred", []):
        out += [{"id": i, "sudsy": "deferred", "coverage": "deferred"} for i in group["ids"]]
    return [e for e in out if sub or "." not in e["id"]]


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


def yamllint(fixture, cwd, prefix):
    """`{(upstream rule, line, column)}` of the fixture."""
    out = subprocess.run(
        [sys.executable, "-m", "yamllint", "-f", "parsable", "-c", ".yamllint", fixture.name],
        cwd=cwd, capture_output=True, text=True, check=False, env={**os.environ, "PYTHONPATH": str(prefix)},
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


def eslint_jsonc(install, folder):
    """`{fixture name: {(upstream rule, line, column)}}` of the `.json` fixtures. ESLint and the plugin
    are installed in `install`; the fixtures and the flat config are copied below it so that the
    config finds the plugin."""
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


def markdownlint(install, folder):
    """`{fixture name: {(upstream rule, line, 0)}}` of the `.md` fixtures (markdownlint reports a column
    for some rules only, so the column is left out of the comparison; Sudsy's is zeroed alike).
    markdownlint-cli2 is installed in `install`; the fixtures and their `.markdownlint.jsonc` are
    copied below it."""
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


def rule_fixtures(tool, sudsy, version, prefix, exe):
    """Runs the tool (hadolint, shellcheck, checkmake, stylelint) and `sudsy lint` on the rule fixtures of its language
    (tests/rules/<lang>/<folder>/*<suffix>, annotations removed) and compares `(rule, line)` of the
    catalog's rules: upstream codes the catalog does not map are left out.
    `tests/parity/<tool>/differences.yaml` lists known differences under the key
    `<folder>/<name> <tool|sudsy>:<rule> <line>`; one that no longer differs fails."""
    lang, suffix, command, parse, load = {
        "hadolint": ("dockerfile", ".dockerfile", [exe, "--no-fail", "--no-color", "-f", "json"],
                     lambda d: (d["code"], d["line"]), json.loads),
        "shellcheck": ("shell", ".sh", [exe, "-s", "bash", "-o", "all", "-f", "json"],
                       lambda d: (f"SC{d['code']}", d["line"]), json.loads),
        # one JSON object per line; --path limits the findings to the fixture, the rest of the module is its imports
        "buf": ("protobuf", ".proto", [exe, "lint", "--error-format=json", "--path"],
                lambda d: (d["type"], d["start_line"]), lambda out: [json.loads(l) for l in out.splitlines()]),
        # shellcheck and pyflakes are the shell and python checks of the scripts, which Sudsy's Shell language does
        "actionlint": ("github-actions", ".yml", [exe, "-no-color", "-format", "{{json .}}", "-shellcheck=", "-pyflakes="],
                       lambda d: (actionlint_check(d), d["line"]), lambda out: json.loads(out) or []),
        # flat config of the catalog's rules (written below); espree parses .js, the TypeScript syntax is typescript-eslint's later
        "eslint": ("js", ".js", ["node", f"{prefix}/node_modules/eslint/bin/eslint.js", "-c", "eslint.config.mjs", "-f", "json"],
                   lambda d: (d["ruleId"], d["line"]), lambda out: [m for r in json.loads(out) for m in r["messages"] if m["ruleId"]]),
        # stylelint writes its JSON report to stderr; the plugins and the dialect parser come from the config written below
        **{t: (lang, "." + lang, ["node", f"{prefix}/node_modules/stylelint/bin/stylelint.mjs", "--config", "stylelint.config.mjs", "-f", "json"],
               lambda d: (d["rule"], d["line"]), lambda out: [w for r in json.loads(out) for w in r["warnings"]])
           for t, lang in (("stylelint", "css"), ("stylelint-scss", "scss"), ("stylelint-less", "less"))},
        "checkmake": ("makefile", ".mk", [exe, "--format", '{"rule":"{{.Rule}}","line":{{.LineNumber}}}'],
                      lambda d: (d["rule"], d["line"]),
                      lambda out: [json.loads(l) for l in out.splitlines() if l.startswith("{")]),
    }[tool]
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    work = prefix / "work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    if tool == "eslint":
        (work / "eslint.config.mjs").write_text(f"export default [{{ rules: {json.dumps({r: 'error' for r in mapping})} }}];\n", encoding="utf-8")
    if tool.startswith("stylelint"):
        plugin = {"stylelint-scss": 'plugins: ["stylelint-scss"], customSyntax: "postcss-scss", ', "stylelint-less": 'plugins: ["stylelint-less"], customSyntax: "postcss-less", '}.get(tool, "")
        # every mapped rule runs with `true`, or with its options from `rules.json` (which `sudsy.yaml` mirrors)
        options = json.loads((folder / "rules.json").read_text(encoding="utf-8")) if (folder / "rules.json").exists() else {}
        (work / "stylelint.config.mjs").write_text(f"export default {{ {plugin}rules: {json.dumps({r: options.get(r, True) for r in mapping})} }};\n", encoding="utf-8")
    for config in ("sudsy.yaml", "buf.yaml"):
        if (folder / config).exists():
            shutil.copy(folder / config, work / config)
    diffs = parity_format.Differences(folder, lint=True)
    mismatches, total = 0, 0
    sources = [(f"{s.parent.name}/{s.stem}", f"{s.parent.name}-{s.stem}{suffix}", s) for s in sorted((ROOT / "tests/rules" / lang).glob(f"*/*{suffix}"))
               if not s.with_suffix(".rules.yaml").exists()]  # a case that sets rule options: both tools run with their defaults here
    if tool == "actionlint":  # the corpus too (tests/conformance/fetch-github-actions-corpus.sh), a file per name
        corpus = ROOT / "tests/conformance/corpus/github-actions"
        sources += [(f"corpus/{p.relative_to(corpus).as_posix()}", "-".join(p.relative_to(corpus).parts), p) for p in sorted(corpus.rglob("*.y*ml")) if p.is_file()]
    if tool == "eslint":  # the real-world corpus (tests/conformance/fetch-js-corpus.sh); Prettier's own tests include proposal syntax espree rejects
        corpus = ROOT / "tests/conformance/corpus/js"
        sources += [(f"corpus/{p.relative_to(corpus).as_posix()}", "-".join(p.relative_to(corpus).parts), p) for p in sorted(corpus.rglob("*.js")) if p.is_file() and p.relative_to(corpus).parts[0] != "prettier"]
    if not mapping:  # nothing to compare yet
        print(f"{len(sources)} {tool} fixtures, no catalog rule is mapped to a Sudsy rule yet ({tool} {version})")
        return 0
    # A workflow is claimed by its folder.
    where = work / ".github" / "workflows" if tool == "actionlint" else work
    where.mkdir(parents=True, exist_ok=True)
    for label, name, source in sources:
        text = "".join(
            l for l in source.read_text(encoding="utf-8").splitlines(keepends=True) if not re.match(r"\s*(?:#|//)?\s*\^", l)
        )
        if tool == "eslint":  # both tools read a `.js` as a module by default: a file with `import`/`export` is one, the others are scripts
            name = str(Path(name).with_suffix(".mjs" if re.search(r"^(?:import|export)\b", text, re.M) else ".cjs"))
        fixture = where / name
        fixture.write_text(text, encoding="utf-8", newline="")
        rel = fixture.relative_to(work).as_posix()
        run = subprocess.run(command + [rel], cwd=work, capture_output=True, text=True, check=False)
        out = run.stdout or run.stderr  # stylelint reports on stderr
        # checkmake runs every rule on every file: a fixture is compared on the rule of its folder, and
        # checkmake reports a variable (and `.PHONY`, which it reads as one) a line late.
        late = {"timestampexpanded", "minphony"} if tool == "checkmake" else set()
        # ESLint reports a column; the other tools' columns are not compared
        found = [(parse(d), d) for d in load(out)]
        upstream = {(mapping[c], l - (c in late), d["column"] if tool == "eslint" else 0) for (c, l), d in found if c in mapping}
        ours = {(r, l, c if tool == "eslint" else 0) for r, l, c in sudsy_lint(sudsy, fixture, work, set(mapping.values()))}
        if tool == "checkmake":
            upstream, ours = ({row for row in side if row[0] == source.parent.name} for side in (upstream, ours))
        fixture.unlink()  # buf compiles the whole folder: fixtures that share a package must not meet
        total += len(upstream)
        problems = []
        for side, rows in ((tool, upstream - ours), ("sudsy", ours - upstream)):
            for rule, line, column in sorted(rows):
                key = f"{label} {side}:{rule} {line}"
                if not diffs.known(key):
                    problems.append(f"  only {side}: {rule} {line}" + (f":{column}" if column else ""))
        print(f"{label}: {len(upstream)} {tool}, {len(ours)} sudsy: {'MISMATCH' if problems else 'ok'}")
        print("\n".join(problems))
        mismatches += len(problems)
    mismatches += diffs.finish()
    print(f"{total} {tool} findings, {mismatches} mismatch(es) ({tool} {version}, {sum(k in diffs.files for k in diffs.differing)} known)")
    return 1 if mismatches else 0


def actionlint_check(d):
    """The catalog id of an actionlint error: its kind, split by message where one kind holds several checks."""
    kind, message = d["kind"], d["message"]
    if kind == "expression":
        if "potentially untrusted" in message:
            return "expression/untrusted"
        if re.match(r"got unexpected|unexpected end|undefined (variable|function)|expected|.*while (lexing|parsing)", message):
            return "expression/syntax"
        return "expression/types"
    if kind == "id":
        return "id/duplicate" if "duplicate" in message else "id/job" if "job ID" in message else "id/step"
    if kind == "job-needs":
        return "job-needs/cycle" if "cyclic" in message else "job-needs/unknown" if "does not exist" in message else "job-needs/duplicate"
    if kind == "runner-label":
        return "runner-label/unknown" if "unknown" in message else "runner-label/conflict"
    if kind == "syntax-check" and "is duplicated" in message:
        return "syntax-check/duplicate-job" if 'in "jobs" section' in message else "syntax-check/duplicate-key"
    return kind


def psscriptanalyzer(sudsy, version, prefix):
    """Invoke-ScriptAnalyzer against `sudsy lint` (preset powershell/psscriptanalyzer) on the PowerShell rule
    fixtures (tests/rules/powershell/*/*.ps1, annotations removed) and the PowerShell corpus, as
    `(file, rule, line)` of the rules the catalog maps. Also checks that the catalog lists exactly the rules
    of the installed module. `tests/parity/psscriptanalyzer/differences.yaml` keys:
    `<file> <psscriptanalyzer|sudsy>:<rule> <line>` (a fixture is `<folder>-<name>.ps1`, a corpus file
    `corpus/<path>`); one that no longer differs fails."""
    script = ROOT / "scripts/parity_psscriptanalyzer.ps1"
    pwsh = lambda *args: subprocess.run(["pwsh", "-NoProfile", "-File", str(script), str(prefix), *args],
                                        capture_output=True, text=True, encoding="utf-8", check=True).stdout
    # The catalog lists the module's rules.
    module = dict(l.split() for l in pwsh("rules").splitlines())
    catalog = {e["id"] for e in catalog_entries("psscriptanalyzer")}
    mismatches = 0
    for rule in sorted(set(module) ^ catalog):
        print(f"  catalog and module differ: {rule} ({'catalog only' if rule in catalog else 'module only'})")
        mismatches += 1
    preset = yaml.safe_load((ROOT / "crates/lang-powershell/presets/psscriptanalyzer.yaml").read_text(encoding="utf-8"))
    mapping = {k: v for k, v in rule_map("psscriptanalyzer").items() if v in preset["powershell"]["lint"]["rules"]}
    folder = ROOT / "tests/parity/psscriptanalyzer"
    work = prefix / "work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    corpus = ROOT / "tests/conformance/corpus/powershell"
    for source in sorted((ROOT / "tests/rules/powershell").glob("*/*.ps1")):
        text = "".join(
            l for l in source.read_text(encoding="utf-8").splitlines(keepends=True) if not re.match(r"\s*#?\s*\^", l)
        )
        (work / f"{source.parent.name}-{source.stem}.ps1").write_text(text, encoding="utf-8", newline="")
    shutil.copytree(corpus, work / "corpus")
    diffs = parity_format.Differences(folder, lint=True)
    key_of = lambda p: Path(p).resolve().relative_to(work.resolve()).as_posix()
    upstream = {(key_of(d["file"]), mapping[d["rule"]], d["line"])
                for d in json.loads(pwsh("run", ",".join(mapping), str(work)) or "[]") if d["rule"] in mapping}
    out = subprocess.run([sudsy, "lint", "--format", "json", "."], cwd=work, capture_output=True, text=True,
                         encoding="utf-8", check=False).stdout
    ours = {(d["path"].replace("\\", "/"), d["rule"], d["range"]["start"]["line"])
            for d in json.loads(out)["diagnostics"] if d["rule"] in set(mapping.values())}
    for rule, count in sorted((r, sum(1 for u in upstream if u[1] == r)) for r in set(mapping.values())):
        print(f"{rule}: {count} psscriptanalyzer, {sum(1 for o in ours if o[1] == rule)} sudsy")
    for side, rows in (("psscriptanalyzer", upstream - ours), ("sudsy", ours - upstream)):
        for path, rule, line in sorted(rows):
            key = f"{path} {side}:{rule} {line}"
            if not diffs.known(key):
                src = (work / path).read_text(encoding="utf-8-sig", errors="replace").splitlines()[line - 1].strip()[:90]
                print(f"  only {side}: {key}  | {src}")
                mismatches += 1
    mismatches += diffs.finish()
    print(f"{len(upstream)} psscriptanalyzer findings, {mismatches} mismatch(es) (psscriptanalyzer {version}, {sum(k in diffs.files for k in diffs.differing)} known)")
    return 1 if mismatches else 0


def sudsy_lint(sudsy, fixture, cwd, only=None):
    """`{(rule, line, column)}`; with `only`, the diagnostics of those rules."""
    out = subprocess.run(
        [sudsy, "lint", "--format", "json", fixture.relative_to(cwd).as_posix()],
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
    tool, sudsy, target = sys.argv[1:4]
    config = parity_format.tools()["lint"][tool]
    version, prefix = parity_format.prepare(config, Path(target).resolve() / "parity")
    if tool == "stylecop":
        return stylecop(sudsy, version)
    if tool == "psscriptanalyzer":
        return psscriptanalyzer(sudsy, version, prefix)
    if tool in ("hadolint", "shellcheck", "checkmake", "actionlint", "buf", "eslint", "stylelint", "stylelint-scss", "stylelint-less"):
        exe = parity_format.tools()["format"][config["install"]] if isinstance(config["install"], str) else config
        return rule_fixtures(tool, sudsy, version, prefix, str(parity_format.executable(prefix, exe["install"].get("exe", ""))))
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    suffixes = {"eslint-jsonc": (".json",), "markdownlint": (".md",)}.get(tool, (".yaml", ".yml"))
    fixtures = sorted(p for p in folder.iterdir() if p.suffix in suffixes and p.name != "sudsy.yaml")
    eslint = {"eslint-jsonc": eslint_jsonc, "markdownlint": markdownlint}[tool](prefix, folder) if tool in ("eslint-jsonc", "markdownlint") else None
    mismatches = 0
    for fixture in fixtures:
        found = eslint.get(fixture.name, set()) if eslint is not None else yamllint(fixture, folder, prefix)
        # An upstream rule with no mapping can never match: report it.
        upstream = {(mapping.get(r, f"<unmapped {tool} rule {r}>"), l, c) for r, l, c in found}
        mismatches += compare(fixture.name, upstream, sudsy_lint(sudsy, fixture, folder), tool)
    print(f"{len(fixtures)} fixtures, {mismatches} mismatch(es) ({tool} {version})")
    return 1 if mismatches else 0


sys.exit(main())
