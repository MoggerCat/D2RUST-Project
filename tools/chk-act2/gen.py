#!/usr/bin/env python3
"""Generates the Act II NPC / quest / waypoint / boss checks (traces/checks/a2-*.check)
for q-chk-act2. Our own code. Usage: python3 tools/chk-act2/gen.py [--out DIR]"""
import os, sys
OUT = os.path.join(os.path.dirname(__file__), "..", "..", "traces", "checks")
SAVE = "save ScnAm2 --class ama --expansion --gold 10000 --act 1 --quests acts=1"
# class, name, [(label, [send lines after the interact+chat pair])]
NPCS = [
    (177, "drognan", [("trade", ["EntityAction action=1 npc=@1:177 item=0"])]),
    (178, "fara", [("trade", ["EntityAction action=1 npc=@1:178 item=0"])]),
    (199, "elzix", [("trade", ["EntityAction action=1 npc=@1:199 item=0"]),
                    ("gamble", ["EntityAction action=2 npc=@1:199 item=0"])]),
    (202, "lysander", [("trade", ["EntityAction action=1 npc=@1:202 item=0"])]),
    (176, "atma", []), (198, "greiz", [("hirelist", ["EntityAction action=3 npc=@1:198 item=0"])]),
    (201, "jerhyn", []), (210, "meshif", []), (175, "warriv", []), (244, "cain", []),
]
def npc_check(cls, name, label, sends):
    L = ["check 1", f"# Act II NPC {name} (monstats {cls}), {label}: goto the NPC, interact (0x13),",
         "# open the chat (0x2F), the menu action, close (0x30); state + packets.",
         f"name a2-npc-{name}-{label}", SAVE, "seed 1234", "ticks 50", "seconds 300", "channels state packets",
         f"at 4 poke goto unit 1:{cls}",
         f"at 14 send InteractWithEntity type=1 id=@1:{cls}", f"at 16 send InitEntityChat id=@1:{cls}"]
    f = 18
    for s in sends:
        L.append(f"at {f} send {s}"); f += 3
    L.append(f"at {f} send TerminateEntityChat id=@1:{cls}")
    return "\n".join(L) + "\n"
HDR = ["seed 1234", "seconds 600", "channels state packets"]
def mk(name, desc, ticks, lines, save=SAVE):
    L = ["check 1"] + ["# " + d for d in desc.split("\n")] + [f"name {name}", save] + HDR[:1] + [f"ticks {ticks}"] + HDR[1:] + lines
    return "\n".join(L) + "\n"
def talk(f, cls, msgs=(), acts=()):
    """interact (0x13), chat (0x2F), quest messages (0x31), menu actions (0x38), close (0x30)."""
    o = [f"at {f} send InteractWithEntity type=1 id=@1:{cls}", f"at {f+2} send InitEntityChat id=@1:{cls}"]
    t = f + 4
    for m in msgs:
        o.append(f"at {t} send QuestMessage npc=@1:{cls} msg={m}"); t += 2
    for a in acts:
        o.append(f"at {t} send EntityAction action={a} npc=@1:{cls} item=0"); t += 2
    o.append(f"at {t} send TerminateEntityChat id=@1:{cls}")
    return o, t + 2
def kill(f, cls, lvl=None):
    """goto the monster, set life 1 beside the player, hit it with a missile (playthrough act2.play)."""
    o = [f"at {f} poke goto unit 1:{cls}"]
    f += 12
    o += [f"at {f} poke pos @1:{cls} @x+3 @y", f"at {f+2} poke stat @1:{cls} 6 0 256",
          f"at {f+4} poke missile 58 @x @y @x+3 @y"]
    return o, f + 30
def quest_checks():
    C = {}
    Q = lambda extra="": f"save ScnAm2 --class ama --expansion --act 1 --quests acts=1{extra}"
    # Q1 Radament (slot 9): Atma 304 -> Sewers 3 -> kill -> Atma 334
    t1, f = talk(14, 176, msgs=[304])
    k, f = kill(f + 4 if False else 80, 229)
    t2, f2 = talk(f + 20, 176, msgs=[334])
    C["a2-quest-radament"] = mk("a2-quest-radament", "Quest 9 Radament's Lair (quests-act2.md s3): Atma msg 304, warp to Sewers 3 (49),\nkill Radament by poke, Atma msg 334; packets carry the 0x5D flag words.", f2 + 30,
        ["at 4 poke goto unit 1:176"] + t1 + ["at 60 poke warp 49"] + k + [f"at {f+14} poke warp 40", f"at {f+16} poke goto unit 1:176"] + t2, Q())
    # Q2 Horadric Staff (slot 10): Cain msgs 335/336
    t, f = talk(14, 244, msgs=[335, 336])
    C["a2-quest-staff"] = mk("a2-quest-staff", "Quest 10 The Horadric Staff (s4.5): Cain msgs 335 then 336.", f + 20, ["at 4 poke goto unit 1:244"] + t, Q())
    # Q3 Tainted Sun (slot 11): Drognan 348, altar (object 149, Lost City 44), msg 362
    t1, f = talk(14, 177, msgs=[348])
    C["a2-quest-taintedsun"] = mk("a2-quest-taintedsun", "Quest 11 Tainted Sun (s5): Drognan msg 348, warp 44, operate the altar (object 149), msg 362.", f + 60,
        ["at 4 poke goto unit 1:177"] + t1 + [f"at {f+2} poke warp 44", f"at {f+6} poke goto unit 2:149", f"at {f+30} send InteractWithEntity type=2 id=@2:149", f"at {f+40} send QuestMessage npc=@2:149 msg=362"], Q())
    # Q4 Arcane Sanctuary (slot 12): Drognan 373, Jerhyn 377, journal (object 357), msg 396
    t1, f = talk(14, 177, msgs=[373]); t2, f2 = talk(f + 4, 201, msgs=[377])
    C["a2-quest-arcane"] = mk("a2-quest-arcane", "Quest 12 Arcane Sanctuary (s6): Drognan 373, Jerhyn 377, warp to the Palace Cellar (52),\noperate Horazon's journal (object 357) and msg 396 (the portal).", f2 + 80,
        ["at 4 poke goto unit 1:177"] + t1 + [f"at {f+2} poke goto unit 1:201"] + t2 + [f"at {f2+2} poke warp 52", f"at {f2+6} poke goto unit 2:357", f"at {f2+40} send InteractWithEntity type=2 id=@2:357", f"at {f2+46} send QuestMessage npc=@2:357 msg=396"], Q(" 11.0"))
    # Q5 Summoner (slot 13): Arcane 74, kill Summoner 250
    k, f = kill(14, 250)
    C["a2-quest-summoner"] = mk("a2-quest-summoner", "Quest 13 The Summoner (s7): warp to the Arcane Sanctuary (74), kill the Summoner by poke.", f + 20, ["at 4 poke warp 74"] + [x.replace("at 14 ", "at 14 ") for x in k], Q(" 12.0"))
    # Q6 Seven Tombs (slot 14): Jerhyn 430, orifice 152, Duriel 211, Tyrael 251 msg 302, Jerhyn 442, Meshif 450
    t1, f = talk(14, 201, msgs=[430])
    k, fk = kill(f + 50, 211)
    t2, f2 = talk(fk + 20, 251, msgs=[302]); t3, f3 = talk(f2 + 20, 201, msgs=[442]); t4, f4 = talk(f3 + 20, 210, msgs=[450])
    C["a2-quest-tombs"] = mk("a2-quest-tombs", "Quest 14 The Seven Tombs (s8.11): Jerhyn 430, kill Duriel (73) by poke, Tyrael 302, Jerhyn 442, Meshif 450.", f4 + 30,
        ["at 4 poke goto unit 1:201"] + t1 + [f"at {f+10} poke warp 73"] + k + [f"at {fk+4} poke goto unit 1:251"] + t2 + [f"at {f2+4} poke warp 40", f"at {f2+8} poke goto unit 1:201"] + t3 + [f"at {f3+4} poke goto unit 1:210"] + t4, Q(" 13.0"))
    return C
WPS = [(40, 156), (42, 156), (43, 156), (44, 156), (46, 402), (48, 323), (52, 288), (57, 288), (74, 288)]
SUPERS = [(12, 61, "fangskin"), (13, 43, "beetleburst"), (14, 46, "leatherarm"), (15, 64, "coldworm"), (16, 54, "fireeye"), (17, 44, "darkelder")]
def other_checks():
    C = {}
    for lvl, cls in WPS:
        C[f"a2-wp-{lvl}"] = mk(f"a2-wp-{lvl}", f"Act II waypoint on level {lvl} (object {cls}): warp, operate (0x13 type 2), open menu; take the waypoint to level 40 (0x49).", 50,
            [f"at 4 poke warp {lvl}", f"at 8 poke goto unit 2:{cls}", f"at 24 send InteractWithEntity type=2 id=@2:{cls}", f"at 30 send TakeOrCloseWp wp=@2:{cls} level=40"] if lvl != 40 else
            [f"at 4 poke goto unit 2:{cls}", f"at 24 send InteractWithEntity type=2 id=@2:{cls}", f"at 30 send TakeOrCloseWp wp=@2:{cls} level=42"], f"save ScnAm2 --class ama --expansion --act 1 --quests acts=1 --waypoints all")
    for row, lvl, nm in SUPERS:
        C[f"a2-super-{nm}"] = mk(f"a2-super-{nm}", f"Act II superunique {nm} (superuniques row {row}): warp {lvl}, spawn beside the player, kill it by poke; the state channel carries minions and drops.", 120,
            [f"at 4 poke warp {lvl}", f"at 10 poke superunique {row} @x+6 @y", "at 30 poke goto unit 1:0" if False else "at 30 poke missile 58 @x @y @x+6 @y"])
    C["a2-npc-fara-heal"] = mk("a2-npc-fara-heal", "Fara (178) heals: life set low by poke, then the interaction (npc.md s8 services).", 50,
        ["at 4 poke goto unit 1:178", "at 8 poke stat @player 6 0 5120", "at 14 send InteractWithEntity type=1 id=@1:178", "at 16 send InitEntityChat id=@1:178", "at 20 send TerminateEntityChat id=@1:178"])
    C["a2-npc-greiz-hire"] = mk("a2-npc-greiz-hire", "Greiz (198) hires a Rogue-class mercenary of act 2 (0x36 after the hire list 0x38 action 3).", 60,
        ["at 4 poke goto unit 1:198", "at 14 send InteractWithEntity type=1 id=@1:198", "at 16 send InitEntityChat id=@1:198", "at 18 send EntityAction action=3 npc=@1:198 item=0", "at 24 send HireMerc npc=@1:198 merc=0", "at 30 send TerminateEntityChat id=@1:198"])
    return C
def main():
    out = OUT
    if "--out" in sys.argv: out = sys.argv[sys.argv.index("--out") + 1]
    n = 0
    for cls, name, acts in NPCS:
        for label, sends in (acts or [("talk", [])]):
            open(os.path.join(out, f"a2-npc-{name}-{label}.check"), "w").write(npc_check(cls, name, label, sends)); n += 1
    for fn, c in list(quest_checks().items()) + list(other_checks().items()):
        open(os.path.join(out, fn + ".check"), "w").write(c); n += 1
    print(n, "checks")
main()
