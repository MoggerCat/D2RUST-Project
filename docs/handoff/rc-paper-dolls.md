# rc-paper-dolls hand-back (REC-2180..2184)

Check: `tools/frontend-sbs` charselect (1.14d under Wine vs d2rs, 3 unequipped saves) and a 12-capture
animation sweep (`/tmp` only, Blizzard art). Ledger EQUAL 894 -> 894 (no scenario check exists yet; the
new row `ui.frontend.charselect.dolls` is NO-CHECK: figure pixels measured equal by hand, see below).

## Result
- Before: no dolls. After: every visible slot draws its figure; at the matching animation frame 100 % of the
  figure pixels of slot 0 equal 1.14d (max diff 0); slots 1 and 2 equal as well (residual = snow/background
  noise under the figure, not the doll). All 12 timed 1.14d captures (5.0 .. 9.4 s) match some d2rs frame
  at 100 %: frame advance is 1 per 40 ms tick (TN sorceress: 16 frames, rate 0x50, 3 frames / 0.4 s).

## Changed
- `specs/ui/frontend-menus.md` §F2.10 (new): build `0x005066C0`, weapon class `0x00504AF0`, files, colour maps
  `0x00505470`, animation `0x00503BA0`, placement, shadows.
- `ui/front_end/doll.rs`: pure doll logic (token/weapon class/paths/colour-map file/animation step) + tests.
- `DrawItem::Doll`; `CharSelect::overlay` emits one per slot (anchor column x + 30, row bottom - 13).
- `app/front_host.rs`: `FrontArt::with_dolls`, COF/DCC/colour-map loading, `draw_doll` (indexed frame).
- `ItemGfx.wclass2` (`2handedwclass`) in d2-formats + d2-server table builder.
- `main.rs menu_once`: gives the host the appearance tables.

## Open (PROVISIONAL)
- REC-2181 (S, needs PC1): equipped saves: slot fix-ups `0x00504D60` not implemented; static 1.00 table
  `0x0072E1E0` stands in as "no class"; front-end token table assumed equal to the in-game one. Queued in pc1-data.md.
- REC-2182 (S): phase = ticks since screen build; start instant not tick-anchored (needs scenario-diff input).
- REC-2183 (S): doll drawn at `anchor.y + y_min - 1` (measured, cause unread).
- REC-2184 (S): shadow pass `0x00503A50` not drawn (invisible on dark capture background).
- Dead hardcore (class' 8/9, `RH`, draw mode 1): implemented from the code, no capture.

## Notes for others
- `tools/coverage.py --check` reports one error in `crates/d2-sim/src/debug/state/tests.rs:409` (malformed rule
  `§2 \`own\``) that came with integ-r16; not mine.
- Disk: building d2-client debug + tests fills the 5 GB allowance; I removed `target/debug/incremental`.
