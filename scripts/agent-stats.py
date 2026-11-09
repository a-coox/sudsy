"""Where agents spend time and what fails: a summary of Claude Code subagent transcripts.

Usage: agent-stats.py [<subagents dir>] [N newest agents, default 10]
Default dir: the newest session with subagents under ~/.claude/projects/<this repo>.
Per agent: model vs tool minutes, gate/bench/parity launches, watchers, errors, and the commit whose
CLAUDE.md the agent was given ("?" when it matches none). Then minutes and errors per command kind,
the slowest calls, and recurring error lines (the worktree guard's refusals grouped by reason).
"""
import collections
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IDLE = 600  # a gap longer than this is the agent paused (rate limit, coordinator), not working
KINDS = [('gate', r'scripts/gate\.sh'), ('bench', r'scripts/bench\.sh'), ('parity', r'scripts/parity\.sh'),
         ('cargo test', r'cargo (nextest|test)'), ('cargo build/clippy', r'cargo (check|clippy|build)'),
         ('cargo other', r'\bcargo '), ('git', r'^git\b'), ('wait loop', r'^(sleep|until|while)\b'),
         ('python', r'^python'), ('read/search', r'^(cat|head|tail|sed -n|grep|rg|ls|find|wc)\b'), ('sed edit', r'^sed -i')]
FAILED = re.compile(r'Exit code [1-9]|GATE FAILED|BENCH: regression|^error(\[E\d+\])?:|panicked|Blocked:|Refusing', re.M)


def ts(s):
    return datetime.fromisoformat(s.replace('Z', '+00:00'))


def text(content):
    if isinstance(content, str):
        return content
    return '\n'.join(b.get('text', '') for b in content or [] if isinstance(b, dict))


def kind(name, inp):
    if name not in ('Bash', 'PowerShell'):
        return name
    cmd = re.sub(r'^(cd \S+ (&&|;) *|export \S+ (&&|;) *|[A-Z_]+=\S+ +)+', '', inp.get('command', '').strip())
    return next((k for k, p in KINDS if re.search(p, cmd)), 'shell other')


def claude_md_commits():
    """Blob hash of CLAUDE.md -> the first commit that has it."""
    log = subprocess.run(['git', 'log', '--reverse', '--format=%h', '--', 'CLAUDE.md'], cwd=ROOT, capture_output=True, text=True).stdout.split()
    blobs = {}
    for c in log:
        blob = subprocess.run(['git', 'rev-parse', f'{c}:CLAUDE.md'], cwd=ROOT, capture_output=True, text=True).stdout.strip()
        blobs.setdefault(blob, c)
    return blobs


def blob_of(md):
    data = md.encode('utf-8')
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


def analyze(path, blobs):
    recs = [json.loads(l) for l in path.open(encoding='utf-8')]
    md_commit = '-'
    for r in recs:  # the CLAUDE.md the agent got rides in its first system-reminder
        m = re.search(r'Sudsy\\CLAUDE\.md \(project instructions[^\n]*\n\n(.*?)\n*(?=\n\nContents of |</system-reminder>)', text(r.get('message', {}).get('content')), re.S)
        if m:
            md_commit = next((blobs[b] for b in (blob_of(m.group(1) + '\n'), blob_of(m.group(1))) if b in blobs), '?')
            break
    recs = [r for r in recs if 'uuid' in r and 'timestamp' in r and r.get('type') in ('user', 'assistant')]
    a = collections.Counter(model=0, tool=0)
    uses, calls, prev = {}, [], None
    for r in recs:
        t = ts(r['timestamp'])
        if prev:
            gap = (t - prev).total_seconds()
            if gap <= IDLE:
                a['model' if r['type'] == 'assistant' else 'tool'] += gap / 60
        prev = t
        content = r['message'].get('content')
        if not isinstance(content, list):
            continue
        for b in content:
            if b.get('type') == 'tool_use':
                uses[b['id']] = (t, b['name'], b.get('input') or {})
            elif b.get('type') == 'tool_result' and b['tool_use_id'] in uses:
                t0, name, inp = uses.pop(b['tool_use_id'])
                out = text(b.get('content'))
                err = b.get('is_error') or FAILED.search(out[:3000] + out[-3000:])
                cmd = inp.get('command') or inp.get('file_path') or inp.get('description') or ''
                calls.append(dict(kind=kind(name, inp), secs=(t - t0).total_seconds(), err=bool(err), cmd=re.sub(r'\s+', ' ', cmd)[:110],
                                  errline=first_error(out) if err else ''))
                if name == 'Monitor' or re.match(r'\s*(until|while|sleep)\b', inp.get('command', '')):
                    a['watch'] += 1
                c = inp.get('command', '')
                for k in ('gate', 'bench', 'parity'):
                    if re.search(rf'scripts/{k}\.sh', c) and not re.search(r'until|grep -q', c):
                        a[k] += 1
                        a['full gate'] += k == 'gate' and not re.search(r'gate\.sh[^;&|>]*-p ', c)
    return a, calls, md_commit


def first_error(out):
    m = re.search(r'This agent is isolated in the worktree \S+, but (this command [^,;.(]*)', out)
    if m:
        return 'worktree guard: ' + m.group(1)[:90]
    lines = [l for l in out.splitlines() if FAILED.search(l) or re.search(r'error|not found|No such', l, re.I)]
    return re.sub(r'\d+', 'N', (lines[0] if lines else out.strip()[-120:]))[:120]


def default_dir():
    common = subprocess.run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, capture_output=True, text=True).stdout
    slug = re.sub(r'[^a-z0-9]', '-', str(Path(common.strip()).parent.resolve()).lower())  # the main checkout, also from a worktree
    projects = [p for p in (Path.home() / '.claude/projects').iterdir() if p.name.lower() == slug]
    dirs = [d for p in projects for d in p.glob('*/subagents')]
    if not dirs:
        sys.exit(f'no subagent transcripts under ~/.claude/projects/{slug}')
    return max(dirs, key=lambda d: d.stat().st_mtime)


def main():
    folder = Path(sys.argv[1]) if len(sys.argv) > 1 else default_dir()
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    files = sorted(folder.glob('*.jsonl'), key=lambda p: p.stat().st_mtime)[-n:]
    blobs = claude_md_commits()
    print(f'{folder}\n\nagent     CLAUDE.md  model  tool | gate full bench parity | watch errors')
    by_kind = collections.defaultdict(lambda: [0, 0.0, 0])
    errors, slow = collections.Counter(), []
    for f in files:
        a, calls, md = analyze(f, blobs)
        name = f.stem.removeprefix('agent-')[:8]
        print(f"{name}  {md:>9} {a['model']:5.0f}m {a['tool']:4.0f}m | {a['gate']:4} {a['full gate']:4} {a['bench']:5} {a['parity']:6} |"
              f" {a['watch']:5} {sum(c['err'] for c in calls):6}")
        for c in calls:
            k = by_kind[c['kind']]
            k[0] += 1
            k[1] += c['secs'] / 60
            k[2] += c['err']
            errors[c['errline']] += c['err']
            slow.append((c['secs'], name, c))
    print('\nkind                 calls minutes errors')
    for k, (count, mins, errs) in sorted(by_kind.items(), key=lambda x: -x[1][1]):
        print(f'{k:20} {count:5} {mins:7.1f} {errs:6}')
    print('\nslowest calls')
    for secs, name, c in sorted(slow, key=lambda x: -x[0])[:15]:
        print(f"{name} {secs / 60:5.1f}m {c['kind']:18} {c['cmd']}")
    print('\nrecurring errors')
    for line, count in errors.most_common(25):
        if count >= 2 and line:
            print(f'{count:4} {line}')


main()
