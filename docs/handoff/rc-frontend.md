# rc-frontend hand-back (REC-1905..1909)

Checks: tools/frontend-sbs, 15 screens, 1.14d (Wine) vs d2rs. EQUAL before 0, after 0
(no pixel-exact screen: snow/fire animation is wall-clock driven, scenario-diff r4 needs
tick-anchored input). Per-screen first difference before -> after:

| Screen | Before | After |
|---|---|---|
| main | gateway label blank | label "GATEWAY: EUROPE" drawn; rest = snow noise |
| charselect | dolls missing | dolls still missing (REC-1907, open) |
| charselect-back (Esc from create) | blue palette | sky palette, no cast |
| create (7 classes) | EXPANSION CHARACTER label missing | label present at 1.14d position |
| credits, cinematics | brightness | mean colour equal (diff mean -0.01..-0.26); snow noise only |

## Changed
- main_menu.rs: gateway button gets string 11049 + name (default gateway list, closest zone to bias 0 = Europe; PROVISIONAL REC-1905); mod.rs/glyphs.rs: a label with string id and literal text substitutes "%s".
- char_select.rs: re-entry loads the sky palette (0x0043AE30 calls loader 0x0042F2E0 with the sky paths); create's fechar is now confirmed from 0x00435580 (REC-1906/1908).
- Brightness (REC-1909): no offset reproduces on current staging (mean RGB within 0.3, static-block medians identical); the earlier gap was already fixed by q-fix-ui-blend.
- Spec frontend-menus.md F1.6 r0 and button 18; ledger rc-frontend.tsv (all still DIVERGED).

## Open
- REC-1907 paper dolls (L): read 0x005066C0 + sprite object draw (0x00503640, 0x00504D60), token table reader, DCC compose on CPU front-end, idle animation timing. Queued in pc1-data.md.
- Gateway name depends on the Windows time zone/registry; d2rs uses UTC.
- Pages (pair-*.png, Blizzard art) in /home/user/sbs-a, not committed.
