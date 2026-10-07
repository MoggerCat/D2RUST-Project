# PC 2 recording list: NPC, vendors, cube, waypoints

Branch `claude/pc2-npc-vendors`. Questions from `specs/world/npc.md`,
`vendors.md`, `cube.md`, `waypoints.md` that the 1.14d binary could not
settle within two function reads. The coordinator merges these entries
into `docs/HANDOFF.md` §7 "PC 2 recording list".

Common setup: Windows, `game/Game.exe` with the reference hash
(`tools/trace-recorder/README.md`), single player, expansion character.
"Packets" = `py tools/trace-recorder/record_packets.py --seconds <N>`
then `py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl`.
"RNG" = `py tools/trace-recorder/record_rng.py --seconds <N>` then
`py tools/trace-recorder/convert_rng.py traces/raw/<file>.jsonl --list`.
Play by hand inside the recording window; note the in-game time of each
step so frames can be matched.

| Id | Spec + question | Steps (recorder) | What to look for |
|---|---|---|---|
| R-NV-1 | `world/npc.md` OQ5, OQ6 (resurrect) | Packets, 300 s. Character level ≥ 9 in Act I with ≥ 2,000 gold, a hired Rogue (hire at Kashya first if none). 1. Leave town to the Blood Moor, let the Rogue die (stand still near a pack). 2. Town portal or walk back. 3. Click Kashya, choose "Resurrect". 4. Close the dialog (Esc). | C→S 0x62; S→C order in that frame: 0x9B (expect `9b ffff 00000000`), 0x81, 0x2A code 5; gold message 0x1D next frame. Same entry as the `world/hirelings.md` R2-27 resurrect line (record once, log both). |
| R-NV-2 | `world/npc.md` OQ6 (heal, Cain) | Packets, 240 s. Act I, Cain rescued (Tristram done), life and mana below max (take a hit outside town, return), 3 unidentified items in the inventory and ≥ 300 gold. 1. Click Akara (her menu opens; the client sends C→S 0x13 then 0x2F). 2. Esc. 3. Click Cain, choose "Identify Items". 4. Esc. 5. Click Cain again with nothing unidentified, choose "Identify Items". | Akara: after C→S 0x2F, SetStat 0x1D–0x1F for life / mana / stamina in that frame, then the sound-10 event (`npc.md` §5); Cain: C→S 0x34, one S→C 0x2A code 3 after the item updates (not one per item), gold −300; second identify: 0x2A code 9. |
| R-NV-3 | `world/npc.md` OQ6 (services) | Packets, 600 s, one run per save. Save A: Act I, Tools of the Trade reward pending (Malus returned to Charsi, imbue not yet used), a normal-quality socketless Long Sword in the inventory. Save B: Act V, Siege on Harrogath done, socket reward unused, a normal socketless Crystal Sword. Save C: Act V, Betrayal of Harrogath done, personalize unused, a rare body armor. Save D (only if one exists): Hell character with the Den of Evil reward unused. A: click Charsi → "Imbue" → put the sword on the cursor into the dialog. B: Larzuk → "Socket" → the sword. C: Anya → "Personalize" → the armor. D: Akara → "Reset" → confirm. Esc after each. | C→S 0x38 (action, NPC GUID, item GUID), then S→C 0x58 result 6 and the item messages (0x9C) in order; socket count for the normal sword = its `MaxSock` cap (`npc.md` §8.1); respec: the skill and stat reset messages. |
| R-NV-4 | `world/npc.md` OQ7 (talk on arrival) | Packets, 120 s. Rogue Encampment. 1. Stand 7–8 sub-tiles (about 2 player widths) from Akara. 2. Click Akara once and let the character run to her. 3. Esc. | One C→S 0x13 only; S→C run/path messages, then 0x27, 0x29, 0x28 in one frame when the character stops, with no second C→S 0x13. |
