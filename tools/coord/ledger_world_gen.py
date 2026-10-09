import csv,re,collections
X='/home/user/d2rust-private-repo/extracted/d2exp.mpq/data/global/excel/'
def tab(n):
    return list(csv.reader(open(X+n,encoding='latin1'),delimiter='\t',quotechar=None))
# check verdicts
V=collections.defaultdict(set)
for l in open('/tmp/cs.md'):
    p=[c.strip() for c in l.split('|')]
    if len(p)>5 and p[1].endswith(('ama','sor','bar','cow')) or (len(p)>5 and p[1] and p[3] in('MATCH','PARTIAL','DIVERGED')):
        V[p[1]].add(p[3])
def verdict(cs):
    s=set()
    for c in cs: s|=V.get(c,set())
    return 'DIVERGED' if 'DIVERGED' in s else 'PARTIAL' if 'PARTIAL' in s else 'MATCH' if s else '-'
prov=[l.split('\t') for l in open('docs/handoff/provisional-index.tsv')][1:]
def pc(*keys):
    return sum(1 for p in prov if any(k in p[1] for k in keys))
OWN={'town':'claude/q-fix-real-unit-seed-order','a2':'claude/q-prov-recording-2','a3':'claude/q-play-act3','a4':'claude/q-prov-data','a5':'claude/q-play-act5','drlg':'claude/q-prov-recording','prog':'session_01UWEn2ZtdtLRHPZRdytF','quest':'claude/q-fix-pc1-day3-a-r2','items':'claude/q-fix-server-store-fill','hire':'(hireling row of owners.tsv)'}
rows=[]
def add(area,kind,src,specs,st,checks,note,own,state=None,size='M',needs='n',prov_n=0,grp='world'):
    cs=[c for c in checks.split(',') if c and c!='-']
    v=verdict(cs)
    if state is None:
        state='DIVERGED' if v=='DIVERGED' else 'NO-CHECK'
    if state=='EQUAL': size='-'
    rows.append([area,'entity' if kind is None else kind,grp,src,specs,st,checks or '-',v,'?',str(prov_n),needs,own,state,size,note])
# levels
L=tab('Levels.txt');h={k:i for i,k in enumerate(L[0])}
LK={'1':'maze','2':'preset','3':'outdoor'}
chk={'Act 1 - Town':'a1-town-arrival-ama,draws-town-arrival-ama,packets-town-arrival-ama,rng-town-arrival-ama,walk-town-ama','Act 1 - Wilderness 1':'combat-pop-blood-moor','Act 1 - Wilderness 2':'warp-cold-plains-ama,combat-pop-cold-plains,combat-cold-plains-wp','Act 1 - Wilderness 3':'combat-pop-stony-field','Act 1 - Cave 1':'a1-warp-cave-ama','Act 1 - Crypt 1':'a1-warp-den-ama','Act 1 - Mausoleum':'a1-warp-catacombs-ama','Act 1 - Jail 1':'a1-warp-jail-ama','Act 1 - Tower Cellar 1':'a1-warp-tower-cellar-ama','Act 1 - Catacombs 1':'a1-warp-catacombs-ama','Act 2 - Town':'act-travel-lut-ama,join-act2-quests-ama','Act 2 - Sewer 1':'a2-warp-sewers-ama','Act 2 - Maggot Lair 1':'a2-warp-maggot-lair-ama','Act 2 - Tomb 1':'a2-warp-tal-rasha-tomb-ama','Act 2 - Arcane Sanctuary':'a2-warp-arcane-ama','Act 3 - Town':'milestone-act3-entry,a3-start-noquest-sor','Act 3 - Dungeon 1':'a3-warp-flayer-dungeon-ama','Act 3 - Sewer 1':'a3-warp-kurast-sewers-ama','Act 3 - Hell 1':'a3-warp-durance-ama','Act 4 - Town':'a4-fortress-arrival-ama,milestone-act4-entry,a4-start-noquest-sor','Act 4 - Outer Steppes':'a4-warp-plains-ama','Act 4 - Plains of Despair':'a4-warp-plains-ama','Act 4 - River of Flame':'a4-warp-river-ama','Act 5 - Town':'a5-harrogath-arrival-ama,a5-town-arrival-bar,milestone-act5-entry','Act 5 - Crystalized Cavern 1':'a5-warp-crystalized-ama','Act 5 - Halls of Anguish':'a5-warp-halls-anguish-ama','Act 5 - Worldstone Keep 1':'a5-warp-wsk-ama','Act 5 - Throne of Destruction':'milestone-baal-throne,milestone-baal-chamber'}
specm={'maze':'specs/drlg/maze.md','preset':'specs/drlg/preset.md','outdoor':'specs/drlg/outdoor.md'}
def ok(s):return re.sub(r'[^a-z0-9]+','-',s.lower()).strip('-')
seen=set()
for r in L[1:]:
    if not r or not r[0] or r[0]=='Null' or r[0]=='Expansion': continue
    nm=r[h['Name']];lid=r[h['Id']];act=int(r[h['Act']])+1
    if nm.strip()=='' :continue
    dk=LK.get(r[h['DrlgType']],'?')
    ca=chk.get(nm,'-')
    a='level.a%d.%s.%s'%(act,lid,ok(nm))
    if a in seen: continue
    seen.add(a)
    town='Town' in nm
    own={1:OWN['town'],2:OWN['a2'],3:OWN['a3'],4:OWN['a4'],5:OWN['a5']}[act] if town else OWN['drlg']
    add(a,'entity','Levels.txt %s (%s) DrlgType=%s'%(nm,lid,dk),'specs/drlg/levels.md,%s,specs/drlg/rooms.md'%specm.get(dk,'specs/drlg/levels.md'),'see spec header',ca,'DRLG kind %s; layout+population compared only if a check lists it; no per-level layout check'%dk,own,prov_n=0)
# presets: group by act via LvlPrest rows
P=tab('LvlPrest.txt');ph={k:i for i,k in enumerate(P[0])}
n=0
for r in P[1:]:
    if len(r)<3 or not r[0] or r[0]=='Expansion':continue
    n+=1
add('drlg.preset.lvlprest','system','LvlPrest.txt (%d rows: preset-level DS1 files, Def, Dt1Mask)'%n,'specs/drlg/preset.md,specs/drlg/preset-tables.tsv','see spec header','-','one row for all preset levels; DS1 selection per level not compared against 1.14d','claude/q-prov-recording',kind='system') if False else None
def S(area,src,specs,checks,note,own,state=None,size='M',needs='n',p=0):
    add(area,'system',src,specs,'see spec header',checks,note,own,state,size,needs,p)
S('drlg.preset.lvlprest','LvlPrest.txt (%d rows)'%n,'specs/drlg/preset.md,specs/drlg/preset-tables.tsv','-','all preset levels share one code path; DS1 pick and preset-unit lists not compared per level','claude/q-prov-recording')
S('drlg.maze.lvlmaze','LvlMaze.txt rows, specs maze-specials.tsv','specs/drlg/maze.md,specs/drlg/maze-specials.tsv','a1-warp-tower-cellar-ama,a2-warp-maggot-lair-ama,a2-warp-sewers-ama,a2-warp-tal-rasha-tomb-ama,a3-warp-kurast-sewers-ama,a3-warp-durance-ama,a5-warp-halls-anguish-ama','maze rooms/specials; checks cover arrival state only; maze layout vs 1.14d not compared','claude/q-prov-recording')
S('drlg.outdoor.tilesub','Levels.txt DrlgType=3 + outdoor-path-floor.tsv','specs/drlg/outdoor.md,specs/drlg/outdoor-tilesub.md,specs/drlg/outdoor-act3-act5.md','warp-cold-plains-ama,combat-pop-blood-moor,a4-warp-plains-ama','outdoor tile substitution and path/floor','claude/q-prov-recording')
S('drlg.wall-remap','specs/drlg/wall-remap.tsv','specs/drlg/wall-remap.md','-','wall remap table','claude/q-prov-recording')
S('drlg.level-seed','function 0x006424A0 level generation, level seed','specs/drlg/levels.md','rng-town-arrival-ama','level seeds and get-or-allocate','claude/q-prov-recording')
S('drlg.warps.vis-lvlwarp','LvlWarp.txt + Levels.txt Vis0-7','specs/drlg/levels.md','a1-warp-cave-ama,a1-warp-den-ama,a1-warp-jail-ama,a2-warp-sewers-ama,a3-warp-flayer-dungeon-ama,a4-warp-river-ama,a5-warp-crystalized-ama','vis/warp records and level connections (Act I, III, V); warp arrival checked for ~19 destinations, other warps unchecked','claude/q-prov-recording')
# waypoints
W=[l.rstrip('\n').split('\t') for l in open('specs/world/waypoints.tsv')][1:]
wchk={'Cold Plains':'combat-cold-plains-wp','Lut Gholein':'act-travel-lut-ama'}
for w in W:
    add('waypoint.%s.%s'%(w[0],ok(w[5])),'entity','waypoints.tsv wp %s level %s act %d'%(w[0],w[1],int(w[2])+1),'specs/world/waypoints.md','see spec header',wchk.get(w[5],'-'),'waypoint object + menu/travel for this level; town waypoints share code','session_01UWEn2ZtdtLRHPZRdytF',prov_n=0)
# objects grouped by operate
OF=[l.rstrip('\n').split('\t') for l in open('specs/world/object-functions.tsv')][1:]
for o in OF:
    if o[0]=='operate':
        add('object.operate.%s.%s'%(o[1],ok(o[3])),'entity','objects.txt operate fn %s @%s (%s rows)'%(o[1],o[2],o[4]),'specs/world/objects.md,specs/world/objects-2.md','draft','-','operate function %s (%s rows share it); no interaction recording exists'%(o[3],o[4]),OWN['items'] if False else '-')
    elif o[0]=='populate' or o[0]=='preset':
        add('object.%s.%s.%s'%(o[0],o[1],ok(o[3])),'entity','objects.txt %s fn %s @%s (%s rows)'%(o[0],o[1],o[2],o[4]),'specs/world/object-population.md','draft','-','%s function %s; only %s rows share it'%(o[0],o[3],o[4]),'-')
S('object.init.functions','objects.txt init fn column (79 functions, object-functions.tsv kind=init)','specs/world/objects.md,specs/world/objects-2.md','-','79 init functions; objanim a1-town facts only (facts/objects/objanim-a1-town.tsv)','-')
# shrines
SH=tab('shrines.txt')
for i,r in enumerate(SH[1:],0):
    if len(r)<2 or not r[1] or r[1]=='Expansion':continue
    add('shrine.%d.%s'%(i,ok(r[1])),'entity','shrines.txt row %d %s (%s)'%(i,r[1],r[0]),'specs/world/objects.md','draft','-','shrine effect: operate fn 2 @0x00583C70 §9; shrine tick effects (duration) unchecked','-')
# NPCs
V2=[l.rstrip('\n').split('\t') for l in open('specs/world/vendors.tsv')][1:]
for v in V2:
    act=int(v[2])+1
    ch='items-vendor-akara-buy' if v[1]=='akara' else '-'
    own=OWN['quest']
    add('npc.%s'%ok(v[1]),'entity','vendors.tsv npc %s %s act %d trader=%s heals=%s identifies=%s hire=%s resurrects=%s; npc.txt multipliers'%(v[0],v[1],act,v[3],v[10],v[11],v[12],v[13]),'specs/world/npc.md,specs/world/vendors.md,specs/world/vendors-2.md,specs/ui/npc-menus.tsv','see spec header',ch,'menu, dialogue, store (items-vendor-akara-buy covers only Akara buy), hire/resurrect'+(' (hireling seller)' if v[12]!='0' else '')+(' (resurrects merc)' if v[13]!='0' else ''),own,size='M')
# quests
Q=[l.rstrip('\n').split('\t') for l in open('specs/world/quests.tsv')][1:]
qchk={'A1Q1':'','A4Q2':'milestone-izual','A4Q3':'milestone-hellforge','A4Q4':'','A5Q4':'milestone-anya','A5Q5':'','A5Q6':'milestone-baal-chamber','A5Q1':'milestone-nihlathak'}
for q in Q:
    nm=q[16];act=int(q[3])+1
    own={1:OWN['quest'],2:OWN['a2'],3:OWN['a3'],4:OWN['a4'],5:OWN['a5']}[act]
    cs=''
    for k,c in qchk.items():
        if nm.startswith(k) and c: cs=c
    if 'Hell' in nm or 'Hephasto' in nm: cs='milestone-hephasto,milestone-hellforge'
    if 'Rescue' in nm: cs=''
    add('quest.%s'%ok(nm),'entity','quests.tsv index %s chain %s init %s callbacks %s'%(q[0],q[1],q[6],q[11]),'specs/world/quests.md,specs/world/quests-status.md,specs/world/quests-act%d.md,specs/world/quest-messages.tsv'%act,'see spec header',cs,'state machine (init/status/callbacks); %s'%('quest-state milestone check partial' if cs else 'no check compares quest flags/states'),own,size='M')
# act travel/town portal/death/difficulty
for a,src,sp,cs,note in [
('system.act.travel','act change: Act transitions','specs/flows/act-change.md','act-travel-lut-ama,milestone-act3-entry,milestone-act4-entry,milestone-act5-entry','act-change flow; Lut Gholein travel DIVERGED, act 3/4 entry PARTIAL, act 5 entry DIVERGED'),
('system.townportal','Town Portal (tome/scroll) open/enter/close','specs/flows/act-change.md,specs/world/objects.md','-','portal object operate (object.operate); no check'),
('system.death.corpse','death, corpse creation and retrieval','specs/flows/save-exit.md,specs/sim/stats.md','death-town-ama','death-town-ama PARTIAL; corpse retrieval and penalty not compared'),
('system.difficulty.progression','Normal/NM/Hell unlock, DifficultyLevels.txt','specs/flows/save-exit.md,specs/world/quests-act5.md','-','difficulty unlock after Baal and monster scaling not compared'),
('system.hireling.hire-follow-level','hireling.txt (60 rows, 4 mercenary classes x difficulty)','specs/world/hirelings.md,specs/world/hirelings-2.md,specs/world/hirelings-ai.md','merc-rogue-town-bar,merc-rogue-cow,merc-barb-cow,merc-desert-cow,merc-sorc-cow','merc hire, follow AI, level-up by act; 4 of 5 merc checks DIVERGED')]:
    add(a,'system',src,sp,'see spec header',cs,note,OWN['prog'] if 'hire' not in a else '-',size='M')
H=tab('hireling.txt')
acts=collections.OrderedDict()
for r in H[1:]:
    if len(r)<6 or not r[0] or r[0]=='Expansion':continue
    acts.setdefault((r[0],r[4]),set()).add(r[1])
for (cls,act),subs in acts.items():
    mc={'Rogue Scout':'merc-rogue-cow,merc-rogue-town-bar','Desert Mercenary':'merc-desert-cow','Iron Wolf':'merc-sorc-cow','Barbarian':'merc-barb-cow'}.get(cls,'-')
    add('hireling.%s.act%s'%(ok(cls),act),'entity','hireling.txt %s act %s (%d subtypes, all difficulties)'%(cls,act,len(subs)),'specs/world/hirelings.md,specs/world/hirelings-ai.md','see spec header',mc,'hire list, stats per level, AI; all difficulties grouped','-')
with open('docs/handoff/ledger/world.tsv','w',encoding='utf8',newline='') as f:
    f.write('#ledger 1\n')
    f.write('\t'.join('area kind group source_1.14d specs spec_status checks last_verdict exercised provisional needs_pc1 owner state size note'.split())+'\n')
    for r in rows: f.write('\t'.join(c.replace('\t',' ').replace('\n',' ') for c in r)+'\n')
print(len(rows),collections.Counter(r[12] for r in rows))
