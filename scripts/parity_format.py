"""Compares `sudsy format` with an upstream formatter on tests/parity/<tool>/ (run by `scripts/parity.sh format`).

Usage: parity_format.py <tool> <sudsy-binary> <target-dir>

The tools are the entries of parity_format.json (documented in docs/development/parity.md): version
(from docs/parity/<catalog>.yaml), how to install it, its command, how it takes its input and which
files are the fixtures. Every fixture is formatted by the tool and by Sudsy (with the fixture
folder's sudsy.yaml, which extends the matching preset); the outputs must be equal. A fixture listed
in the folder's `expected-diff.txt` (`<fixture>: <reason>` per line) may differ; it is reported as
known, and as stale when the outputs are in fact equal.
"""
import difflib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

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
        if not (prefix / "node_modules" / spec["npm"]).is_dir():
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
    """The fixtures as (name, path); names are the keys of expected-diff.txt."""
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
            if not rel.endswith(suffixes) or path.name == "sudsy.yaml":
                continue
            try:
                path.read_bytes().decode("utf-8")
            except UnicodeDecodeError:
                continue
            out.append((source.get("prefix", "") + rel, path))
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


def expected_diffs(folder):
    path = folder / "expected-diff.txt"
    out = {}
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip() and not line.startswith("#"):
                name, _, reason = line.partition(":")
                out[name.strip()] = reason.strip()
    return out


def verdict(name, theirs, ours, known, label):
    """Prints the result of one fixture; returns 1 for a mismatch."""
    same = theirs == ours
    if theirs is None:
        print(f"{name}: known difference ({known[name]})" if name in known else f"{name}: DIFFERENT (the tool rejects the file)")
        return name not in known
    if same and name in known:
        print(f"{name}: STALE expected-diff entry (outputs are equal)")
        return 1
    if same:
        print(f"{name}: ok")
        return 0
    if name in known:
        print(f"{name}: known difference ({known[name]})")
        return 0
    print(f"{name}: DIFFERENT")
    sys.stdout.writelines(difflib.unified_diff((theirs or "").splitlines(True), (ours or "").splitlines(True), label, "sudsy"))
    return 1


def editorconfig_cases(version, sudsy, parity):
    """One case per `<case>.editorconfig` of tests/parity/dotnet-format: `<case>.cs`, else the
    shared `input.cs`, formatted by `dotnet format whitespace` and by Sudsy in a folder that holds the
    `.editorconfig` and the folder's sudsy.yaml."""
    folder = ROOT / "tests/parity/dotnet-format"
    known = expected_diffs(folder)
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
    known = expected_diffs(folder)
    found = fixtures(config, folder)
    bad = 0
    for name, fixture in found:
        copy = work / name.replace("/", "__")
        shutil.copy(fixture, copy)
        theirs, ours = upstream(config, prefix, version, copy, work), sudsy_format(sudsy, copy, work, config.get("sudsy_command", "format"))
        if config.get("ignore_trailing_blanks"):  # the tool keeps the trailing blanks of a line, Sudsy never does
            theirs, ours = (re.sub(r"[ 	]+$", "", t, flags=re.M) if t is not None else None for t in (theirs, ours))
        bad += verdict(name, theirs, ours, known, tool)
    print(f"{len(found)} fixtures, {bad} mismatch(es) ({tool} {version})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
