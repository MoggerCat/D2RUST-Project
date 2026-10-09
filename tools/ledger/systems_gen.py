#!/usr/bin/env python3
"""Generate docs/handoff/ledger/systems.tsv (fidelity ledger, part: systems). Measurement only."""
import re, subprocess, glob, os, sys, csv
R = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
os.chdir(R)
def sh(c): return subprocess.run(c, shell=True, capture_output=True, text=True).stdout
status = sh("git show origin/claude/q-fix-check-triage:docs/handoff/checks-status.md").splitlines()
verd = {}  # check -> list of (channel, verdict, frame)
for l in status:
    p = [x.strip() for x in l.split('|')]
    if len(p) > 6 and p[1] and p[1] not in ('Check', '---'):
        verd.setdefault(p[1], []).append((p[2], p[3], p[5]))
prov = open('docs/handoff/provisional-index.tsv', encoding='utf-8').read().splitlines()[1:]
owners = [l.split('\t') for l in open('tools/coord/owners.tsv') if not l.startswith('#')][1:]
def prov_stats(path):
    rows = [l.split('\t') for l in prov if path in l]
    return len(rows), any(len(r) > 6 and r[6].strip() == 'no' for r in rows)
checks_all = sorted(os.path.basename(f)[:-6] for f in glob.glob('traces/checks/*.check'))
def verdict_for(pats, chans=None):
    cs = [c for c in checks_all if any(re.search(p, c) for p in pats)]
    if not cs: return '-', '-'
    vs = [v for c in cs for v in verd.get(c, []) if not chans or v[0] in chans]
    if chans: cs = [c for c in cs if any(v[0] in chans for v in verd.get(c, []))]
    if not cs: return '-', '-'
    if not vs: return ','.join(cs[:6]), '-'
    d = [v for v in vs if v[1].startswith('DIVERGED')]
    if d: return ','.join(cs[:6]), 'DIVERGED@' + (re.search(r'(?:frame|tick) (\d+)', d[0][2]).group(1) if re.search(r'(?:frame|tick) (\d+)', d[0][2]) else '?')
    if all(v[1] == 'MATCH' for v in vs): return ','.join(cs[:6]), 'MATCH'
    return ','.join(cs[:6]), 'PARTIAL'
src_cache = {}
def spec_in_code(spec):
    if spec not in src_cache:
        src_cache[spec] = sh(f"grep -rl --include=*.rs -F '{spec}' crates | head -3").split()
    return src_cache[spec]
rows = []
def add(**k):
    d = dict(area='', kind='system', group='systems', src='-', specs='-', spec_status='none', checks='-', verdict='-',
             exercised='?', prov='0', pc1='n', owner='-', state='UNKNOWN', size='M', note='')
    d.update(k); rows.append(d)
def owner_of(path):
    best = ('-', 0)  # branch name as in owners.tsv col 4
    for a, p, s, b in [(o[0], o[1], o[2], o[3].strip()) if len(o) >= 4 else (o[0], '', '', '') for o in owners]:
        for q in p.split(','):
            if q and path.startswith(q) and len(q) > best[1]: best = (b, len(q))
    return best[0]
def slug(s): return re.sub(r'[^a-z0-9]+', '-', s.lower()).strip('-')[:48]
def status_of(path):
    for l in open(path, encoding='utf-8'):
        m = re.search(r'\*\*Status:\*\*\s*(.*)', l)
        if m: return re.split(r'[:;(,.]', m.group(1).strip().replace('`',''))[0].strip()[:40] or 'none'
    return 'none'
pk_div = {}
for l in status:
    p = [x.strip() for x in l.split('|')]
    if len(p) > 6 and p[2] == 'packets' and p[3].startswith('DIVERGED'):
        m = re.search(r'stream (c2s|s2c).*\(id 0x([0-9a-fA-F]+)\)', p[5]); f = re.search(r'frame (\d+)', p[5])
        if m: pk_div[(m.group(1), '0x' + m.group(2).upper())] = p[1] + '@' + (f.group(1) if f else '?')
# ---- messages
for tbl, dirn, kindname in (('client-messages.tsv', 'c2s', 'c2s'), ('server-messages.tsv', 's2c', 's2c')):
    rd = csv.DictReader(open('specs/sim/' + tbl, encoding='utf-8'), delimiter='\t')
    for r in rd:
        name = r['name']; i = r['id'].lower()
        h = r.get('handler') or r.get('client_handler') or '-'
        used = sh(f"grep -rl --include=*.rs -w '{name}' crates/d2-server/src crates/d2-sim/src crates/d2-client/src 2>/dev/null | head -2").split() if name not in ('-', '') else []
        div = pk_div.get((dirn, '0x' + i[2:].upper()))
        if name in ('-', ''): st, note, sz = 'NO-CHECK', 'id unused/rejected in 1.14d; implemented: nothing; no packets check contains it', 'S'
        elif div: st, sz, note = 'DIVERGED', 'S', f'packets check {div} diverges on this id (checks-status.md first divergence)'
        elif used: st, sz, note = 'NO-CHECK', 'S', 'implemented (' + used[0].split('crates/')[-1] + '); no packets check shown to contain this id on both sides (recorded 1.14d packet logs are not in the repo, not scanned)'
        else: st, sz, note = 'NOT-IMPLEMENTED', 'S', 'decided by grep: name unused in d2-server/d2-sim/d2-client (d2-proto codegen only parses the layout)'
        chk, v = (div.split('@')[0], 'DIVERGED@' + div.split('@')[1]) if div else ('-', '-')
        add(area=f'net.{dirn}.{i}', kind='message', src=f'{tbl[:-4]} {i} {name} handler {h}', specs=f'specs/sim/{tbl}',
            spec_status='confirmed=' + (r.get('confirmed') or '?'), checks=chk, verdict=v, owner=owner_of(f'specs/sim/{tbl}#{i}'),
            state=st, size=sz, note=note, pc1='n')
# ---- specs
areas = {
 'sim': 'system', 'combat': 'system', 'flows': 'system', 'seams': 'system', 'client': 'system', 'render': 'system',
 'audio': 'system', 'ui': 'ui', 'formats': 'system',
}
pick = ['specs/sim/*.md', 'specs/combat/*.md', 'specs/flows/*.md', 'specs/seams/*.md', 'specs/client/*.md', 'specs/render/*.md',
        'specs/audio/*.md', 'specs/ui/*.md', 'specs/formats/d2s*.md', 'specs/formats/font-tbl.md', 'specs/formats/palette.md',
        'specs/formats/cof.md', 'specs/formats/animdata.md', 'specs/formats/dcc.md', 'specs/formats/dc6.md', 'specs/formats/dt1.md', 'specs/formats/wav.md']
chk_map = [(r'sim/rng|unit-order', (['^rng-'], {'rng'})), (r'draw-order', (['^draws-'], {'draws'})), (r'path|movement', (['^walk-'], {'state'}))]
def spec_checks(p):
    for pat, c in chk_map:
        if re.search(pat, p): return c
    return (['^zzz'], None)
for pat in pick:
    for path in sorted(glob.glob(pat)):
        txt = open(path, encoding='utf-8').read()
        top = path.split('/')[1]
        kind = 'ui' if top == 'ui' else 'system'
        st = status_of(path)
        code = spec_in_code(path)
        chk, v = verdict_for(*spec_checks(path))
        if chk == '-' and False: pass
        pc, pcn = prov_stats(path)
        ow = owner_of(path)
        m = re.search(r'\n## Rules\n(.*?)(?=\n## )', txt, re.S)
        body = m.group(1) if m else ''
        heads = re.findall(r'^### (.+)$', body, re.M)
        pc1 = 'y' if pcn else 'n'
        if v == 'MATCH': state, size, why = 'EQUAL', '-', 'mapped check MATCH'
        elif v != '-': state, size, why = 'DIVERGED', 'M', f'mapped check {v}'
        elif code: state, size, why = 'NO-CHECK', 'M', 'a crate cites the spec (// Spec: line) but no check compares it'
        else: state, size, why = 'NOT-IMPLEMENTED', 'M', 'no crate cites this spec (// Spec: grep)'
        unit = [(None, path)] if not heads else [(h, path) for h in heads]
        for h, _ in unit:
            base = path[len('specs/'):-3].replace('/', '.')
            area = f'system.{base}' + (f'.{slug(h)}' if h else '')
            n = 'whole spec (no ### rule groups)' if not h else 'rule group row; state is per spec file'
            note = why + (f' ({code[0].split("crates/")[-1]})' if code else '') + '; spec status "' + st + '"; ' + n
            add(area=area, kind=kind, src=f'{path} ' + (f'§{h}' if h else 'whole'), specs=path, spec_status=st, checks=chk, verdict=v,
                prov=str(pc) if False else str(pc), pc1=pc1, owner=ow, state=state, size=size, note=note)
add(area='system.perf.budget', src='docs/handoff/bench-baselines.md', note='bench baselines exist (docs/handoff/bench-baselines.md); no 1.14d-side budget to compare, original timing is tick-based (tick.md)', state='NO-CHECK', size='S', specs='-')
seen = set(); out = []
for r in rows:
    a = r['area']; k = a; j = 2
    while k in seen: k = f'{a}-{j}'; j += 1
    r['area'] = k; seen.add(k); out.append(r)
hdr = 'area kind group source_1.14d specs spec_status checks last_verdict exercised provisional needs_pc1 owner state size note'.split()
keys = 'area kind group src specs spec_status checks verdict exercised prov pc1 owner state size note'.split()
os.makedirs('docs/handoff/ledger', exist_ok=True)
with open('docs/handoff/ledger/systems.tsv', 'w', encoding='utf-8', newline='') as f:
    f.write('#ledger 1\n' + '\t'.join(hdr) + '\n')
    for r in out: f.write('\t'.join(str(r[k]).replace('\t', ' ').replace('\n', ' ') for k in keys) + '\n')
from collections import Counter
print(len(out), Counter(r['state'] for r in out))
