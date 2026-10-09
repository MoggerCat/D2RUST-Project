#!/usr/bin/env python3
"""Writes the Act 5 checks of q-chk-act5 (REC-14100) into traces/checks/.
    python3 tools/chk-act5/gen.py
Spec: specs/tools/scenario-diff.md (check files), specs/tools/poke.md
(goto, superunique, msg, missile). Inputs only; no game data."""
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "..", "traces", "checks")
SAVE = "ScnAm5 --class ama --expansion --act 4 --quests acts=4 --waypoints all"
SAVEK = "ScnSo5 --class sor --expansion --act 4 --quests acts=4 --waypoints all"


def write(name, comment, body, ticks, seconds=300, save=SAVE, channels="state"):
    text = ["check 1"] + ["# " + c for c in comment] + [
        f"name {name}", f"save {save}", "seed 1234", f"ticks {ticks}",
        f"seconds {seconds}", f"channels {channels}"] + body
    with open(os.path.join(OUT, name + ".check"), "w") as f:
        f.write("\n".join(text) + "\n")


# NPC talk: Harrogath (109), msg 0x13 type 1 (monster/NPC) to the NPC by class.
NPCS = {"larzuk": 511, "malah": 513, "nihlathak-town": 514, "qual-kehk": 515, "cain5": 520}
for n, cl in NPCS.items():
    write(f"a5-npc-{n}",
          [f"Act 5 NPC interaction (q-chk-act5): walk next to class {cl} (poke goto), talk (C->S 0x13 type 1).",
           "Compares the unit table after the talk (mode, state); the menu/store contents are the packets channel (open)."],
          [f"at 4 poke goto preset 109 1:{cl}", f"at 150 poke msg 0x13 1 @1:{cl}"], 190)

# Waypoints: warp to the level, walk to the waypoint object, open it (0x13 type 2), travel back to Harrogath (0x49).
WPS = [(31, 111, 496), (32, 112, 496), (33, 113, 511), (34, 115, 511), (35, 123, 496),
       (36, 117, 496), (37, 118, 511), (38, 129, 494)]
for wp, lv, ob in WPS:
    write(f"a5-wp-{wp}-lv{lv}",
          [f"Act 5 waypoint {wp} (level {lv}, object {ob}): open it (0x13 type 2), then travel to Harrogath (0x49 wp 30, level 109)."],
          [f"at 4 poke warp {lv}", f"at 20 poke goto preset {lv} 2:{ob}", f"at 450 poke msg 0x13 2 @2:{ob}",
           "at 470 send TakeOrCloseWp wp=30 level=109"], 540, 420)
write("a5-wp-30-harrogath", ["Act 5 waypoint 30 (Harrogath, object 429): open it, travel to Rigid Highlands (wp 31, level 111)."],
      ["at 4 poke goto preset 109 2:429", "at 150 poke msg 0x13 2 @2:429", "at 170 send TakeOrCloseWp wp=31 level=111"], 220)

# Superuniques / act bosses: row -> (name, monstats class); spawn next to the player in Bloody Foothills (110),
# life set to 1, Fire Bolt at it; drops show as ut 4 units. Natural placement: milestone-nihlathak / -baal-* checks.
SU = [(42, "shenk", 479), (43, "ancient1", 540), (44, "ancient2", 541), (45, "ancient3", 542),
      (46, "axe-dweller", 508), (47, "bonesaw-breaker", 437), (48, "dac-farren", 494),
      (49, "megaflow-rectifier", 453), (50, "eyeback", 529), (51, "threash-socket", 443),
      (52, "pindleskin", 440), (53, "snapchip", 501), (54, "anodized-elite", 472),
      (55, "vinvear-molech", 475), (56, "sharp-tooth-sayer", 481), (57, "magma-torquer", 496),
      (58, "blaze-ripper", 533), (59, "frozenstein", 449), (60, "nihlathak-boss", 526)]
for row, n, cl in SU:
    write(f"a5-su-{n}",
          [f"Act 5 superunique row {row} (class {cl}) spawn + kill + drops (q-chk-act5): spawned beside the player in level 110",
           "(poke superunique), life set to 1, killed with a Fire Bolt missile poke. Drops are ut 4 units in the state."],
          ["at 4 poke warp 110", f"at 12 poke superunique {row} @x+4 @y", f"at 14 poke stat @1:{cl} 6 0 256",
           "at 16 poke missile 58 @x @y @x+4 @y skill 36 1"], 90, save=SAVEK)
