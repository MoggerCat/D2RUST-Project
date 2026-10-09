import csv,re,sys
P=[l.rstrip('\n').split('\t') for l in open('docs/handoff/provisional-index.tsv')][1:]
def spec_status(path):
    try:
        t=open(path,encoding='utf8').read(4000)
    except OSError: return 'none'
    m=re.search(r'\*\*Status:\*\*\s*([a-zA-Z][a-zA-Z-]*)',t)
    return m.group(1).lower() if m else 'none'
KEYS={
 'quest':['quest_host','quest_events','quest_reward','specs/world/quests','world/quests.rs','tests/quests_act','npc_talk'],
 'object':['wiring/action/objects','specs/world/objects','bridge/objects','objects/mech'],
 'shrine':['wiring/action/objects','specs/world/objects','objects/mech'],
 'level':['specs/drlg','drlg/','worldgen/'],'drlg':['specs/drlg','drlg/','worldgen/'],
 'npc':['npc_talk','messages/hire','specs/world/npc','specs/world/vendors'],
 'hireling':['messages/hire','hireling'],'waypoint':['waypoint'],
 'system':['specs/flows','save-exit','act-change'],
}
rows=[l.rstrip('\n').split('\t') for l in open('docs/handoff/ledger/world.tsv')]
hdr=rows[1];out=rows[:2]
for r in rows[2:]:
    pre=r[0].split('.')[0]
    if r[0].startswith('system.hireling'): pre='hireling'
    if r[0].startswith('drlg'): pre='drlg'
    ks=KEYS.get(pre,[])
    if pre=='quest':
        m=re.search(r'quests-act(\d)',r[4]); 
    hits=[p for p in P if any(k in p[1] for k in ks)]
    r[9]=str(len(hits))
    r[10]='y' if any(p[5] in('recording','binary') and p[6]=='no' for p in hits) else 'n'
    specs=[s for s in r[4].split(',') if s.endswith('.md')]
    sts=sorted({spec_status(s) for s in specs}) or ['none']
    r[5]='/'.join(sts) if specs else 'none'
    if r[11] in('-',''): pass
    r[11]=re.sub(r'^session_01UWEn.*$','claude/q-fix-pc1-proto-items',r[11])
    if pre=='hireling': r[11]='claude/q-diff-skills-2'
    if pre in('object','shrine') and r[11]=='-': r[11]='claude/q-fix-server-store-fill' if False else '-'
    v=r[7]
    if v in('PARTIAL','DIVERGED'): r[12]='DIVERGED'
    elif v=='MATCH': r[12]='EQUAL'
    elif r[12]=='DIVERGED': r[12]='NO-CHECK'
    if r[12]=='EQUAL': r[13]='-'
    elif r[13]=='M':
        r[14]+='; size M: spec draft, needs a check recorded/compared against 1.14d and any divergence fixed'
    out.append(r)
open('docs/handoff/ledger/world.tsv','w',encoding='utf8').write('\n'.join('\t'.join(r) for r in out)+'\n')
import collections;print(collections.Counter(r[12] for r in out[2:]),collections.Counter(r[5] for r in out[2:]))
