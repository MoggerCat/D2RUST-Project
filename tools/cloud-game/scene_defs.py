"""Scene groups for scenes.py: character, seed, input script (ticks, marks), scenes."""
GROUPS = {
    "panels": {
        "char": "SceSor", "seed": 1234,
        "script": "waitticks 20; mark idle; key I; waitticks 30; mark inv; click 678 330; waitticks 10; "
                  "click 563 250; waitticks 15; mark invbelt; key I; waitticks 20; "
                  "key 0xC0; waitticks 30; mark belt; key 0xC0; waitticks 15; "
                  "key C; waitticks 30; mark char; key C; waitticks 20; "
                  "key T; waitticks 30; mark skill; key T; waitticks 20; "
                  "key TAB; waitticks 30; mark automap; key TAB; waitticks 20; "
                  "key I; waitticks 10; rclick 446 345; waitticks 30; mark cube; key ESC; waitticks 20; "
                  "key ESC; wait 4; mark esc; key ESC; wait 2; end",
        "scenes": {"a1-panel-inventory": ("inv", 0), "a1-panel-character": ("char", 0),
                   "a1-panel-skilltree": ("skill", 0), "a1-panel-automap": ("automap", 0),
                   "a1-panel-esc-menu": ("esc", 0, "last"), "a1-panel-cube": ("cube", 0),
                   "a1-panel-belt-open": ("belt", 0)},
    },
}

for _a, _n in ((2, "lut-gholein"), (3, "kurast-docks"), (4, "pandemonium-fortress"), (5, "harrogath")):
    GROUPS[f"act{_a}town"] = {
        "char": f"SceAct{_a}", "seed": 1234,
        "script": "waitticks 40; mark t; waitticks 10; end",
        "scenes": {f"a{_a}-town-{_n}": ("t", 0)},
    }
GROUPS["a1town"] = {
    "char": "SceSor", "seed": 1234,
    "script": "waitticks 40; mark t; waitticks 10; end",
    "scenes": {"a1-town-idle-sor": ("t", 0)},
}

GROUPS["npc"] = {
    "char": "SceSor", "seed": 1234,
    "script": "goto 2 267; waitticks 70; mark s0; waitticks 30; mark s1; "
              "goto 1 150; waitticks 70; mark n0; waitticks 30; mark n1; waitticks 10; end",
    "scenes": {"s0": ("s0", 0), "s1": ("s1", 0), "n0": ("n0", 0), "n1": ("n1", 0)},
}

# walking and running in the 8 screen directions (SceSor, Rogue Encampment): click a point 200 px
# from the player's screen position (400, 284); the frame is the first one 10 ticks after the click
_DIRS = (("n", 400, 184), ("s", 400, 384), ("ne", 541, 213), ("sw", 259, 355),
         ("e", 600, 284), ("w", 200, 284), ("se", 541, 355), ("nw", 259, 213))
_walk = "waitticks 20; "
for _n, _x, _y in _DIRS:
    _walk += f"click {_x} {_y}; waitticks 10; mark w_{_n}; waitticks 4; "
_walk += "key R; waitticks 6; "
for _n, _x, _y in _DIRS:
    _walk += f"click {_x} {_y}; waitticks 10; mark r_{_n}; waitticks 4; "
GROUPS["walk"] = {
    "char": "SceSor", "seed": 1234, "script": _walk + "waitticks 6; end",
    "scenes": {**{f"a1-walk-{n}": (f"w_{n}", 0) for n, _, _ in _DIRS},
               **{f"a1-run-{n}": (f"r_{n}", 0) for n, _, _ in _DIRS}},
}

# Cold Plains by waypoint, fixed clicks only (no unit lookup), so every input lands on the same tick:
# walk to the Act I waypoint, open it, take Cold Plains, cast Fire Bolt into the Fallen group
GROUPS["fight"] = {
    "char": "SceSor", "seed": 1234,
    "script": "waitticks 20; click 700 300; waitticks 60; click 650 300; waitticks 40; click 590 262; waitticks 50; "
              "click 207 176; waitticks 80; mark cp; "
              + "".join(f"click 330 190; waitticks 12; click 300 200; waitticks 13; mark f{i}; " for i in range(1, 15)) + "end",
    "scenes": {"a1-cold-plains-monsters": ("cp", 0), "a1-cold-plains-corpse": ("f3", 0), "a1-cold-plains-drop": ("f9", 0)},
}
