# local-buddy q-saves: HANDOFF C66 (character saves), 1.14d

Run 2026-10-07 on the user's 1.14d install; entry 66, `LOCAL-RUN.md` 2.18 and 6.7. No save bytes are committed; values only.

## Part 1: `real_saves_round_trip` (2.18)

`cargo test --release -p d2s-tool --test real_saves -- --ignored --nocapture`: **PASS**, 13 of 13 saves parse in `d2s.md` §1 order and rewrite byte for byte; `d2s-tool check` prints OK for each.
bdAma 981, bdSor 958, bdNec 958, bdPal 987, bdBar 987, bdDru 987, bdAss 980, bdDead 974, bdMerc 985, bdMercTwo 1079, bdGolem 967, ScnSor 958, ScnAma 981 bytes. All version 0x60, expansion (status 0x20; bdDead, bdGolem, bdMercTwo 0x28 = dead flag).
The "Claim once" line became "// Covers:" and `py tools/coverage.py --check` passes.

Field values (d2s OQ3):
- Header +0x10 weapon switch 0, +0x26 = 0, +0x29 = 0x10, +0x2A = 30, +0x34 = 0xFFFFFFFF, +0xCF = 0 in every save. +0x2C create time was 0 in the dumps of the first save (bdSor). Town bytes (+0xA8) = 0x80, 0, 0 in all 13 (Normal, act 1).
- Mouse skills (left, right): fresh Amazon/Assassin/Barbarian-merc/Dead/Merc saves 0/0; Sorceress 0/36; Necromancer 0/70; Paladin 0/98; Barbarian bdBar 0/126; Druid 0/221; bdGolem 0/75. All item indexes -1, hotkeys empty.
- +0x88 appearance components (16 bytes): 0xFF except slot 5 by class (Ama 0x1B, Sor 0x25, Nec 0x09, Pal 0x11, Bar 0x04, Dru 0x0C, Ass 0x2D) and slot 7 = 0x4F for Ama, Pal, Bar, Dru, Ass (not Sor, Nec). bdGolem has slot 6 = 0x09, bdMercTwo slot 6 = 0x04 plus slot 7 = 0x4F; bdDead all 0xFF. +0x98 colours all 0xFF in every save.
- Stats at 0x2FD (stat id = stored value): Amazon level 1: 0=20, 1=15, 2=25, 3=20, 6/7=12800, 8/9=3840, 10/11=21504, 12=1. Sorceress: 10, 35, 25, 10, 10240, 8960, 18944. Necromancer: 15, 25, 25, 15, 11520, 6400, 20224. Assassin: 20, 25, 20, 20, 12800, 6400, 24320. Fresh Barbarian (bdMerc): 30, 10, 20, 25, 6=11887, 7=14080, 8/9=2560, 10/11=23552, stat 13 (experience) = 54. Levelled saves add stats 4/5 (unused points), 12, 13, and for bdMercTwo 14 (gold 136). All ids < itemstatcost count and stream ended with 0x1FF (reader accepted).
- `jf`: marker only, no list, in every save except bdMercTwo (hireling present, 0 hireling items). `kf`: flag 0 in every save (all expansion, including Necromancer). bdMercTwo hireling: present, flags 0, name index 21, experience 39482.
- Header +0x38 hotkeys and the 30 skill bytes: all zero (skill bytes begin at the class's first skill id: Sor 36, Nec 66).

## Part 2: generated characters in the game (6.7)

Save folder backed up (copy) to `C:\Users\zffit\Desktop\D2test\saves-backup-<time>\` first (52 files). Created TestAma (`new`, classic, 843 bytes), TestSor (`new`, level 30 expansion, acts=4, hp1 and lsd@0,0, 898 bytes) and TestStub (`new-stub`, 335 bytes), all `check` OK before the run; copied into the save folder (names did not collide).
All three **appear in the character select** with the right class and level (TestSor with the Champion title), and all three **entered a single-player game**, the Normal difficulty chosen for TestSor (difficulty dialog appears for expansion characters), and were saved with Save and Exit (automation `d2ui.py` / `make_saves.py` pattern, WM_CLOSE shutdown). The game refused nothing; no message. Game.exe not running afterwards, game.lock released.

`check` on the three game-re-saved files: OK, byte-for-byte rewrite.

`dump` ours vs the game's:
- TestAma: only checksum, save time (+0x30) and +0x88 components differ. Ours wrote components 01 at slots 0-4, 8, 9 (the stub fill), the game wrote 0xFF in all 16. Stats, quests, waypoints, skills, NPC flags identical.
- TestSor: same three differences plus the two item records: item flags 0x00A02010 -> 0x00A00010 (hp1) and 0x00802010 -> 0x00800010 (lsd), i.e. the game cleared flag 0x2000 (instore) on load. Item codes, positions, quality, ilvl, bit lengths (14 and 23 bytes), trailer and stats identical, so the 1-bit-0 trailer and unit +0x28 choices were accepted. Waypoints and quest words survived the game's re-save unchanged (act and waypoint state as written by `new`).
- TestStub: the game completed the stub: file 335 -> 953 bytes, status 0, +0x34 = 0xFFFFFFFF, create time 0, town 0x80, map seed generated, class skills, quest/waypoint/NPC sections, stats (Necromancer level 1 base), 7 starting items (4 hp1, tsc, isc, wnd) at version 2 item records, `jf`/`kf` absent (classic). Expected for a stub.

Findings to fold into specs (not applied here):
1. `items/bitstream.md` §5 / `inventory.md` §5.7: the game clears item flag 0x2000 (instore) at load/re-save; `new` keeps it. Writer should not set 0x2000 on a hand-built item (no other item-record difference).
2. `d2s.md` §2.1 +0x88: components of a generated save are rewritten by the game to 0xFF for TestAma/TestSor with no equipment graphics; the stub fill 01 is not preserved. Not an error for loading.
3. `d2s.md` OQ3 answered for 13 saves (above). Classic character: `jf`/`kf` absent after the re-save; expansion characters: `jf` marker and `kf` flag 0.

Not committed: saves, dumps, screenshots (in `out-q-saves\`).
