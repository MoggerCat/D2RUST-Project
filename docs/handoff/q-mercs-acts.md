# Q: act 2–5 mercenaries (`claude/q-mercs-acts`)

Nothing here is verified against 1.14d (rule 10); fills are `d2rs-own, unverified`. PROVISIONAL: REC-157 (`docs/HANDOFF.md` §7).

## Finding

Hire rows existed for Kashya, Greiz, Asheara and Qual-Kehk, and the hire list, price and 0x36 handler were in the sim. But the synthetic game had `hireling_tables: None`, so the hire stopped at `NoHirelingTables` (no unit), and the preview rest's `owner` always answered `None`, so a merc's death never reached the hireling list and no seller could resurrect it.

## Links connected

| Link | Where |
|---|---|
| Synthetic `hireling` rows from the hire rows (shared `Id`, class, act, seller, level, names) | new `app/merc_rows.rs`, installed in `GameParts::synthetic` |
| Client unit rows for merc classes 271 / 357 / 560 (a class without a row is ignored on S→C 0xAC) | `single_player.rs` `MERC_CLASSES`, `synthetic_unit_rows` |
| Owner stored by `set_owner` (kill → dead node → 0x9B / 0x7A; resurrect needs the dead node) | `app/rest.rs` `AppRest::owners` |
| Greiz / Asheara / Qual-Kehk rows priced 0 (no gold stat row in the synthetic game) | `town_npcs.rs`, `single_player.rs` |

## Tests

`tests/app_mercs_acts.rs`: for Greiz (act 2), Asheara (act 3) and Qual-Kehk (act 5, quest flag 36 set): talk, hire (C→S 0x36) → the merc unit is in the client model; kill it through the pet-death queue; resurrect (C→S 0x62) → result code 5. The resurrect fails (code 9) without the `owners` change. `app_single_player` now expects `hireling_tables` to be present in the synthetic game (it asserted `None`; the spec has no value for the preview's tables, so the expectation follows the new fill).

## Left

- Auras / skills / equipment: they come from the live `hireling.txt` rows (skill1.. columns) through `life::level`; the synthetic rows carry none, so nothing here shows them.
- `HirelingRest::warp_to` is still a logging stub (see `q-hire-follow.md`).
- The coordinator's REC id is 157; staging's highest was 155 at the last fetch.

## Your local check (Windows, 1.14d files)

```powershell
git fetch origin claude/q-mercs-acts; git checkout claude/q-mercs-acts
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new sorceress Test
```
Travel to Lut Gholein, Kurast Docks and Harrogath (Qual-Kehk needs Prison of Ice done). Hire from Greiz, Asheara and Qual-Kehk: the merc appears and follows you; check its aura / skill and gear. Let it die and use Resurrect on the seller: it returns for the listed price. Report the aura each class shows.
