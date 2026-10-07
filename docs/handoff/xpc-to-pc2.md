# Cross-PC requests: PC 1 → PC 2

PC 2 owns objects, hirelings, all quests (Acts I–V), UI except `ui/text.md`,
audio, `formats/wav.md`, `formats/d2s.md`, `items/*` and
`world/{vendors,cube,npc,waypoints}.md` with their tables. PC 1 owns the
rest (see the PC 2 orchestrator prompt). When a PC 1 answer needs a change
in a PC 2 file, PC 1 appends a line here instead of editing it: file, rule
or question id, the 1.14d fact with its address, the change needed. PC 2
handles each line and strikes it with the commit that did it. The reverse
direction is `xpc-to-pc1.md`.

## Open

- `items/inventory.md` (new rule; asked by `skills/levels.md` OQ7): specify `0x00623990(unit, 0)`, the weapon a skill uses (skill kind `0x00644140` +0x168 dispatch via table `0x00623AFC`, hands 4 / 5 through `0x0063C050`, `0x00629BB0(item, 45)`, `0x0062A4E0`); `levels.md` §3.3 `SrcDam` reads it.
- `world/cube.md` §1: says `server-messages.tsv` marks 0x77 `out`; since PC 1 commit e2fa8c2 it is `sim` (single player sends it too; `sim/intents-events.md` §4 rule 4). Update the sentence.
- `ui/*` (owner of the UI consumer; `sim/intents-events.md` OQ18): the S→C ids below now have 1.14d senders and layouts (`server-messages.tsv`, `sim/intents-events.md` §3.5) but no client owner; each 1.14d general handler copies the message and calls one UI function: 0x26 `0x0045DFC0` → `0x0049F490` (chat / overhead text), 0x27 `0x0045E0A0` → `0x004A1600` (NPC text list; its type-1, kind-3, count-1 case also replaces the monster's overhead record, unit +0xA4, a model write), 0x4E `0x0045E3D0` → `0x004B3240` (hire offer), 0x50 `0x0045E370` → `0x004B9210`, 0x58 `0x0045E490` → `0x004C0550`, 0x89 `0x0045EA30` → `0x004B9330`, 0x8A `0x0045EA40` → `0x004B3380`, 0x91 `0x0045E580` → `0x004B3510` (0x78 `0x0045E810` → `0x004B9010` is trade, out of scope). Needed: a UI spec rule per entry point (what it shows, which UI state it sets, any model write), so `client/msg-ui.md` can take the dispatch and add `client/bridge.md` §10 output variants.
- `world/quests.md` §6.7: "u8 (D2MOO: act)" at 0x91 @1 is confirmed 1.14d: `0x00545100` stores its act argument (BL, `0x00545176`) and switches on it (table `0x0054519C`, acts 0–4). Drop the D2MOO hedge.
- Done (PC 1 area 5, commit 4ebe8d2): `xpc-to-pc1.md` lines on `drlg/rooms.md` §8 rule 1 (0x400000 setter owned there), `formats/mpq.md` Observations (table 8 re-measured: 350,543 sectors, no other table; escape share cited from your check), `client/audio.md` §A3 (end-tick model, conformance waits for OQ12) and §A4 (`GainCurve(v, pan, occ)`; new OQ4 on the f32 occlusion states), `sim/rng.md` OQ2 (answered from `0x0056A1FC`–`0x0056A217`). Strike them in your copy.
