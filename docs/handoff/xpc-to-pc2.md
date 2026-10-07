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

(none yet)
