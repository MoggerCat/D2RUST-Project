"""Act IV Diablo flow on one seed, GUIDs resolved per run (q-fix-a4-deseis).

    python3 docs/handoff/q-fix-a4-deseis-seeds.py <seed> [-v]

Runs `d2-client state-dump` (target/release, build it first) step by step:
for each seal a `goto preset 108 2:<seal>`, its GUID from the goto record,
a `pos` onto the seal and msg 0x13; before the De Seis seal (394) a walk
33 + 41 sub-tiles south and back (loads the arm room the boss spot needs,
see q-fix-a4-deseis.md); each seal boss (the first new unit of its class)
is reached, set to 1 hp and shot with Ice Bolt; then the start point (255)
and Diablo's kill. Prints "DIABLO present/killed"; -v prints the pokes
(the act4.play milestones were made from them). Needs D2_GAME_DIR.
Standard library only. Our own code.
"""
import json, os, subprocess, sys, tempfile
REPO=os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)),"..",".."))
WORK=tempfile.mkdtemp(prefix="a4-deseis-")
SAVE=os.path.join(WORK,"PtSorD.d2s")
subprocess.run([REPO+"/target/release/d2s-tool","new","--name","PtSorD"]+"--class sor --expansion --level 50 --skill 8=20 --right-skill 44 --stat 8=128000 --stat 9=128000 --waypoints all --quests acts=3 --act 3 --difficulty normal".split()+["-o",SAVE],check=True,capture_output=True)
def run(seed, pokes, ticks):
    out=os.path.join(WORK,f"run{seed}.jsonl")
    cmd=[REPO+"/target/release/d2-client","state-dump","--save",SAVE,"--seed",str(seed),"--ticks",str(ticks),
         "--out",out,"--date","2026-01-01"]
    for p in pokes: cmd+=["--poke",p]
    subprocess.run(cmd,capture_output=True,text=True)
    return [json.loads(l) for l in open(out)]
def last_snap(recs): return [r for r in recs if r["k"]=="snap"][-1]
def goto_guid(recs, f):
    for r in recs:
        if r["k"]=="poke" and r["d"]=="goto" and r["src"].startswith("goto") and r["frame"]>=f-1: return r.get("guid"), r["r"]
seed=int(sys.argv[1])
P=["5 warp 108","6 stat @player 7 0 7680000","7 stat @player 6 0 7680000"]
plan=[(20,392,362),(500,393,None),(1000,394,312),(1500,395,None),(2000,396,306)]
for f,seal,boss in plan:
    P.append(f"{f} goto preset 108 2:{seal}")
    recs=run(seed,P,f+399)
    g,res=goto_guid(recs,f)
    if res!="ok": print(seed,"goto",seal,res); sys.exit(1)
    if seal==394:
        P+=[f"{f+300} pos @player @x @y+33", f"{f+320} pos @player @x @y+41", f"{f+340} pos @player @x @y-41", f"{f+360} pos @player @x @y-33"]
    su=[u for u in last_snap(recs)["units"] if u["ut"]==2 and u["g"]==g][0]
    P.append(f"{f+390} pos @player {su['x']} {su['y']}")
    P.append(f"{f+400} msg 0x13 2 {g}")
    if boss:
        recs=run(seed,P,f+449)
        snaps=[r for r in recs if r["k"]=="snap"]
        before={u["g"] for u in snaps[f+398]["units"] if u["ut"]==1}
        b=sorted([u for u in last_snap(recs)["units"] if u["ut"]==1 and u["cl"]==boss and u["g"] not in before],key=lambda u:u["g"])
        if not b:
            print(seed,"seal",seal,"boss",boss,"absent; seal mode",[u["m"] for u in last_snap(recs)["units"] if u["ut"]==2 and u["g"]==g]); sys.exit(1)
        bg=b[0]["g"]; bx,by=b[0]["x"],b[0]["y"]
        pl=[u for u in last_snap(recs)["units"] if u["ut"]==0][0]
        if abs(pl["x"]-bx)+abs(pl["y"]-by)>20:
            P+=[f"{f+420} pos @player {(pl['x']+bx)//2} {(pl['y']+by)//2}", f"{f+435} pos @player {bx-4} {by}"]
        for t,(ox,oy) in zip((455,463,471,479,487,495),(("+3",""),("-3",""),("","+3"),("","-3"),("+2","+2"),("-2","-2"))):
            P+= [f"{f+t} pos 1/{bg} @x{ox} @y{oy}", f"{f+t} stat 1/{bg} 6 0 256", f"{f+t} missile 59 @x @y @x{ox} @y{oy} skill 39 1"]
P.append("2600 goto preset 108 2:255")
recs=run(seed,P,2900)
d=[u for r in recs if r["k"]=="snap" for u in r["units"] if u["ut"]==1 and u["cl"]==243]
print(seed, "DIABLO", "present" if d else "absent", d[0]["g"] if d else "", "at f", min(r["f"] for r in recs if r["k"]=="snap" and any(u["ut"]==1 and u["cl"]==243 for u in r["units"])) if d else "")
if d:
    dg=d[0]["g"]
    for t,(ox,oy) in zip((2905,2913,2921,2929,2937,2945),(("+3",""),("-3",""),("","+3"),("","-3"),("+2","+2"),("-2","-2"))):
        P+= [f"{t} pos 1/{dg} @x{ox} @y{oy}", f"{t} stat 1/{dg} 6 0 256", f"{t} missile 59 @x @y @x{ox} @y{oy} skill 39 1"]
    recs=run(seed,P,3100)
    k=[r["f"] for r in recs if r["k"]=="snap" for u in r["units"] if u["ut"]==1 and u["g"]==dg and u["m"] in (0,12)]
    print(seed,"DIABLO killed" if k else "DIABLO alive", k[0] if k else "")
if "-v" in sys.argv:
    for p in P: print("POKE", p)
