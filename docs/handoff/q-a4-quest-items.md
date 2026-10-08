# q-a4-quest-items: the Hellforge flow on real items (`claude/q-a4-quest-items`)

Nothing here is verified against 1.14d (rule 10). Open point: REC-235 (`docs/HANDOFF.md` §7). No audio.

## Links connected

| Link | Where |
|---|---|
| Item, inventory, drop and stat tables for the synthetic game (`hfh `, `mss `, 21 gems / skulls, runes) | `d2-client` `app/synthetic_items.rs`; `GameParts::synthetic` in `single_player.rs` (`items`, `inventory`, `drops`, `stats`) |
| The player's inventory exists without charstats (the start items need vitals, the quests do not) | `WiredWorld::start_items` adds it first; the join log reports start-item faults only when vitals exist |
| `has_item`, `delete_item`, `wielded_weapon_code` of a quest call read the inventory model | `QuestInventory` (`items_of`, `cursor_of`, `weapon_in_use`, `delete`); `HostQuests` overrides; `EconomyQuests::find_item_in` |
| The object operate (Hellforge) had no inventory model | `QuestLoan<R, I>` carries it (`LoanedInventory`, `impl for InvParts` in `wired.rs`); `lend_quests` lends and returns it; the queued sends are drained (`WiredWorld::take_inventory_sent`) |
| Quest drops placed nothing (`NoSpot`) and left the item at (0, 0) | `StartSpot` (`chest_drop.rs`, shared with the monster drop); `HostQuests::drop_item_at` places the item in its static path; the staged place of an item reads the static path |
| Hellforge animation end (mode 1 to 2) | synthetic object row `Mode2` 1 |

Test: `crates/d2-client/tests/app_a4_endgame.rs` `the_hellforge_takes_the_soulstone_three_hammer_hits_and_drops_gems` (Cain 679, forge, Hephasto kill, pick up, wield, three hits, perfect gems on the ground). It failed at every link above before.

Changed expectation (named in the commit): `the_hellforge_answers_and_hephastos_death_reaches_chain_24` asserted the rest's "drop item" log line; the drop is real now, so it asserts the hammer is in the server's item store, not held.

## Changed expectations (the synthetic game's new content)

- `app_single_player`: `object_drops` is `Some` (drop tables exist).
- `app_amazon`, `app_skill_gaps`: the tests set the weapon copy by hand; they clear the inventory model so its sync does not replace it.
- `app_mercs_acts`: the resurrect costs gold now (a stat table exists); the test gives the player gold.

## PROVISIONAL (REC-235)

See the REC text: weapon in use fallback (+0x1C unset), start-spot quest drops (identified), delete without a separate equipped branch, synthetic tables.

## What is left

- Ground items reach the client only through the update pass (q-monster-drops); the test reads the server's store.
- Chest drops (`object_treasure`) still use `NoSpot`; `walk` lends the quests without draining the inventory sends.
- The test gives the player strength / dexterity / level by hand (the synthetic player has no charstats).

## The user's local check

```
cargo nextest run -p d2-client --test app_a4_endgame
```
With game files nothing changes (live tables); the synthetic fills are only used without `D2_GAME_DIR`.
