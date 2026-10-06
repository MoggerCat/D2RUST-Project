# Handoff: interaction wiring (`d2_sim::wiring::interaction`)

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Branch `claude/wire-interaction`, from
`claude/bold-ptolemy-jvyvxy` at `1470723`. Repo only. Scope of every
claim: this branch, synthetic tables, fixed seeds, no game files (M09).
For the coordinator to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (not
edited here).

## 1. State

**Wired, unverified** (M02): every module behind these adapters is a
draft-spec implementation; the adapters add no rule of their own.

- New module `crates/d2-sim/src/wiring/interaction/`, one file per seam
  pair, plus one line `pub mod interaction;` in `wiring/mod.rs` (doc
  comment untouched).
- **One module change** (§5): `world::vendors::NpcLink::make_hire_list`
  takes the NPC-control seed.
- Tests: 11 integration tests in `wiring/interaction/tests/` run the real
  modules together; the only fakes are the seams this wiring leaves open
  (one `Rest` fake for NPC / vendor / quest rests, `Open` for the action
  wiring's `Pending` + `UseRest`, `Notes` for `VitalsRest`).
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
  -p conformance --all-targets -- -D warnings`, `cargo test -p d2-sim -p
  conformance` (d2-sim 839 pass, 5 ignored; conformance `tick_replay` 7,
  `tick_traces` 2, `rng_traces` 2), `cargo run -p depcheck`, `python3
  tools/spec_index.py --check`, `python3 tools/methods.py check`,
  `python3 tools/coverage.py --check` (2429 claims, 0 errors). `cargo
  check -p d2-server` also builds.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `wiring/interaction/mod.rs` | `InteractionState` (one `VendorRecord` per NPC record, the NPCs' `InteractionList`s, errors), `Desk` (borrows `wiring::economy::Economy`, `QuestControl`, `VendorTables`, the state, the rest, host ms), `PlayerQuestsRef`, `InteractionError` | `npc.md` §1.1, `vendors.md` §1 |
| `wiring/interaction/npc_world.rs` | `NpcWorld` for `Desk`; `NpcRest` | `npc.md` §2–§8 |
| `wiring/interaction/npc_vendors.rs` | `NpcVendors` for `Desk`; `VendorDesk` with `NpcLink`; entry points `buy` (0x32), `sell` (0x33), `repair` (0x35), `identify_gamble` (0x37), `level_changed`, `client_left` | `vendors.md` §4–§9, `npc.md` §7.1 |
| `wiring/interaction/vendor_world.rs` | `VendorWorld` for `VendorDesk`; `VendorRest` | `vendors.md` §3, §6, §7, §9.2 |
| `wiring/interaction/skill_use.rs` | `UseView` (wraps `wiring::action::combat::CombatView`): `SkillUnits`, `ManaUnits`, `SkillFunctions`, `UseMissiles`, `UseWorld`; `ActionSim::skill_use`; `UseRest` | `use.md` §1–§7, `missiles.md` §R2 |
| `wiring/interaction/vitals.rs` | `VitalsView`: `VitalsUnits`; `kill_experience`; `VitalsRest` | `vitals.md` §1–§4 |
| `wiring/interaction/tests/` | `mod.rs` (synthetic stat / item / vendor / monstats / hireling tables, `World`, `Rest`); `npc.rs`, `vendors.rs`, `skill_use.rs` (own copy of the action DRLG fixture, which is private to that module), `vitals.rs` | |

## 3. What is wired

| Seam (module) | Provider | Integration test |
|---|---|---|
| `npc::NpcVendors::open_trade` | `vendors::store::open` on the class's `VendorRecord` (lent out of the state), NPC-control seed lent to the store code and back to `NpcControl::make_hire_list` | `vendors::npc_menu_opens_a_real_store_and_buy_pays_real_gold` (0x13 → 0x38 action 1 → real store: permanent cap made by `Economy::create_item`, game seed stepped exactly twice, NPC seed untouched, page / flags / unit +0xC8 marked); `vendors::trade_open_lends_the_npc_seed_to_the_hire_list` (store range draw then the hire list on the continued seed; a list drawn from the pre-store seed differs) |
| `NpcVendors::drop_gamble_list`, `pay`, `repair` | `gamble::drop_list`, `trade::pay`, `trade::repair_item` | `pay` in identify / hire / resurrect / buy tests |
| `vendors::NpcLink` | unit lists (monster hash + interaction list), unit records, `Rest` interact unit, `NpcRecord::hire_made`, `NpcControl::make_hire_list` | same tests |
| `vendors::VendorWorld` | `GameFields` (difficulty, expansion, game type), unit records / lists (GUIDs, mode, +0xC8), `StatLists`, the player's quest slot words, `quests::npc_gossip` (town entered, `quests.md` §6.7), `Economy::create_item` / `free_item`, `ItemStore`, `PriceItem` from item data + stats | buy: gold 5000 → 5000 − cost on the real stats, 0x2A kind 4 with the item GUID, last bought, mode 4, flag 0x2; a second buy without gold → code 12, nothing paid |
| `npc::NpcWorld` units | GUIDs, monster lookup, class, mode, +0xC4, act test, AI think reschedule on the timer queue (with the state-54 check) | `npc::talk_starts_on_real_units_timers_and_quests` (think at frame + 1; node, interact unit; 0x27, 0x29, 0x28 in order, 0x29 = the real game flags) |
| `NpcWorld` stats / states | `StatLists` (totals, base, set, maxima, `has_state`, `curable` = state flag bit 12 of `fields.tsv`, state lists) | `npc::chat_open_heals_on_real_stats_and_state_lists` (life / mana to max with SetStat, stamina already full, poison and a curable list removed, a plain one kept, sound 10) |
| `NpcWorld` quests | `QuestControl::npc_activate`, `send_game_flags`, `npc_deactivate`, `act_completion`; `quests::send_player_flags`, `act1::{respec_offer, respec_done, imbue_granted}` through `wiring::economy::EconomyQuests` | talk test |
| `NpcWorld` items | `ItemStore` (page, flags), `items::create::{max_sockets, socket_count}`, unit seed | — (Charsi / Larzuk services need `NpcRest` item calls) |
| identify, hire, resurrect | the above | `npc::cain_identify_pays_from_real_gold` (2 items → 200 gold, code 3); `npc::hire_at_asheara_pays_real_gold_for_the_offer` (price = `hire_init` of the slot seed, slot hired, code 5 with the merc GUID); `npc::resurrect_revives_a_real_mercenary` (cost 750 for level 10, flag 0x10000 cleared, life = max, 0x9B, code 5) |
| `skills::use_` seams | `CombatView` (`SkillUnits`), frame, unit lists (`find_unit`), act test, plain mode set `0x00553570`, +0xC4, alive, ENDANIM expire from the timer queue, hostility / melee range / position (the action wiring's `Pending`), room kind, `schedule` / `delete_timers`, delay list (flags 2, state 121, callback id `0x0056E900`), aura state free, stat set | `skill_use::skill_do_fires_a_real_missile_that_hits_a_real_monster` (`do_skill` → flag 0x40, 2 mana charged, a real missile (skill / level stored) flies 3 frames and hits: to-hit on the caster seed, 2560 off the monster's life, missile removed); `skill_use::cooldown_list_and_its_expiry_timer_are_real` (§6: list, state, type-12 timers, expiry moved, list gone at its frame) |
| `UseMissiles::create_skill_missile` | `missiles::create_missile` on the real store (owner / origin = caster, class, skill, level) | same test |
| `combat::vitals::VitalsUnits` | unit records, `StatLists` (base / unit getters, set, add, maxima) | `vitals::kill_grants_experience_and_a_level_up_on_real_stats` (Sorceress created per the spec vector; a level-1 monster with 600 experience → level 2, +256 life / +512 mana / +256 stamina, 5 stat points, 1 skill point, notify + event 12); `vitals::stat_points_and_regeneration_share_the_real_lists` (0x3A vitality ×2 and energy ×1; then the unit dispatch's player regeneration fills mana by max / 100 on the same lists) |

## 4. Design points (no module edits beyond §5)

1. **One desk, two views.** `Desk` implements `NpcWorld + NpcVendors`;
   the vendors' world is `VendorDesk { desk, ctl, npc }` because
   `NpcLink` needs the NPC control block, which the NPC handlers own
   (`&mut self`). `VendorDesk::ctl` is `None` for `pay` / `repair` from
   an NPC service (no hire-list call happens there; one would be logged).
2. **Records lent out.** A vendor function holds `&mut VendorRecord`
   while it calls the world: the record is `mem::take`n out of
   `InteractionState::vendors` for the call and put back (the action
   wiring lends its stores the same way). A message whose NPC GUID has
   no unit runs the handler on an empty record (the module refuses
   before reading it).
3. **The skill list's one owner.** `UseRest` is implemented by the action
   wiring's `Pending` value, so the skill list and used skill combat
   reads (`Pending::skill_list`, `used_skill`) and the ones skill use
   writes (`UseRest::set_used_skill`) are the same.
4. **Quests share the economy's rest.** `Desk`'s rest is one value that
   is `NpcRest + VendorRest + QuestRest + PlayerQuestsRef`; NPC messages
   and sounds go through `QuestRest::send` / `attach_sound`.
5. **Regeneration** is the unit dispatch's (`stat-lists.md` §10.1,
   already on the real lists): vitals add no tick hook of their own; the
   test shows a level-up's / stat point's new maximum is what
   regeneration reads.

## 5. Change outside `wiring/` (smallest, provably needed)

`world::vendors::NpcLink::make_hire_list(&mut self, class)` →
`make_hire_list(&mut self, class, seed: &mut Seed)`; `store::open` passes
`c.seed`; the vendors test fake updated. Why: `vendors.md` §4 rule 2 runs
store generation (§3, draws on the NPC-control seed) and then, in the
same open, the hire list (`npc.md` §7.1 steps 3–4, draws on the same
seed). `store::open` holds `StoreCtx::seed` (`&mut`) across the
`w.make_hire_list` call, so no provider could reach the seed: the hire
list would draw from a stale copy and its draws would be lost. Test:
`trade_open_lends_the_npc_seed_to_the_hire_list`.

## 6. Remaining seams and why

| Seam | Why not wired |
|---|---|
| `NpcRest`: distance, axis check, unit check, paths, approach | movement / path spec, `intents-events.md` §2.4 positions |
| `NpcRest`: interact unit, busy, pets, player name, start allowed, Tristram Cain | player data / player spec; `npc.md` OQ1 |
| `NpcRest`: `npc_ai_param` (`0x0058EC00`) | `npc.md` OQ2 (monster spec) |
| `NpcRest`: `stat_sent`, `encode_text_list` (`0x00661480`), respec sound | transport; `server-messages.tsv` 0x27 `partial` |
| `NpcRest`: `reset_stats`, `reset_skills` | address conflict, open question I3 |
| `NpcRest`: inventory, identify (`0x00562590`), cursor, item facts, put back, duplicate, imbue creation, refresh, personal name, place / drop | inventory spec; not in the items specs |
| `NpcRest`: act change, waypoint activation, Act V hooks | `waypoints.md` OQ1; Act V quests not specified |
| `NpcRest`: mercenary spawn / init / revive, mode set of a revived merc | mercenary spec not written |
| `VendorRest`: players in level, player level, gold / stash caps, drop gold, last bought, cursor | DRLG rooms / player data |
| `VendorRest`: copy (`0x0055A2A0`), filled sockets, socketed items, (B) bonuses (`0x00625560`, `vendors.md` OQ1), recharge, broken repair | not in the items specs (economy handoff §5 lists the same) |
| `VendorRest`: NPC and player inventories, 0x3E, 0x2A transport | inventory spec, `d2-server` |
| `UseRest`: skill list and its fields, use state (`use.md` §2 order), dec quantity, charges, blood mana | units / skills (no skill list in d2-sim); `use.md` OQs, `levels.md` OQ8 |
| `UseRest`: player data, reach, owner, pet / ally, equipment, state mask, start mode / run to / target / event arg / path / target position / line test | player, path, items, units specs |
| `UseRest::skill_missile_fill` | `0x0056ECB0` / `0x0056EE90` record fill not specified (I8) |
| `UseRest::set_aura_state`, `srvst` / `srvdo` | aura list contents (`use.md` §7); per-skill bodies (`use.md` OQ10) |
| `VitalsRest`: refresh `0x0064C040`, level-up notifications, event 12 | not specified / transport / event registry |
| Tick events 5, 8, 9, 14 → skill use (`periodic_event`, `item_aura_event`, `active_state_event`) | the routing lives in the action wiring's `impl UnitHooks for ActionHooks` (keeps the defaults); not this session's file. Proposal: route `active_state`, `periodic_skills`, `apply_item_aura` there through `UseView` (needs `X: UseRest`) |
| The kill → `kill_experience` | the kill (`damage.md` §7.2) has no provider (action `Pending::reaction`); `vitals.md` OQ2 |
| `QuestRest::mercenary_reward` → `NpcControl::quest_mercenary` | the quests' rest has no NPC control (the desk holds it); needs a quest-rest wrapper over `Desk + NpcControl` |

## 7. Open questions (each has a `TODO` at its site)

- **I1** (`npc_world.rs` `unit_by_guid`, `npc.md` §3): "the unit with
  GUID" names no type; read as the monster hash (a GUID of another type
  answers 1 instead of 3 if the original searches all types).
- **I2** (`reschedule_ai_think`, `npc.md` §2 rule 2): the think's
  arguments are not written; 0, 0.
- **I3** (`reset_stats`, `npc.md` §8.2 vs `vitals.md` §2.1): npc.md says
  `0x00570360` resets stats and `0x00570C80` skills; vitals.md says
  `0x00570C80` is the stat reset. Not wired until a spec session
  settles the addresses.
- **I4** (`npc_vendors.rs` `repair`, `npc.md` §8.1): the service passes
  only the item; the player of `0x005761C0(item, player)` read as none.
- **I5** (`vendor_world.rs` `create_item`, `vendors.md` §3.1 rule 2): the
  allocation flags of `0x00559CE0` are not written; 1 (as drops and the
  cube), so the store item is in the unit lists the 0x32 lookup reads.
  The request unit is the trade open's NPC.
- **I6** (`set_item_mode`, `vendors.md` §7): the mode set the vendor code
  calls is not named; unit +0x10 is written.
- **I7** (`price_item`, `vendors.md` §9.2 Inputs): stats 70, 152, 72, 73,
  252–254 read as unit totals, 31 as base; entries of 107 / 204 from the
  full array of an extended list, else the base array.
- **I8** (`skill_use.rs` `create_skill_missile`, `use.md` §5.4 step 7):
  the skill missile helpers' record fill (flags, target, `aim`
  position) is not specified; owner, origin, class, skill and level are
  set, the rest comes from `UseRest::skill_missile_fill`.
- **I9** (`create_delay_list`, `use.md` §6, `stat-lists.md` §8.1): the
  attach `reset` argument is not stated; 1 (as the action wiring's state
  lists).
- **I10** (`vitals.rs` `kill_experience`, `vitals.md` OQ2, `damage.md`
  OQ7): only §4.2 (on the defender's base stat 13 and both levels as unit
  totals) and §4.3's add are applied; `ExpRatio`, stat 85, hireling cap,
  pet credit and party share are not.
- **Observation for the vendors spec owner** (no TODO; the module's
  reading): `vendors.md` §3.1 rule 4 repairs a new store item before rule
  5 identifies it, and the repair (`0x005761C0`, §8.2) starts with
  "repairable" (§9.2 rule 0), which requires flag 0x10. So a store item
  keeps its created durability. If 1.14d store items are always at full
  durability, the repair's identified test or the step order needs a
  check (the queued store recording, `impl-vendors.md` check 2, shows
  it: compare 0x9C item durability).

## 8. Checks to queue (local, `docs/HANDOFF.md` §5)

1. With the talk / trade recording (`impl-npc.md` check 2,
   `impl-vendors.md` checks 2–3): replay 0x13 / 0x2F / 0x38 / 0x32 /
   0x33 through `Desk` and `VendorDesk` with recording-backed rests;
   compare message bytes (0x2A bytes 3–6 masked), the store item list
   and durabilities, gold after each transaction, and the NPC-control
   seed state (store draws then hire-list draws in one open).
2. With the `use.md` / `vitals.md` recordings (`impl-skilluse-vitals.md`
   check 1): replay a missile skill's do through `ActionSim::skill_use`
   and a level-up / stat spend through `VitalsView`; expect equal stats,
   the same draw count on the caster seed, and the type-12 timer frames.
3. Re-run `cargo test -p conformance --test tick_replay` after routing
   the skill-use tick events (§6) into the action wiring.
