# q-a2-tyrael: Duriel's fight, Tyrael's portal and the road east (Act II close)

Branch `claude/q-a2-tyrael`. Nothing is verified against 1.14d (rule 10). Open point: REC-174 (`docs/HANDOFF.md` §7). (The task text named REC-174; `build-loop.md` says to take the highest REC in staging plus one, which was REC-174.)

## Links connected

| Link | Before | Now |
|---|---|---|
| Monster AI after an act change | the player's unit record act stayed at the old act, so `main_search` ("same act", `ai.md` §5.2 step 5.1) never qualified him: no monster in Act II+ attacked a player who had travelled | `wiring/path/act_change.rs` sets the record act |
| Tyrael's portal (msg 302) | host `create_portal` returned false (rest stub): state stayed 3 | `View::create_quest_portal` (`wiring/action/town_portal.rs`), called from `HostQuests::create_portal` |
| Duriel's fight | test spawned and killed him | Duriel AI 44 (existing `bodies2::duriel`) runs; he hits the player, then the kill parse moves chain 13 to state 3 |
| Jerhyn 442, Meshif 450, Meshif's travel row | quest code existed | driven end to end: state 5, then C→S 0x38 to Act III's town (75, REC-230's Act III world) |

Test: `crates/d2-client/tests/app_a2_tyrael.rs`. Failed before the first two rows (no attack; state stayed 3).

## PROVISIONAL (REC-174)

See HANDOFF §7. Quest portal without a partner; the Lair's Duriel and Tyrael are spawned by the test, not by the world; Duriel fights with plain A1/A2.

## What's left

- Tyrael's chamber door (object 153) and Tyrael placed in the Lair; Duriel placed by the Lair's population.
- Walking through Tyrael's portal (arrival spot, `portal_destination`).
- Charge/Jab skills (no skill rows in the synthetic game).

## The user's local check (game files)

```
cargo test -p d2-client --test app_a2_tyrael
cargo run -p d2-client --release -- play --new sorceress Test
```
In `play`: with a save that has killed Duriel, talk to Tyrael: a portal appears next to you; Jerhyn then Meshif ("sail" row) lead to Kurast Docks. Duriel in the Lair should attack in melee. Record console lines mentioning `portal` and any `rejected`.
