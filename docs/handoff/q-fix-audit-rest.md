# q-fix-audit-rest: open seam / flow / proto / save / prov audit rows (2026-10-09)

Branch `claude/q-fix-audit-rest` (from staging-7). Status read from
`docs/HANDOFF.md`, `docs/handoff/q-fix-*.md`. Done rows are not listed.

## Open at start (one line each)

| Row | State at start |
|---|---|
| q-fix-seam-beltable | `fits_belt` is a fixed code list (inv_items.rs:164); scrolls never reach the belt |
| q-fix-seam-store-grid | shop repacks store items (shop_ui.rs page_items) instead of the stream's x, y |
| q-fix-seam-grid-facts | stack / book / cube / ctrl-sell facts hard-coded off; cube click mode 0 |
| q-fix-seam-click-sounds | ClickOut::Sound and interact outputs dropped |
| q-fix-seam-npc-dialog-capture | NPC menu level / unidentified count read at draw, not receive |
| q-fix-seam-shake-camera | pick cameras unshaken (latent: no shake starts) |
| q-fix-seam-room-order | code done, spec line + test missing |
| q-fix-seam-stamina-scale | needs spec decision (client/model.md OQ2) -> PC 1 |
| q-fix-seam-vitals-delta, q-fix-proto-vitals-dx-sign | needs the binary -> PC 1 |
| q-fix-proto-state-param-sign | needs the binary -> PC 1 |
| q-fix-proto-one-type | refactor across d2-proto + client (bytes identical today) |
| q-fix-proto-docs (rest) | spec-session items |
| q-fix-save-gaps | mouse skills fallback (1); (2)-(4) hireling / golem / status credits |
| q-fix-flow-join-load | loader 0x22/0x21/0x5E/0x28/0x29 and refused-load 0xB4 |
| q-fix-flow-client-order | C5 (Esc on dead player x3), C7 (doubled registrations) |
| q-fix-flow-act-change | steps 1, 6, 7, 10 left open |
| q-fix-prov-rec-ids | REC id clashes in HANDOFF |
| q-fix-prov-handoff-stale | SUPERSEDED marks on REC bullets |
| q-fix-prov-hireling-search | two d2rs-own hireling searches |
| q-fix-prov-frame-clock | 40 ms tick const repeated |

## Results

(filled in per row below)
