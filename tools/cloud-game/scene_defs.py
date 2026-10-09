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
