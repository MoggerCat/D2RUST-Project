"""Create real 1.14d single-player test characters by driving Game.exe -w -ns.

Usage:  py tools/trace-recorder/make_saves.py classes|merc|dead|all [--game DIR]
Own code, stdlib only. Saves land in %USERPROFILE%\\Saved Games\\Diablo II and are
never committed. Screenshots go to C:\\Users\\zffit\\Desktop\\D2test\\out-saves (outside the repo).
Coordinates are screenshot pixels of the client area (1200x900 at 150% DPI; the game
reads window-message coordinates as those physical pixels). In-game keys and mouse
clicks go through PostMessage (SendInput scancodes are ignored in-game).
"""
import os, sys, time, subprocess, argparse
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d2ui
from d2ui import VK

OUT = r"C:\Users\zffit\Desktop\D2test\out-saves"
LOCK = r"C:\d2slots\game.lock"
SAVES = os.path.join(os.environ["USERPROFILE"], "Saved Games", "Diablo II")
GAME_DIR = r"C:\Users\zffit\Desktop\D2test\D2RUST-Project\game"

CLASSES = [("bdAma", "Amazon", 150), ("bdAss", "Assassin", 300), ("bdNec", "Necromancer", 440),
           ("bdBar", "Barbarian", 600), ("bdPal", "Paladin", 790), ("bdSor", "Sorceress", 920),
           ("bdDru", "Druid", 1080)]
CLASS_X = {n: x for _, n, x in CLASSES}
SCN = [("ScnSor", "Sorceress"), ("ScnAma", "Amazon")]   # names the .scenario files load


def have(name):
    return os.path.exists(os.path.join(SAVES, name + ".d2s"))


class Game:
    def __init__(self, gamedir):
        self.gamedir = gamedir; self.proc = None; self.h = None; self.n = 0

    def start(self):
        self.proc = subprocess.Popen([os.path.join(self.gamedir, "Game.exe"), "-w", "-ns"], cwd=self.gamedir)
        self.h = d2ui.find_window(self.proc.pid, 90)
        if not self.h:
            raise RuntimeError("no game window")
        time.sleep(12); d2ui.focus(self.h)

    def stop(self):
        if not self.proc: return
        if self.h and d2ui.u32.IsWindow(self.h):
            d2ui.u32.PostMessageW(self.h, 0x10, 0, 0)   # WM_CLOSE (kill may be refused)
            for _ in range(20):
                if self.proc.poll() is not None: return
                time.sleep(0.5)
        try: self.proc.kill()
        except OSError: pass
        if self.proc.poll() is None:
            subprocess.call(["taskkill", "/F", "/PID", str(self.proc.pid)])

    def shot(self, tag):
        self.n += 1
        d2ui.screenshot(self.h, os.path.join(OUT, f"{tag}-{self.n:02d}.png"))

    def click(self, x, y, wait=1.0):
        d2ui.pclick(self.h, x, y); time.sleep(wait)

    def key(self, vk, wait=0.5):
        d2ui.pkey(self.h, vk); time.sleep(wait)

    # --- menus ---
    def create(self, name, cls):
        self.click(600, 462, 2.0)                     # Single Player
        if any(f.endswith(".d2s") for f in os.listdir(SAVES)):
            self.click(175, 750, 1.5)                 # Create New Character (list shown)
        self.click(CLASS_X[cls], 480, 1.0)
        self.click(605, 760, 0.5)                     # name field
        d2ui.typetext(name); time.sleep(0.5)
        self.shot(name + "-named")
        self.click(1037, 832, 5.0)                    # OK -> into Normal game (Rogue Encampment)
        self.shot(name + "-ingame")

    def save_exit(self, name):
        self.key(VK["ESC"], 1.0)
        self.shot(name + "-menu")
        self.click(600, 390, 6.0)                     # Save And Exit Game
        self.shot(name + "-exited")


def orb_red(g):
    return d2ui.getpixel(g.h, 105, 830)[0]      # ~92 while life shows, <30 when dead


def run_dead(g, name="bdDead", cls="Sorceress", budget_s=1500):
    """Walk out of town and let monsters kill the character, then ESC (respawn in town,
    corpse stays in the field) and Save And Exit WITHOUT touching the corpse.
    The route and the wander are probabilistic; the real run needed ~25 min of
    walking (partly by hand-steered clicks), so this replay may stall: check the screenshots."""
    import random
    g.create(name, cls)
    for x, y in [(1080, 725), (1080, 725), (1000, 600), (1150, 560), (1150, 560)]:
        g.click(x, y, 8.0)                           # town -> Blood Moor gate
    t0 = time.time(); n = 0
    while time.time() - t0 < budget_s:
        x = random.choice([60, 120, 250, 1100]); y = random.choice([100, 200, 650, 750])
        g.click(x, y, 3.5); n += 1
        if n % 4 == 0:
            g.shot(name + "-walk")
            if orb_red(g) < 30:
                break
    else:
        raise RuntimeError("character did not die within budget")
    time.sleep(2); g.shot(name + "-dead")
    g.key(VK["ESC"], 4.0)                            # "You have died. Press ESC to continue"
    g.save_exit(name)


def probe_kashya(g, name="bdMerc", cls="Barbarian"):
    """Create a char, click Kashya (south-east of the start point) and screenshot her menu.
    Finding on 1.14d: a fresh char only gets TALK / CANCEL (no HIRE): hiring below
    level 8 needs Sisters' Burial Grounds (Blood Raven) done, plus gold."""
    g.create(name, cls)
    g.click(1083, 725, 7.0); g.shot(name + "-kashya")
    g.click(690, 265, 1.0)                           # CANCEL
    g.save_exit(name)


def with_lock(fn):
    os.makedirs(os.path.dirname(LOCK), exist_ok=True)
    while True:
        try:
            fd = os.open(LOCK, os.O_CREAT | os.O_EXCL); os.close(fd); break
        except FileExistsError:
            print("game.lock held, waiting 60 s"); time.sleep(60)
    try:
        fn()
    finally:
        try: os.remove(LOCK)
        except OSError: pass


def run_classes(g):
    for name, cls, _ in CLASSES:
        if have(name):
            print(name, "exists, skipped"); continue
        g.create(name, cls); g.save_exit(name)
        print(name, "done")


def run_scn(g):
    for name, cls in SCN:
        if have(name):
            print(name, "exists, skipped"); continue
        g.create(name, cls); g.save_exit(name)
        print(name, "done")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("what", choices=["classes", "merc", "dead", "all", "scn"])
    ap.add_argument("--game", default=GAME_DIR)
    a = ap.parse_args()
    os.makedirs(OUT, exist_ok=True)
    g = Game(a.game)

    def go():
        g.start()
        try:
            if a.what in ("classes", "all"): run_classes(g)
            if a.what == "scn": run_scn(g)
            if a.what in ("merc", "all") and not have("bdMerc"): probe_kashya(g)
            if a.what in ("dead", "all") and not have("bdDead"): run_dead(g)
        finally:
            g.stop()
    with_lock(go)
    for f in sorted(os.listdir(SAVES)):
        print(f, os.path.getsize(os.path.join(SAVES, f)))


if __name__ == "__main__":
    main()

