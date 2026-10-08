# q-a3-town: Act III town (Kurast Docks) in the play preview

Branch `claude/q-a3-town`. Nothing is verified against 1.14d (rule 10). Open point: REC-137 (`docs/HANDOFF.md` §7).

## Finding

The Act III town needed no new rules: the NPC classes (Ormus 255, Asheara 252, Hratli 253, Alkor 254, Meshif 264, Cain 246, Natalya 251), their menus (`specs/ui/npc-menus.tsv` records 16–30), the vendor tables (`vendors.rs`), hire list and Meshif's sail-west act travel (q-act-travel) are all in the sim. The preview's synthetic world only knew Akara and Warriv as interactive classes, so nothing of Act III could be driven headless. With game files the live town's NPCs come from the town presets and the same menu path as Akara.

## Links connected

| Link | Where |
|---|---|
| Act III town NPC set for the synthetic world | `app/town_npcs.rs` (`ACT1`, `ACT3`, `all_classes`) |
| Every town class is `npc` + `interact` in the synthetic `monstats` and the client's synthetic unit rows | `single_player.rs` (`synthetic_monstats`, `synthetic_unit_rows`) |
| Synthetic town takes a list of NPCs (class, x offset) | `single_player.rs` `build_with_town` / `start_with_town` (`build_with_objects` / `start_with_objects` pass `ACT1`, so every existing caller is unchanged) |
| Asheara's hire list: made-up `hireling` rows (seller 252, three difficulties) so `make_hire_list` finds a row instead of failing the interaction | `town_npcs::synthetic_hire_rows`, `GameParts::synthetic` |

Tests: `crates/d2-client/tests/app_play_act3_town.rs` — click each of Ormus, Asheara, Hratli, Alkor, Cain, Natalya and Meshif: C→S 0x13, S→C 0x27/0x29/0x28, and the menu rows of the class's `npc-menus.tsv` record (Talk / Trade / Hire / Gamble / Identify / sail west).

## PROVISIONAL (REC-137)

- The synthetic placement, the hireling rows (mercenary class 357, price, level, name ids) are made up.
- The town waypoint, vendor stock for Act III and Cain's identify are not exercised here (the synthetic game has no vendor tables; identify is `q-identify`'s).
- **Approach gap (found, not fixed):** when the player's walk ends 7–8 sub-tiles from an NPC, the server's `NpcWorld::approach` only logs in the preview rest (`app/rest.rs`), so the talk never starts and the player must click again from closer. The test therefore places one NPC beside the start per game. A real `approach` needs the host to walk the player to the NPC.

## Left

- Act III as a play world (Kurast Docks level, waypoint object, the preset NPC positions) is `q-act-worlds`.
- Vendor stock / gamble / repair panels for these NPCs: `q-vendor-items`, `q-gamble`.
- Meshif's "sail west" click is not driven end to end here (the act change is `q-act-travel`'s).

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Reach Act III (a save in Act III, or Meshif/Warriv travel). Click Ormus (Talk / Trade), Asheara (Talk / Hire / Trade), Hratli (Talk / Trade), Alkor (Talk / Trade / Gamble), Cain (Talk / Identify), Meshif (Talk / Sail West). Note any NPC with no menu or missing from the town.
