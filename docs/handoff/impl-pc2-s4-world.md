# Cloud impl-pc2-s4-world (2026-10-07): PC 2 session 4 answers for objects, hirelings, quests

Branch `claude/impl-pc2-s4-world`, base `claude/specs-staging-6` @ 9f8b16c.
Inputs: `docs/handoff/pc2-session4.md`, `pc2-s4-objects-hirelings.md`,
`pc2-s4-objects-client.md`, `pc2-s4-quests.md`, `pc2-for-cloud.md`.
Implementation session: specs/docs/crates only.

## 1. Landed (commits in order)

| Commit | What | Spec |
|---|---|---|
| 21ff9594, 4ffc761a | Hirelings part 2: owner death kills the hireling (0x7A, room notice, node dead, death mode, 0x9B) wired from the player corpse through `ActionHooks::owner_deaths` → `WiredWorld::pet_deaths`; restore (`Loader` Current / Old / OldV47, Id 0xFFFF only from 0x47 saves, per-loader row checks, `restore_tail` order, `0x005738D0` = cancel timer types 2, 3); `join_follow`; item swap cancels type-9 timers (merc, then player), failed duplicate loses the item; range pets (dx²+dy² > 1600), free branch frees units, list head | `hirelings-2.md` §15–§19 |
| 40178db0 | **Object population** `0x00552610` (`world::objects::populate`): pre-check and region counters, theme gate (draws, never runs a theme), 8 `ObjGrp` slots, `PopulateFn` 1–9, fits A/B/C, random / oriented / spread spots, shrine / well caps and spacing; wired from `WorldHost::populate_objects` (DRLG room facts, room seed, room count, `0x0064D800`); `objgroup` in `ObjectTables`. Objects part-1 answers: same-mode set does no setup (§4 r5), trap monster id `0x005474C0` in d2-sim (region cache + monster region walk), fire objects mode 1 with the x room bound and fatal 160, barrel returns 0, exploding barrel metrics, locked chest / door with no operator fatal, assassin test by class id, shrine base-stat adds (0 writes nothing), storm flags 3 / no kill, shrine with no operator fatal, portal non-player fatal, well "used" = a write, refill mode set queues | `object-population.md`; `objects.md` §4, §8–§12; `objects-2.md` §22 r4, §24 |
| c7e81baa | **Objects part 2** (`world::objects::mech`, seam `MechWorld`): operates 13, 16–20, 26, 27, 29, 30, 32, 47, 48, 50, 51, 61; inits 8, 10, 13, 14, 22, 24, 26, 27, 28, 34, 51, 58; events 0, 3, 8, 9, 10; ENDANIM and the fire event write the mode field directly (`ObjectWorld::store_mode`). Routes follow `object-functions.tsv` (fixes `routes_match_function_table` / `route_check_catches_perturbations`, red on staging-6) | `objects-2.md` §16–§18 |
| 72a91b98 | **Portal travel** §12 rules 4–13 (owner / party gate, partner, leveldefs quest gate, destination, flags \|= 5, town quest hook, free point 0x1C09, placement, sound 8, walk, S→C 0x0D, removal / ENDANIM, state 102); `leveldefs` in `ObjectTables`; host facts via new `Pending::object_*` methods | `objects.md` §12 |
| 16ac0b74 | S→C 0x60 built in d2-sim (`portal_message`); `Pending::object_portal_message` removed; G1–G8 status in `xpc-to-pc2.md` | `objects.md` §14 |
| 60cc5d8b, 7e74334f | **Quests**: new `world/quests/helpers.rs` (free spot `0x00545340` on DRLG rooms, critical spawn, superunique at a point, quest missiles 541 / 625 / 368, end interaction + 0x62, game end / save pass as `QuestControl::host_requests`, TP close, item search `0x00558110`); `quests-act2-2.md` §5 (quest-chest gate, Tainted Sun on the environment record, remove unit for everyone, scroll text 0x27); stairs warp `0x0059D9D0`; contradicted readings fixed (Jerhyn first player, sanctuary stored value only, 0x5D status 0, Tyrael distance loop, Act V freed equality, §6.8 flags kept, §8.5 no room ends the callback); every mapped `TODO(quests…)` | `quests-helpers.md`, `quests-act2-2.md` §5, `pc2-s4-quests.md` table |

## 2. Changed test expectations

Objects (all because the behaviour is now specified):

| Test | Old → new | Spec |
|---|---|---|
| `objects::tests::anim_vector_and_sync` | set from mode 0 to 0 ran the setup → starts from mode 1; new check: same mode only writes | `objects.md` §4 r5 |
| `objects::tests::create_routes_inits_owned_elsewhere` | init 8 `NotCovered` → `Here` | `objects-2.md` §17 |
| `objects::tests::dispatch_table_rules` | operate 13 `NotCovered` → `Done(0)` | §16.1 |
| `objects::tests::end_anim_and_delayed_portal_events` | ENDANIM `Mode(O, 2, false)` → direct store; events 0/3/8/9/10 `NotCovered` → `Done` | §18, §18.6 |
| `objects::tests` route table check | quest owner must be `world/quests.md` → any `world/quests*.md` | `object-functions.tsv` |
| `objects::tests` update messages | `Portal(O)` → the 0x60 bytes | §14 |
| `chests::tests` (casket, barrel, trap arm, trap 8/9) | scripted trap id 77 / 44 / 235 → region list ids 170 / 274 / 0 (family bases), log "trapid UnitId" → "trapid level" | §8.3 trap monster id |
| `chests::tests` barrel, exploding barrel | return 1 → 0 | §8.2 return values |
| `chests::tests::trap_event_fire_objects` | allocation mode 0 → 1 | §8.3 fire objects |
| `shrines::tests` codes 1–5, storm, full sequence, dispatch | stat sets → base-stat adds (deltas); storm unit with 0 loss writes nothing; missile flags 0 → 3 | §9.2, §9.3 |
| `shrines::tests::operate_without_operator…` | effect skipped → `ShrineNoOperator` after rules 1–3 | §9.1 "No guards" |
| `misc::tests::well_charge_vector` | refill mode set `queue false` → `true` | `objects-2.md` §24 r7 |
| `misc::tests::portal_busy_and_owner` | monster / no operator `Done(0)` → `PortalOperator`; travel seam → real travel (`Done(0)`, placement) | §12 r5, r4–13 |
| `misc::tests::portal_hostile_delay_and_travel_seam` | `NotCovered` → the full rule 8–13 call sequence | §12 |

Hirelings and quests: see the tables in the two worker reports, reproduced
here in short. Hirelings: `follow_without_warp_or_range_frees_nodes` (log
`[]` → `["free 10"]`, §18 r2); `restore_plan_clamps_name_and_checks_act`
(plan gains class / id / seller / loader, §16 r1, r3); `restore_level_walk`
(classic with no rows `1` → `None`, §16 r4); items tests "notice" →
"cancel" (§17 r1). Quests: Jerhyn start init (no spawn), sanctuary portal
mode 2 → 1, palace blocker first player, Tyrael per-player distance, bird
boss nothing reported, orb operate mode request + missile 368, act IV end →
`HostRequest`s, Act V scroll reward / town dummies / game start restore
(5D sent, flags kept) / altar portals freed / summit stairs / Tyrael chat
end last portal / spawn Tyrael, gaps class table, e2e
`reading_horazons_journal` and d2-server `forgotten_tower_through_every_state`
expect the 40-byte 0x27.

## 3. Remaining TODOs (with reason)

- **Footprints (G2):** the stamp `0x00620A70` is answered but its mask
  needs objects `BlockMissile` (+438), which the generated `Objects`
  record does not carry (a `fields.tsv` / codegen change); the free
  `0x00623830` has no path-level spec. Both stay `Pending` seams.
- **Item drops of objects:** the §20 helpers (`0x005594C0`, `0x00559630`,
  `0x00559300`, `0x00559A30`) and the code drop `0x00585970` (no body,
  G3) stay seams (`stand_drop`, `gold_drop`, `drop_code_quality`,
  `code_drop`): item generation wiring is impl-items-wiring's area.
- **Object seams without a provider:** trap damage `0x005DFA00`, the gem
  test, tome recount, warp tile `0x005550B0`, day period, quest link of
  init 13, shrine missile creator / unit finder / nearest unique (OQ5),
  name reversal. Portal travel host facts (`Pending::object_party_id`,
  `object_portal_partner`, `object_quest_record`, `object_quest_bit`,
  `object_portal_guid`, `object_level_spawn`, `object_remove_portal`,
  `object_just_portaled`, …) default narrow: a live d2-server host must
  provide them (quest records and party live in `WiredWorld`). Portal
  creation `0x0056D130` has no owner spec.
- **Hirelings:** kill / owner-death run after the handler, not inside the
  kill (lists live in `WiredWorld`); no C→S 0x61 caller (`HirelingItems`
  provider is inventory wiring); no act-change caller (`0x0053ACC0` not
  in d2-sim); no join-follow / restore callers (game entry is action
  wiring; d2-server `character.rs` reports the restore unapplied); free
  branch unit free stands in for `0x00574C60`; pet types other than 7.
- **Quests:** `reward_item` (§9.1) needs the inventory host; seams with no
  provider (`path_target_xy`, `trade_button`, `free_chat_node`,
  `clear_npc_chats`, `obelisk_close`, `steeg_release`, `close_cube`,
  `town_portal_guid`, `portal_partner`, `free_portal_object`,
  `monster_mode_at`); d2-server must drain
  `QuestControl::take_host_requests()`; `helpers.md` OQ1 (recording);
  missiles' own `spawn_tyrael` seam to be pointed at
  `quests::act5::q6::spawn_tyrael` (missiles session).
- `world/objects-client.md` §25–§28 (ClientFn) is client work (d2-client
  model); not started here.

## 4. Gate

Head after the staging-6 merge (`8858c4c`): `CARGO_INCREMENTAL=0 sh
tools/gate.sh` passes every step except `test d2-client`; the coverage
step's 7 dangling `§test-vectors rN` claims were fixed after the run
(`coverage --check`: 0 errors). `test d2-client`: every `Bridge::new`
test fails with `Table(Mismatch([NoHandler { id: 180 }]))`; the same test
fails on `origin/claude/specs-staging-6` @ 8858c4c without this branch
(checked: `bridge::tests::version_mismatch_is_refused`), so it is inherited
(S→C 0xB4 has no client handler row); not fixed here.

Local run queue item (game files): the d2-server `GameTables::object_tables`
and `test-fixtures` `object_tables` now load `objgroup` and `leveldefs`;
run the ignored game-file tests that build them (e.g. `cargo test -p
d2-server -- --ignored`) and check they load.
