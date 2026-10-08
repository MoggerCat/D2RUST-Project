# q-smoke-quests: quest-line readiness smoke test

Branch `claude/q-smoke-quests`. Readiness check before the user's local testing. Nothing here is verified against 1.14d (rule 10). No REC id used (REC-283 stays free: no provisional point was added).

## What was built

`crates/d2-client/tests/smoke_quests.rs`: one rig (`Smoke`) over the real play path (bridge + predict link + in-process server thread + sim of the synthetic game, original UI with the quest log, no Bevy window). It audits every frame:

- server: every drained C→S message must be `Dispatched(Done)` (or a system message); a `Refused`/`Invalid`/`Malformed`/gate drop fails the step;
- client: the bridge `ReceiveLog` must have no `rejected`, `dropped`, `unowned` or `discarded` entry;
- a panic anywhere (client systems included) fails the test.

NPCs are opened with the bridge's interact (C→S 0x13), the menu rows (Talk, Cancel) are clicked in the UI, the quest log is opened with Q and read per tab.

| Test | Walks |
|---|---|
| `act1_den_and_burial_grounds` | Akara ×2 → Den started (server state 2, client record 0x28 bit, log row in progress) → Blood Moor → Den tile (state 3) → Den monster dies (state 4, reward pending) → town, Akara 76 (+1 skill point, state 5, log done) → Kashya 81 (A1Q2 started, log) → Cold Plains → Burial Grounds tile (state 3) → Blood Raven (placed by the world) killed (state 4) → town, timer → Kashya 92 (state 5) |
| `act1_tower_andariel_and_the_way_east` | Black Marsh → Moldy Tome (state 2) → Tower + 5 cellars by tiles (state 3, log in progress) → Countess (state 5) → Catacombs 4 (A1Q6 state 3) → Andariel (credit) → town, Warriv 183 (reward granted) → C→S 0x38 travel → Lut Gholein, client act 1 |

Both pass, every audit clean.

## Breaks found and fixed

| # | Break | Cause | Fix (commit) |
|---|---|---|---|
| 1 | Client panic `index out of bounds: len 128, index usize::MAX` in `d2_sim::path::walk::geom::direction_vector`, from `UnitArt::observe_facing` after the Tower-cellar walk | `bridge::predict::facing` (d2rs-own) fed far positions (a unit's position from before a level change) to the original's direction routine, whose 32-bit `127 × l` wraps past ~258 sub-tiles | far deltas halved until in range, direction kept; unit test `facing_a_far_point_keeps_the_direction` (`bb8543d8`) |

## Still broken / missing (named follow-ups; not invented here)

1. **Host-placed units are lost when their DRLG room is freed.** `UnitLists::free_room` unlinks the room's units (`rooms.md` §8.2 TODO), and the synthetic world places its town NPCs (`build_with_town`) and the Moldy Tome once at build. After ~300 frames away (e.g. a stay in the Burial Grounds), the Rogue Encampment's NPCs are gone from the client on return (the server units remain, room `None`); the Moldy Tome has room `None` after ~850 frames even if the Black Marsh was never visited. In `play` on synthetic data the Tower quest is unreachable after ~30 s, and quest rewards after a long trip. With game files presets re-create objects, but host-placed NPCs need checking. Owner: world / town (repro: return to town after `step(340)` in another level; `smoke_quests.rs` orders its steps around it).
2. **Warriv's "Go East" row** (and Meshif's "Sail East", Jerhyn, etc.) is a runtime menu insert the spec leaves open (`ui/panels.md` open question 8, `panels-2.md` §14.8): the client menu of Warriv (155) shows only Talk/Cancel after Andariel, so the act cannot be changed from the UI. The test sends the row's intent (C→S 0x38 action 0) itself. Needs a spec answer (RE) or an M22 provisional insert.
3. **Warriv is not in the synthetic Rogue Encampment** (`town_npcs::ACT1` has Akara, Kashya, Gheed, Charsi; Warriv is a class only): the test places him by hand, as `app_andariel.rs`. His talk sends messages 0, 141, then 183 over three talks; the leading C→S 0x31 message 0 was not investigated.
4. **Synthetic client rows**: only Blood Raven, Izual, Duriel and the NPCs/mercs have client `monstats` rows; the client ignores the 0xAC of any other monster (`msg-units.md` §1.2 rule 2) and then drops its 0x6D (dropped S→C 0x6D seen for a Den monster). The test adds rows for the classes it places (Den monster 1, Countess 45, Andariel 156).
5. **Synthetic `monstats.killable`** is set only for Blood Raven, Izual, Duriel and the Act IV bosses, so the kill reaction refuses the Den's monsters, the Countess, Andariel (and Radament, the Summoner, Mephisto, Shenk, Baal …). The test runs the kill's quest step (`KillStep::QuestKill`) for those, as the per-quest tests do.
6. **Den of Evil content**: no Den population; `AppRest::den_region` answers `(0, 0, 0, 0)`, so the first chain-1 kill clears the Den. The test places one monster with the chain-1 link monster init would give.
7. **Not walked in this session (time)**: Cain / Tristram (no Stony Field stones, tree, gibbet in the synthetic world: `q-a1-cain.md`), Act II (Radament, staff, Summoner, Duriel/Tyrael), Act III (Durance, Mephisto), Act IV (Izual, Hellforge, Diablo), Act V (Shenk, Anya, Ancients, Baal). The per-quest app tests cover pieces of these (`app_a2_*`, `app_a3_quests`, `app_a4*`, `app_a5_ancients`) but without the quest-log / dropped / unowned / refused audit; the `Smoke` rig is ready to take them (`put_in`, `take`, `talk`, `spawn`, `kill`, `quest_kill`, `log`, `check`). Known content gaps from their handoffs: no `hst `/`ass `/`mss `/`hfh ` item tables, no Act III–V boss populations, Act V hooks with no fight.

## The user's local check

```
cargo nextest run -p d2-client --test smoke_quests
cargo test -p d2-client --lib facing_
```
Both smoke tests pass (about 35 s each in debug). With game files: `cargo run -p d2-client --release -- play --new sorceress Test`, walk the Den of Evil and back, go to the Burial Grounds, stay a minute, come back by waypoint: Kashya and Akara must still be there (follow-up 1); after Andariel, Warriv's menu should offer travel (follow-up 2).
