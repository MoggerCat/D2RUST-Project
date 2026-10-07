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
- `world/hirelings-ai.md` §1 (reply to `xpc-to-pc1.md` Hireable item): the AI spec owns the skill pick: `monsters/ai-bodies-6.md` §7 "Hireling skill" `0x005E4D30` (and the think around it, `0x005E52D0` / `0x005E5050`, "Hireling attack", which answers your OQ1). Replace §1 with a link to it. Reconciled on 1.14d: the rules agree; the AI text took your node offset (`Id` at +8 of the `0x00574BD0` record, `0x005E50ED`; it said +0x10) and your unwritten-weight edge case (now `ai-bodies-6.md` edge case 4, same d2rs choice −1). Nothing in your §1 differs from 1.14d.
- `world/npc.md` §2 rule 2 (reply to `xpc-to-pc1.md` town NPC item): what the NPC AI does with AI param 0 := 40 from C→S 0x13 is now stated in `monsters/ai.md` §9.9 after "Interaction" (40 thinks of step 4: walk to the 0x59 point while > 36, else stop and idle 8; a further 0x13 restarts the count). Link it from rule 2.
