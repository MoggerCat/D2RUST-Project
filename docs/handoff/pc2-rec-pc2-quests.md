# PC 2 recording list: quests lane (`claude/pc2-quests`)

Open questions of `specs/world/quests*.md` that the 1.14d binary cannot
settle within two function reads. Each entry: id, spec + question, the
recording, the steps, what to look for. Recorder commands (Windows,
`game/`, `tools/trace-recorder/README.md`):

- packets: `py tools/trace-recorder/record_packets.py --seconds 300`
  then `py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl`
- RNG: `py tools/trace-recorder/record_rng.py --seconds 300` (full
  inline; run beside or after the packets run on the same save)
- tick: `py tools/trace-recorder/record_tick.py --seconds 300`

"= HANDOFF ..." marks an entry already queued there: record it once.

| Id | Spec + question | Recording | Player steps | Look for |
|---|---|---|---|---|
| R-PQ-1 | `quests.md` OQ2 (Flavie's chat, edge case 2) | packets + RNG | Expansion character in Act I, Normal, A1Q2 not done; walk to Flavie (Cold Plains exit of the Rogue Encampment), talk to her once, close, talk once more; quit | per talk: the number of player-seed draws between the C→S 0x2F/0x31 chat and the S→C 0x27; how many message lines the 0x27 lists (two records, chains 25 and 30, share Flavie's event 0) |
| R-PQ-2 | `quests.md` OQ3 (cow portal; `world/cube.md` OQ4) | packets + RNG | Character that killed Baal on that difficulty (cow level allowed), in the Rogue Encampment with Wirt's Leg and a Tome of Town Portal in the cube; press Transmute | the RNG draws between C→S 0x4F 0x18 and the portal's S→C 0x51; the portal object class / mode |
| R-PQ-3 | `quests.md` OQ4 / §3 (new character: mode 1 then mode 0) | packets | Create a new single-player character (any class), enter the game, wait 5 s, quit | 0x5E, 0x28 (type 6), 0x29 each sent **twice** at game entry (code reading `0x0056A072` then `0x005344EF`); an existing character's entry sends each once |
| R-PQ-4 | `quests.md` OQ10 (0x61 bytes per act change; = HANDOFF §5 S9-A1 (7) for the change itself) | packets | Character with every act open: take each act change once (Warriv I→II, Meshif II→III, the Act III Hellgate red portal III→IV, Tyrael's portal IV→V), then waypoint back to an earlier act | every S→C 0x61 against `quests.md` §8.1 |
| R-PQ-5 | `quests.md` OQ14 (= HANDOFF §7 R-QC-1) | save files | A 1.14d expansion character that completed every Normal quest: save right after the last one; play one more game and save again | `d2s-tool dump` both quest sections; per slot the set bits; bits 13 / 14 gone in the second |
| R-PQ-6 | `quests-act1-rest.md` OQ3 (= HANDOFF §5 S9-A1 (12)–(13)) | packets + RNG + tick | (a) Tristram with the Cairn stone done: operate the gibbet; (b) Forgotten Tower 5: kill the Countess with at least one chest in her room | (a) gibbet mode 1, slot 4 = `0x2002`, 0x28 at once, 17 frames later Cain (class 146) at gibbet + (3, 3) and `5d 04 00 06 0000`; (b) one monster 326 mode 12 at the death spot, one missile 332 per listed chest |
| R-PQ-7 | `quests-act1-rest.md` OQ5 (Catacombs part = HANDOFF §5 S9-A2) | packets | (a) After killing Andariel (state 4), leave and re-enter Catacombs level 1; (b) new character, Den of Evil not needed: rescue nothing, complete A1Q2 (Blood Raven) with no hireling, talk to Kashya | (a) no 0x5D on entry, state kept; (b) order: 0x28, the hireling's 0x50 and creation messages, then 0x27, 0x29 (`quests-act1-rest.md` §8 item 8) |
| R-PQ-8 | `quests-act2.md` OQ10 (also OQ2, OQ3 confirmations; = HANDOFF §5 S9-A3 Act II) | packets + RNG | Expansion character entering Act II on Normal with no Act II quest done: talk to Jerhyn, Drognan, Atma; clear Radament; open the scroll chest and the cube chest; enter the Claw Viper Temple (Tainted Sun start), destroy the altar; read Horazon's journal; kill the Summoner; insert the staff; kill Duriel; talk to Tyrael, take his portal; talk to Meshif | chat-end statuses; kill timers; the gate's mode 1 / 2 set and end-animation frame per chest (`quests-act2.md` §1.3); `53 05000000 00000000 01` at the start and `53 02000000 00000000 00` at the altar; Tyrael's portal; Meshif's completion order |
| R-PQ-9 | `quests-act2-2.md` OQ1, `quests-act2.md` OQ31 (= HANDOFF §7 PC 2 list, orifice line) | packets | Act II, Horadric Staff assembled, in the true tomb's orifice chamber: operate the orifice, cancel; operate, insert a wrong cursor item; operate, insert the staff | the 7 bytes of each S→C 0x58, byte 6 for results 0, 1, 4 |
