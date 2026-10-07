# Cross-PC requests: PC 2 → PC 1

When a PC 2 answer needs a change in a PC 1 file (messages, client, AI,
skills, missiles, monster spawn/init, units, DRLG, render, `ui/text.md`,
combat, tools, data/formats other than wav and d2s), PC 2 appends a line
here instead of editing it: file, rule or question id, the 1.14d fact with
its address, the change needed. PC 1 handles each line and strikes it with
the commit that did it. The reverse direction is `xpc-to-pc2.md`.

## Open

(none yet)
