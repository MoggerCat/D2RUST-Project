#!/usr/bin/env python3
"""Groups the provisional points whose settle_kind is `recording`
(docs/handoff/provisional-index.tsv) by the recording scenario that would
settle them (q-prov-recording; plan: docs/handoff/q-prov-recording.md).
Writes docs/handoff/q-prov-recording.tsv (group, rec, location) and prints
the count per group. `py tools/cloud-game/prov_recording_groups.py G1 G2`
prints the points of those groups instead. Our own code."""
import csv,re,collections,sys,os
R=os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))+"/"
rows=[r for r in csv.DictReader(open(R+'docs/handoff/provisional-index.tsv'),delimiter='\t') if r['settle_kind']=='recording']
G={
 'G1':('Join stream (packets at game start, fresh and existing character, equipped items, portal and hireling join)',
       'REC-02 REC-188 REC-405 REC-241 REC-282 REC-46 REC-44'.split()),
 'G2':('Item moves in town (cursor drop / pick-up / drop, equip, belt potion, identify, socket, charm, stash)',
       'REC-289 REC-281 REC-102 REC-113 REC-121 REC-163 REC-158 REC-266 REC-34 REC-177'.split()),
 'G3':('Town Portal (scroll / tome, field cast, in-town cast, portal pair, rejoin)',
       'REC-117 REC-243'.split()),
 'G4':('Blood Moor field: kill, drop spot, chest, shrine, hit reaction, level-up, death',
       'REC-108 REC-260 REC-141 REC-124 REC-96 REC-93 REC-08 REC-07 REC-239 REC-263 REC-126 REC-291'.split()),
 'G5':('Warp tile and object interact range (Den of Evil entrance, waypoint, chest from 1/3/5/8 subtiles)',
       'REC-99 REC-94'.split()),
 'G6':('Town walk / run and NPC interaction: local vs server position, walk prediction, shop open, visibility',
       'REC-51 REC-277 REC-288 REC-286 REC-269 REC-268 REC-242'.split()),
 'G7':('Protocol hook sittings (R-* hooks: overlay 0x11, leap 0xA5, heal 0xAB, item cast 0x99, missile 0x73, chat 0x15, skill-mode messages, two clients)',
       'REC-410 REC-411 REC-412 REC-413 REC-414 REC-402 REC-95 REC-461 REC-62 REC-275'.split()),
 'G8':('DRLG / population hooks (border substitution file, Trees DS1, waypoint walk probes, Radament)',
       'REC-404 REC-35 REC-04 REC-81'.split()),
 'G9':('Later-act and quest sittings (Act II–V saves: Cain, Radament, Duriel, Baal portal, Act V hooks)',
       'REC-45 REC-27 REC-128 REC-129 REC-136 REC-148 REC-167 REC-235 REC-246 REC-254 REC-11'.split()),
 'G10':('Front end (menus, create, controls, credits, cinematics, loading, Esc menu, exit target)',
       'REC-181 REC-184 REC-185 REC-186 REC-200 REC-201 REC-205 REC-206 REC-207 REC-209 REC-212 REC-221 REC-222 REC-223 REC-225 REC-226 REC-228 REC-236'.split()),
 'G11':('Audio (UI request sites, async loads, unit footstep frames)',
       'REC-19 REC-430'.split()),
 'G12':('Render-only captures (summit clouds, state tint, camera, x87 word, weather, automap, sprite limits)',
       'REC-420 REC-245 REC-21 REC-97'.split()),
 'SKIP':('Owned by running sessions (client missiles REC-450..452; scenes compare REC-440/441)',
       'REC-450 REC-451 REC-452 REC-440 REC-441'.split()),
 'DONE':('REC entry already settled from the binary (pc1-s8); the PROVISIONAL line cites it only by proximity or awaits clean-up',
       'REC-60 REC-82'.split()),
}
loc={ # unnumbered rows by file
 'audio/sound_table':'G11','bridge/msg/items.rs':'G1','bridge/msg/unit_misc.rs':'G1','bridge/world.rs':'G1',
 'draw_order/weather.rs':'G12','msg_ui_more.rs':'G7','d2-native/src/tileset.rs':'G12','action/death.rs':'G4',
 'monster_add.rs':'G7','objects.rs:730':'G4','objects.rs:1201':'G4','action/switch.rs':'G3','warp_tile.rs:162':'SKIP',
 'drop_helpers.rs':'G4','quest_reward.rs':'G9','path/walk.rs':'G6','worldgen/dispatch.rs':'SKIP','client/model.md':'G6',
 'client-bodies-2.md':'SKIP','camera.md':'G12','sprite-placement.md':'G12','sim/pathing.md':'G6','automap.md':'G12',
 'specs/ui/frontend':'G10',
}
r2g={r:g for g,(t,l) in G.items() for r in l}
out=collections.defaultdict(list)
for r in rows:
    ids=re.findall(r'REC-\d+',r['rec'])
    g=None
    for i in ids:
        if i in r2g: g=r2g[i];break
    if r['rec'].startswith('~') or not g:
        for k,v in loc.items():
            if k in r['location']: g=v;break
    if g is None and ids and ids[0] in r2g: g=r2g[ids[0]]
    out[g or '??'].append(r)
with open(R+'docs/handoff/q-prov-recording.tsv','w') as o:
    o.write('group\trec\tlocation\n')
    for g in list(G)+['??']:
        for r in out[g]: o.write(f"{g}\t{r['rec']}\t{r['location']}\n")
if len(sys.argv)>1:
    for g in sys.argv[1:]:
        for r in out[g]: print(r['rec'],r['location'],r['chosen'][:200],sep=' | ')
else:
    for g in list(G)+['??']:
        print(g,len(out[g]),G.get(g,('',))[0])
    print(sum(len(v) for v in out.values()))
    for r in out['??']: print('??',r['rec'],r['location'])
