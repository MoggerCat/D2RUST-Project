#!/usr/bin/env python3
"""Harvest function names and argument types for the 1.14d Game.exe from
our own specs (specs/**/*.md, *.tsv), for ApplyNames.java / ApplyTypes.java.

Ours: reads only specs/ (our written record) and the private repo's
re/exports/functions.tsv (to keep only real function entries).

  python3 tools/ghidra/spec_harvest.py <functions.tsv> <out_dir>

Writes <out_dir>/spec-names.tsv (entry, name, votes) and
<out_dir>/spec-signatures.tsv (entry, nargs, votes, types: one per argument,
'-' when unknown). A name is taken when the specs pair it with the address
(`NAME (0xADDR)`, `0xADDR (NAME)`, table rows); argument types come from
call shapes like `0x00625480(unit, stat, 0)`.
"""
import collections, os, re, sys

FUNCS, OUT = sys.argv[1], sys.argv[2]
entries = set()
for line in open(FUNCS).read().splitlines()[1:]:
    entries.add(int(line.split('\t')[0], 16))

ADDR = r'0x(00[4-6][0-9A-Fa-f]{5})'
NAME = r'([A-Z][A-Za-z0-9]*_[A-Za-z0-9_]*[A-Za-z0-9]|[A-Z][a-z]+(?:[A-Z][a-z0-9]+)+)'
NAME_PATS = [
    (re.compile(NAME + r'`?\s*\(?\s*(?:at |@ ?|=\s*)?`?' + ADDR), 1, 2),
    (re.compile(ADDR + r'`?\)?\s*\(?`?' + NAME + r'`?\)?'), 2, 1),
]
NOT_NAMES = re.compile(r'^(The|This|See|Spec|Note|Each|With|From|When|And|For|'
                       r'Then|Game|Only|After|Before|Set|Get|Read|Write)$')
CALL = re.compile(r'`?' + ADDR + r'`?\(([^()]{0,120})\)')

UNIT = {'unit', 'u', 'p', 'm', 'i', 'player', 'monster', 'item', 'owner', 'target',
        'merc', 'missile', 'attacker', 'defender', 'caster', 'source', 'object',
        'obj', 'punit', 'pplayer', 'pmonster', 'pitem', 'pobject', 'pmissile',
        'npc', 'minion', 'leader', 'corpse', 'pet', 'golem', 'hireling'}
TYPES = {**{w: 'D2UnitStrc*' for w in UNIT},
         'game': 'D2GameStrc*', 'g': 'D2GameStrc*', 'pgame': 'D2GameStrc*',
         'room': 'D2ActiveRoomStrc*', 'r': 'D2ActiveRoomStrc*', 'proom': 'D2ActiveRoomStrc*',
         'path': 'D2DynamicPathStrc*', 'ppath': 'D2DynamicPathStrc*',
         'list': 'D2StatListStrc*', 'statlist': 'D2StatListStrc*',
         'pstatlist': 'D2StatListStrc*'}

names = collections.defaultdict(collections.Counter)
shapes = collections.defaultdict(list)
for root, _, files in os.walk('specs'):
    for f in files:
        if not f.endswith(('.md', '.tsv')):
            continue
        for line in open(os.path.join(root, f), errors='ignore'):
            for pat, ni, ai in NAME_PATS:
                for m in pat.finditer(line):
                    a, n = int(m.group(ai), 16), m.group(ni)
                    if a in entries and not NOT_NAMES.match(n):
                        names[a][n] += 1
            for m in CALL.finditer(line):
                a = int(m.group(1), 16)
                if a not in entries:
                    continue
                args = [x.strip() for x in m.group(2).split(',')]
                if args == [''] or any(x in ('…', '...') for x in args):
                    continue
                shapes[a].append([TYPES.get(re.sub(r'[^a-z]', '', x.lower().split(' ')[0]))
                                  if re.fullmatch(r'[A-Za-z][\w ]{0,20}', x) else None
                                  for x in args])

# AI think functions (specs/monsters/ai-functions.tsv; ai.md §2.1: ECX game,
# EDX unit, stack: the tick parameter record).
AI = 'specs/monsters/ai-functions.tsv'
if os.path.exists(AI):
    rows = [r.split('\t') for r in open(AI).read().splitlines()]
    col = {k: i for i, k in enumerate(rows[0])}
    for r in rows[1:]:
        if not re.fullmatch(r'0x[0-9A-Fa-f]{8}', r[col['think_1_14d']]):
            continue
        a = int(r[col['think_1_14d']], 16)
        if a in entries:
            names[a][r[col['d2moo_name']]] += 100
            shapes[a] += [['D2GameStrc*', 'D2UnitStrc*', 'D2AiTickParamStrc*']] * 100

os.makedirs(OUT, exist_ok=True)
with open(os.path.join(OUT, 'spec-names.tsv'), 'w') as o:
    o.write('entry\tname\tvotes\n')
    for a, c in sorted(names.items()):
        n, k = c.most_common(1)[0]
        if '_' not in n and k < 2:  # bare CamelCase needs two mentions
            continue
        o.write(f'0x{a:08X}\t{n}\t{k}\n')
with open(os.path.join(OUT, 'spec-signatures.tsv'), 'w') as o:
    o.write('entry\tnargs\tvotes\ttypes\n')
    for a, ss in sorted(shapes.items()):
        nargs, votes = collections.Counter(len(s) for s in ss).most_common(1)[0]
        ss = [s for s in ss if len(s) == nargs]
        types = []
        for i in range(nargs):
            c = collections.Counter(s[i] for s in ss if s[i])
            # one type, never contradicted by another type at this position
            types.append(next(iter(c)) if len(c) == 1 else '-')
        if any(t != '-' for t in types):
            o.write(f'0x{a:08X}\t{nargs}\t{votes}\t{",".join(types)}\n')
