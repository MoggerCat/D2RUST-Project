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
