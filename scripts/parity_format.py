"""Compares `sudsy format` with an upstream formatter on tests/parity/<tool>/ (run by `scripts/parity.sh format`).

Usage: parity_format.py <tool> <sudsy-binary> <target-dir>

The tools are the entries of parity_format.json (documented in docs/development/parity.md): version
(from docs/parity/<catalog>.yaml), how to install it, its command, how it takes its input and which
files are the fixtures. Every fixture is formatted by the tool and by Sudsy (with the fixture
folder's sudsy.yaml, which extends the matching preset); the outputs must be equal. A fixture listed
in the folder's `differences.yaml` may differ; it is reported as
known; the file is `differences.yaml` (docs/development/parity.md): named differences, the fixtures that
differ, and the `ratchet` minimums of the groups (a corpus too large for entries per file) that are only counted.
"""
import difflib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent


def run(cmd, cwd, stdin=None, check=True):
    data = stdin.encode("utf-8") if stdin is not None else None
    cmd = [shutil.which(cmd[0]) or cmd[0], *map(str, cmd[1:])]
    r = subprocess.run(cmd, cwd=cwd, input=data, capture_output=True, check=False)
    if check and r.returncode != 0:
        raise SystemExit(f"{' '.join(map(str, cmd))} failed:\n{r.stderr.decode()}{r.stdout.decode()}")
    return r


def tools():
    return json.loads((ROOT / "scripts/parity_tools.json").read_text())


def catalog_version(name):
    text = (ROOT / "docs/parity" / f"{name}.yaml").read_text(encoding="utf-8")
    return re.search(r'^version: "([^ "]+)', text, re.M).group(1)


def executable(prefix, name):
    exe = prefix / f"{name}.exe"
    return exe if exe.exists() else prefix / name


def install(spec, catalog, version, prefix):
    """Installs the tool into prefix unless it is there."""
    if "npm" in spec:
        wanted = [spec["npm"], *(w.rsplit("@", 1)[0] for w in spec.get("with", []))]
        if not all((prefix / "node_modules" / w).is_dir() for w in wanted):
            prefix.mkdir(parents=True, exist_ok=True)
            run(["npm", "install", "--silent", "--no-audit", "--no-fund", "--prefix", prefix, *spec.get("with", []), f"{spec['npm']}@{version}"], ROOT)
    elif "pip" in spec:  # a Python tool, run with PYTHONPATH=prefix
        if not (prefix / spec["pip"]).is_dir():
            run([sys.executable, "-m", "pip", "install", "--quiet", "--target", prefix, f"{spec['pip']}=={version}"], ROOT)
    elif "dotnet-tool" in spec:
        if not executable(prefix, spec["exe"]).exists():
            run(["dotnet", "tool", "install", spec["dotnet-tool"], "--version", version, "--tool-path", prefix], ROOT)
    elif "psmodule" in spec:
        if not (prefix / spec["psmodule"]).is_dir():
            prefix.mkdir(parents=True, exist_ok=True)
            run(["pwsh", "-NoProfile", "-Command", f"Save-Module {spec['psmodule']} -RequiredVersion {version} -Path '{prefix}'"], ROOT)
    elif "release" in spec:
        if not executable(prefix, spec["exe"]).exists():
            prefix.mkdir(parents=True, exist_ok=True)
            system = {"win32": "windows", "linux": "linux", "darwin": "darwin"}.get(sys.platform, sys.platform)
            key = system if system == "windows" else f"{system}-{platform.machine().lower()}"
            asset = (spec["assets"].get(key) or sys.exit(f"parity: no {catalog} release for {key}")).format(version=version)
            archive = prefix / asset
            run(["curl", "-fsSL", "-o", archive, spec["release"].format(version=version, asset=asset)], ROOT)
            if asset.endswith((".zip", ".tar.gz", ".tar.xz")):
                shutil.unpack_archive(archive, prefix)
                if not executable(prefix, spec["exe"]).exists():  # inside a folder of the archive
                    found = next(p for n in (spec["exe"] + ".exe", spec["exe"]) for p in prefix.rglob(n))
                    found.replace(prefix / found.name)
                    (prefix / found.name).chmod(0o755)
            elif asset.endswith(".gz"):
                target = prefix / spec["exe"]
                run(["gunzip", "-f", archive], ROOT)
                (prefix / asset[: -len(".gz")]).rename(target)
                target.chmod(0o755)
            else:  # the asset is the executable
                target = prefix / (spec["exe"] + (".exe" if asset.endswith(".exe") else ""))
                archive.replace(target)
                target.chmod(0o755)


def fixtures(config, folder):
    """The fixtures as (name, path, ratchet group or None); names are the keys of differences.yaml."""
    out = []
    for source in config["fixtures"]:
        base = ROOT / source["dir"] if "dir" in source else folder
        suffixes = tuple(source["suffixes"])
        if "list" in source:
            listing = base / source["list"]
            if not listing.exists():
                print(f"{source['dir']} not fetched: its files are skipped")
                continue
            paths = [(rel, base / rel) for rel in listing.read_text().splitlines() if rel.startswith(source["starts"])]
        else:
            walk = base.rglob("*") if source.get("recursive") else base.iterdir()
            paths = [(p.relative_to(base).as_posix(), p) for p in sorted(walk) if p.is_file() and ".git" not in p.parts]
        for rel, path in paths:
            if not rel.endswith(suffixes) or path.name == "sudsy.yaml" or rel.startswith(source.get("skip", "\0")) or not rel.startswith(source.get("only", "")):
                continue
            try:
                path.read_bytes().decode("utf-8")
            except UnicodeDecodeError:
                continue
            out.append((source.get("prefix", "") + rel, path, source.get("ratchet")))
    return out


def upstream(config, prefix, version, copy, work):
    """The tool's output for the copy of a fixture, or None when it rejects the file."""
    exe = executable(prefix, config.get("install", {}).get("exe", ""))
    mode = config["input"]
    text = copy.read_bytes().decode("utf-8")
    if mode == "in-place":  # the tool rewrites its file: it gets a copy of the copy
        copy = work / "theirs" / copy.name
        copy.parent.mkdir(exist_ok=True)
        shutil.copy(work / copy.name, copy)
    command = [str(a).format(prefix=prefix, exe=exe, file=copy.name, root=ROOT) for a in config["command"]]
    r = run(command, copy.parent if mode == "in-place" else work, text if mode == "stdin" else None, check=False)
    if r.returncode != 0:
        if config.get("tool_may_reject"):
            return None
        raise SystemExit(f"{' '.join(command)} failed:\n{r.stderr.decode()}{r.stdout.decode()}")
    return copy.read_bytes().decode("utf-8") if mode == "in-place" else r.stdout.decode("utf-8")


def sudsy_format(sudsy, fixture, work, command="format"):
    r = run([sudsy, command, fixture.name], work, check=False)
    return fixture.read_bytes().decode("utf-8") if r.returncode == 0 else None  # None: Sudsy rejects the file


class Differences:
    """`tests/parity/<tool>/differences.yaml` (docs/development/parity.md): `differences` (name -> `<choice|gap>  <text>`),
    `files` (fixture key -> names; a ratchet group's name stands for all its files) and `ratchet` (group -> least identical files).
    `lint`: the keys with a space are the lint mode's (a tool's two modes share the file)."""

    def __init__(self, folder, lint=False):
        path = folder / "differences.yaml"
        data = (yaml.safe_load(path.read_text(encoding="utf-8")) if path.exists() else None) or {}
        self.defs = {n: v.split(None, 1) for n, v in (data.get("differences") or {}).items()}
        self.files = {k: [n.strip() for n in v.split(",")] for k, v in (data.get("files") or {}).items()}
        self.ratchet = data.get("ratchet") or {}
        self.lint = lint
        self.differing = {}  # key -> files that differ, listed or not

    def known(self, key, count=1):
        """Records that `key` differs; True when it is listed."""
        self.differing[key] = count
        return key in self.files

    def finish(self, filtered=False):
        """Prints the problems of the lists and the report; returns the number of problems."""
        bad = 0
        used = {n for names in self.files.values() for n in names}
        for n in sorted(used - set(self.defs)):
            print(f"UNDEFINED difference name: {n}")
            bad += 1
        for n in sorted(set(self.defs) - used):
            print(f"UNUSED difference name: {n}")
            bad += 1
        for n, (kind, _) in self.defs.items():
            if kind not in ("choice", "gap"):
                print(f"difference {n}: the kind is `choice` or `gap`, not `{kind}`")
                bad += 1
        if not filtered:
            for key in self.files:
                if (" " in key) == self.lint and key not in self.differing:
                    print(f"STALE: {key} is listed but no longer differs")
                    bad += 1
        counts = Counter()
        for key, count in self.differing.items():
            for n in self.files.get(key, []):
                counts[n] += count
        for kind in ("choice", "gap"):
            row = ", ".join(f"{n} {counts[n]}" for n, (k, _) in self.defs.items() if k == kind and counts[n])
            print(f"{kind}: {row or 'none'}")
        return bad


def verdict(name, theirs, ours, diffs, label, quiet=False):
    """Prints the result of one fixture; returns 1 for a mismatch. `quiet`: no line for an equal pair, no diff."""
    if theirs is not None and theirs == ours:
        if not quiet:
            print(f"{name}: ok")
        return 0
    if diffs.known(name):
        print(f"{name}: known difference ({', '.join(diffs.files[name])})")
        return 0
    print(f"{name}: DIFFERENT (the tool rejects the file)" if theirs is None else f"{name}: DIFFERENT")
    if theirs is not None and not quiet:
        sys.stdout.writelines(difflib.unified_diff(theirs.splitlines(True), (ours or "").splitlines(True), label, "sudsy"))
    return 1


def editorconfig_cases(version, sudsy, parity):
    """One case per `<case>.editorconfig` of tests/parity/dotnet-format: `<case>.cs`, else the
    shared `input.cs`, formatted by `dotnet format whitespace` and by Sudsy in a folder that holds the
    `.editorconfig` and the folder's sudsy.yaml."""
    folder = ROOT / "tests/parity/dotnet-format"
    known = Differences(folder)
    cases = sorted(p.stem for p in folder.glob("*.editorconfig"))
    bad = 0
    for name in cases:
        outputs = []
        for side in ("dotnet-format", "sudsy"):
            case = parity / f"dotnet-format-work/{side}/{name}"
            shutil.rmtree(case, ignore_errors=True)
            case.mkdir(parents=True)
            shutil.copy(folder / f"{name}.editorconfig", case / ".editorconfig")
            own = folder / f"{name}.cs"
            shutil.copy(own if own.exists() else folder / "input.cs", case / "input.cs")
            shutil.copy(folder / "sudsy.yaml", case / "sudsy.yaml")
            if side == "sudsy":
                outputs.append(sudsy_format(sudsy, case / "input.cs", case))
            else:
                run(["dotnet", "format", "whitespace", case, "--folder"], case)
                outputs.append((case / "input.cs").read_bytes().decode("utf-8"))
        bad += verdict(name, *outputs, known, "dotnet-format")
    bad += known.finish()
    print(f"{len(cases)} cases, {bad} mismatch(es) (dotnet format {version})")
    return bad


def prepare(config, parity):
    """Builds what the entry needs, fetches its corpora and installs the tool; returns (version, prefix)."""
    if "worker" in config.get("needs", []):
        run(["dotnet", "build", "--nologo", "-v:q", "workers/csharp/Sudsy.Worker"], ROOT)
        os.environ.setdefault("SUDSY_CSHARP_WORKER", str(ROOT / "workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll"))
    for script in config.get("fetch", []):
        run(["bash", script], ROOT)
    if "catalog" not in config:
        return run(config["version_command"], ROOT).stdout.decode().strip(), None
    version = catalog_version(config["catalog"])
    prefix = parity / f"{config['catalog']}-{version}"
    spec = config.get("install")
    if spec:  # a name: the install of that format entry
        install(tools()["format"][spec]["install"] if isinstance(spec, str) else spec, config["catalog"], version, prefix)
    return version, prefix


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    tool, sudsy, target = sys.argv[1:4]
    config = tools()["format"][tool]
    parity = Path(target).resolve() / "parity"
    version, prefix = prepare(config, parity)
    if config.get("kind") == "editorconfig-cases":
        return 1 if editorconfig_cases(version, sudsy, parity) else 0
    folder = ROOT / "tests/parity" / tool
    work = parity / f"{tool}-work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    known = Differences(folder)
    only = os.environ.get("PARITY_FILTER")  # a name substring: those fixtures only, with their diffs, no ratchet
    found = [f for f in fixtures(config, folder) if only is None or only in f[0]]
    quiet = bool(config.get("quiet")) and only is None

    def compare(item):
        name, fixture, _ = item
        copy = work / name.replace("/", "__")
        shutil.copy(fixture, copy)
        theirs, ours = upstream(config, prefix, version, copy, work), sudsy_format(sudsy, copy, work, config.get("sudsy_command", "format"))
        if config.get("ignore_trailing_blanks"):  # the tool keeps the trailing blanks of a line, Sudsy never does
            theirs, ours = (re.sub(r"[ 	]+$", "", t, flags=re.M) if t is not None else None for t in (theirs, ours))
        return theirs, ours

    with ThreadPoolExecutor(os.cpu_count() if config.get("parallel") else 1) as pool:
        results = list(pool.map(compare, found))
    bad, groups = 0, {}  # a ratchet group is counted: [identical, different, rejected by the tool]
    for (name, _, group), (theirs, ours) in zip(found, results):
        if group and only is None:
            tally = groups.setdefault(group, [0, 0, 0])
            tally[2 if theirs is None else 0 if theirs == ours else 1] += 1
        else:
            bad += verdict(name, theirs, ours, known, tool, quiet)
    for group, (same, different, rejected) in sorted(groups.items()):
        floor = known.ratchet.get(group, 0)
        if different:
            known.known(group, different)
        note = "BELOW the minimum" if same < floor else f"raise the minimum in differences.yaml to {same}" if same > floor else "at the minimum"
        print(f"ratchet {group}: {same} identical, {different} different, {rejected} rejected by {tool} ({note}: {floor})")
        bad += same < floor
    bad += known.finish(only is not None)
    print(f"{len(found)} fixtures, {bad} mismatch(es) ({tool} {version})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
