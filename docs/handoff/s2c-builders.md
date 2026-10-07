# Handoff: typed S→C builders in `d2-proto` — `claude/s2c-builders`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation
from written specs, medium (METHODS M14). Branch `claude/s2c-builders`
from `claude/tender-meitner-mphas3` at `9b49081`, then merged with it at
`93b37c8`. Repo only, no game
files. Scope of every claim: this branch, synthetic and spec-quoted
bytes (M09).

## 1. State

**Implemented, unverified** (M02): every layout comes from a draft spec;
the byte vectors are the recorded messages the specs quote, not a replay
of a recording.

- New module `d2_proto::s2c` (`crates/d2-proto/src/s2c/`): one typed
  builder per S→C message whose full layout a spec gives, a pure
  client-side parser, and the audit of every S→C id (§3). No Bevy, no
  I/O.
- `crates/d2-proto/src/generated.rs` **regenerated** with `cargo run -p
  data-tool -- gen-proto`: the base branch was stale against
  `server-messages.tsv` (the `items/inventory.md` commit `ba4438b` added
  `layout` cells for 0x19, 0x3F, 0x42, 0x47, 0x48, 0x7D without
  regenerating; `generated_file_is_current`, `tables_match_tsv` and
  `check_reports_exactly_a_changed_row` failed on `9b49081`). The output
  is deterministic: the coordinator's fix on the base (`e909c15`) is
  byte-identical, so the merge of `93b37c8` left no diff in this file. M21 escape: a TSV edit in a spec session did not run the
  generator; caught by `cargo test -p d2-proto` at the start of this
  session; the existing staleness test is the check (it was not run by
  the spec commit).
- `crates/d2-proto/src/lib.rs`: `pub mod s2c;` and one doc bullet.

Not touched: `d2-server` adapters (host-merge), `d2-client`, `d2-sim`
(the inventory senders 0x9C / 0x9D / 0x7D / 0x3F / 0x42 / 0x47 / 0x48 are
impl-moves' in `d2_sim::items::moves`; their fixed types come from the
generator and are only re-exported here).

## 2. API

- `trait ServerMsg { ID, SIZE, FIELDS, CONSTS, UNWRITTEN, decode, write }`
  plus an inherent `encode() -> [u8; SIZE]` on each type. `CONSTS` =
  bytes a spec fixes (0x28 byte 6 = 0; 0x50 bytes 1–2 = `01 00`, 9–14 =
  0; 0x8A byte 1 = 1); `decode` checks them. `UNWRITTEN` = bytes the
  1.14d builder leaves as stack contents (0x2A bytes 3–6, 0x58 byte 6):
  written 0 (as `d2_sim::world::npc::{transaction, service_result}`
  already do), ignored by `decode`; a comparison against a recording must
  mask them (`intents-events.md` §6 says nothing is ignored by default:
  a spec session should state the mask there).
- Built types (`s2c::messages`): `GameHandshake` 0x0B, `PlayerStop` 0x0D, `QuestInfo` 0x28,
  `GameQuestInfo` 0x29, `NpcTransaction` 0x2A, `MercForHire` 0x4E,
  `QuestSpecial` 0x50 (quest form only), `QuestLogInfo` 0x52, `OpenUi`
  0x58, `AssignPlayer` 0x59, `QuestItemState` 0x5D, `WaypointMenu` 0x63, `TradeAction` 0x77,
  `UniqueEvent` 0x89, `NpcWantsInteract` 0x8A, `NpcGossipAct` 0x91,
  `Unknown9B` 0x9B, `WardenRequest` 0xAE (variable; transport row, out of
  scope, built because its layout is complete). Constants
  `QUEST_RECORD` 96, `WAYPOINT_RECORD` 16, `QUEST_LOG_ENTRIES` 41,
  `GOSSIP_SLOTS` 12, `WARDEN_MAX` 0x1FD.
- Re-exported generated types (TSV layouts): the 103 `generated` rows of
  §3 (73 of them since the 46-row layout batch `9d063f2`, impl-pc1-s5).
- `parse(&[u8]) -> Result<Message, ParseError>`: one whole message (as
  `transport::split_server_buffer` yields it); size by the §3.1 rule
  (`WrongSize`, `Incomplete`, `Invalid`), then the typed decode
  (`WrongId`, `Const`); ids without a full layout →
  `ParseError::Unbuilt { id, status }`. For the bridge
  (`client/bridge.md` §6): a handler can match on `Message` instead of
  reading bytes.
- `AUDIT: [Audit; 0xB5]`, `audit(id)`: `Status::{Built, Generated,
  Partial, IdOnly, Unspecified, Never}`, builder name, note.

Field names: TSV labels for types; a field without a stated meaning is
named by offset (`f6`, as the generator names unnamed fields). 0x0D's
widths come from its recorded bytes (`waypoints.md` Test vectors) and its
values from `waypoints.md` §7 rule 7 (unit type, GUID, 1, x, y, 0, 0):
seven values in 12 bytes have one split only (u8, u32, u8, u16, u16, u8,
u8).

## 3. Audit: every S→C id

`built` = type in `s2c::messages`; `generated` = TSV layout, type in
`generated::server`; `partial` = some bytes given (what is missing in the
note); `unspecified` = size rule only; `idonly` = empty layout that is complete; `never` = size 0. Checked row for
row against `s2c::AUDIT` by `note_table_matches_audit`.

| Id | Name | Size | Status | Builder | Note |
|---|---|---|---|---|---|
| 0x00 | GameLoading | 1 | generated | `GameLoading` | TSV layout |
| 0x01 | GameFlags | 8 | generated | `GameFlags` | TSV layout |
| 0x02 | LoadSuccessful | 1 | generated | `LoadSuccessful` | TSV layout |
| 0x03 | LoadAct | 12 | generated | `LoadAct` | TSV layout |
| 0x04 | LoadComplete | 1 | generated | `LoadComplete` | TSV layout |
| 0x05 | UnloadComplete | 1 | generated | `UnloadComplete` | TSV layout |
| 0x06 | GameExit | 1 | generated | `GameExit` | TSV layout |
| 0x07 | MapReveal | 6 | generated | `MapReveal` | TSV layout |
| 0x08 | MapHide | 6 | generated | `MapHide` | TSV layout |
| 0x09 | AssignLevelWarp | 11 | generated | `AssignLevelWarp` | TSV layout |
| 0x0A | RemoveUnit | 6 | generated | `RemoveUnit` | TSV layout |
| 0x0B | GameHandshake | 6 | built | `GameHandshake` | model.md §3 r1 (type, GUID) |
| 0x0C | MonsterHit | 9 | generated | `MonsterHit` | TSV layout |
| 0x0D | PlayerStop | 13 | built | `PlayerStop` | waypoints.md §7 r7 values + recorded widths |
| 0x0E | ObjectState | 12 | generated | `ObjectState` | TSV layout |
| 0x0F | PlayerMove | 16 | generated | `PlayerMove` | TSV layout |
| 0x10 | PlayerToTarget | 16 | generated | `PlayerToTarget` | TSV layout |
| 0x11 | ReportKill | 8 | generated | `ReportKill` | TSV layout |
| 0x12 | Unknown12 | 26 | unspecified | - | size rule only |
| 0x13 | Unknown13 | 14 | unspecified | - | size rule only |
| 0x14 | Unknown14 | 18 | unspecified | - | size rule only |
| 0x15 | ReassignPlayer | 11 | generated | `ReassignPlayer` | TSV layout |
| 0x16 | UnitPositions | u16@1;min=13 | unspecified | - | size rule only |
| 0x17 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x18 | LifeManaUpdate | 15 | generated | `LifeManaUpdate` | TSV layout |
| 0x19 | SmallGoldPickup | 2 | generated | `SmallGoldPickup` | TSV layout |
| 0x1A | AddExpByte | 2 | generated | `AddExpByte` | TSV layout |
| 0x1B | AddExpWord | 3 | generated | `AddExpWord` | TSV layout |
| 0x1C | AddExpDword | 5 | generated | `AddExpDword` | TSV layout |
| 0x1D | SetStatByte | 3 | generated | `SetStatByte` | TSV layout |
| 0x1E | SetStatWord | 4 | generated | `SetStatWord` | TSV layout |
| 0x1F | SetStatDword | 6 | generated | `SetStatDword` | TSV layout |
| 0x20 | StatUpdate | 10 | generated | `StatUpdate` | TSV layout |
| 0x21 | UpdateItemOSkill | 12 | generated | `UpdateItemOSkill` | TSV layout |
| 0x22 | UpdateItemSkill | 12 | generated | `UpdateItemSkill` | TSV layout |
| 0x23 | SetSkill | 13 | generated | `SetSkill` | TSV layout |
| 0x24 | Unknown24 | 90 | unspecified | - | size rule only |
| 0x25 | Unknown25 | 90 | unspecified | - | size rule only |
| 0x26 | Chat | chat26 | unspecified | - | size rule only |
| 0x27 | NpcInfo | 40 | generated | `NpcInfo` | TSV layout |
| 0x28 | QuestInfo | 103 | built | `QuestInfo` | quests.md §1.5 |
| 0x29 | GameQuestInfo | 97 | built | `GameQuestInfo` | quests.md §1.5 |
| 0x2A | NpcTransaction | 15 | built | `NpcTransaction` | npc.md §9; bytes 3–6 unwritten |
| 0x2B | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x2C | PlaySound | 8 | generated | `PlaySound` | TSV layout |
| 0x2D | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x2E | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x2F | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x30 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x31 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x32 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x33 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x34 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x35 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x36 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x37 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x38 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x39 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x3A | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x3B | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x3C | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x3D | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x3E | UpdateItemStats | u8@1;min=2 | unspecified | - | size rule only |
| 0x3F | UseStackableItem | 8 | generated | `UseStackableItem` | TSV layout; senders: impl-moves (inventory-moves.md §11) |
| 0x40 | ItemFlags | 13 | generated | `ItemFlags` | TSV layout |
| 0x41 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x42 | ClearCursor | 6 | generated | `ClearCursor` | TSV layout; senders: impl-moves (inventory-moves.md §11) |
| 0x43 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x44 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x45 | Unknown45 | 13 | unspecified | - | size rule only |
| 0x46 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x47 | Relator1 | 11 | generated | `Relator1` | TSV layout; senders: impl-moves (inventory-moves.md §11) |
| 0x48 | Relator2 | 11 | generated | `Relator2` | TSV layout; senders: impl-moves (inventory-moves.md §11) |
| 0x49 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x4A | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x4B | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x4C | UnitSkillOnUnit | 16 | generated | `UnitSkillOnUnit` | TSV layout |
| 0x4D | UnitSkillOnPoint | 17 | generated | `UnitSkillOnPoint` | TSV layout |
| 0x4E | MercForHire | 7 | built | `MercForHire` | npc.md §7.2 |
| 0x4F | StartMercList | 1 | generated | `StartMercList` | TSV layout |
| 0x50 | QuestSpecial | 15 | partial | `QuestSpecial` | quest form (u16 1 @1) built as QuestSpecial; mercenary form (u16 2 @1, name u16 @3; npc.md §7.5) has no bytes 5–14 |
| 0x51 | AssignObject | 14 | generated | `AssignObject` | TSV layout |
| 0x52 | QuestLogInfo | 42 | built | `QuestLogInfo` | quests.md §6.2 step 4 |
| 0x53 | Darkness | 10 | generated | `Darkness` | TSV layout |
| 0x54 | Unknown54 | 3 | unspecified | - | size rule only |
| 0x55 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x56 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x57 | NpcEnchants | 14 | generated | `NpcEnchants` | TSV layout |
| 0x58 | OpenUi | 7 | built | `OpenUi` | npc.md §8.1; byte 6 unwritten |
| 0x59 | AssignPlayer | 26 | built | `AssignPlayer` | msg-units.md §1.1 r1 offsets, intents-events.md §7.2 fields |
| 0x5A | EventMessage | 40 | generated | `EventMessage` | TSV layout |
| 0x5B | PlayerJoined | u16@1;min=34 | unspecified | - | size rule only |
| 0x5C | PlayerLeft | 5 | generated | `PlayerLeft` | TSV layout |
| 0x5D | QuestItemState | 6 | built | `QuestItemState` | quests.md §6.3 |
| 0x5E | GameQuestAvailability | 38 | generated | `GameQuestAvailability` | TSV layout |
| 0x5F | PortalFlags | 5 | generated | `PortalFlags` | TSV layout |
| 0x60 | TownPortalState | 7 | generated | `TownPortalState` | TSV layout |
| 0x61 | CanGoToAct | 2 | generated | `CanGoToAct` | TSV layout |
| 0x62 | MakeUnitTargetable | 7 | generated | `MakeUnitTargetable` | TSV layout |
| 0x63 | WaypointMenu | 21 | built | `WaypointMenu` | waypoints.md §5.3 |
| 0x64 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x65 | PlayerKillCount | 7 | generated | `PlayerKillCount` | TSV layout |
| 0x66 | Unknown66 | 7 | unspecified | - | size rule only |
| 0x67 | MonsterMove | 16 | generated | `MonsterMove` | TSV layout |
| 0x68 | MonsterMoveToTarget | 21 | generated | `MonsterMoveToTarget` | TSV layout |
| 0x69 | MonsterState | 12 | generated | `MonsterState` | TSV layout |
| 0x6A | Unknown6A | 12 | generated | `Unknown6A` | TSV layout |
| 0x6B | MonsterAction | 16 | generated | `MonsterAction` | TSV layout |
| 0x6C | MonsterAttack | 16 | generated | `MonsterAttack` | TSV layout |
| 0x6D | MonsterStop | 10 | generated | `MonsterStop` | TSV layout |
| 0x6E | Unknown6E | 1 | generated | `Unknown6E` | TSV layout |
| 0x6F | Unknown6F | 1 | generated | `Unknown6F` | TSV layout |
| 0x70 | Unknown70 | 1 | generated | `Unknown70` | TSV layout |
| 0x71 | Unknown71 | 1 | generated | `Unknown71` | TSV layout |
| 0x72 | Unknown72 | 1 | generated | `Unknown72` | TSV layout |
| 0x73 | Unknown73 | 32 | generated | `Unknown73` | TSV layout |
| 0x74 | PlayerCorpseAssign | 10 | generated | `PlayerCorpseAssign` | TSV layout |
| 0x75 | PlayerPartyInfo | 13 | generated | `PlayerPartyInfo` | TSV layout |
| 0x76 | PlayerInProximity | 6 | generated | `PlayerInProximity` | TSV layout |
| 0x77 | TradeAction | 2 | built | `TradeAction` | cube.md §1 (action byte) |
| 0x78 | TradeAccepted | 21 | generated | `TradeAccepted` | TSV layout |
| 0x79 | GoldInTrade | 6 | generated | `GoldInTrade` | TSV layout |
| 0x7A | PetAction | 13 | generated | `PetAction` | TSV layout |
| 0x7B | AssignHotkey | 8 | generated | `AssignHotkey` | TSV layout |
| 0x7C | UseScroll | 6 | generated | `UseScroll` | TSV layout |
| 0x7D | SetItemState | 18 | generated | `SetItemState` | TSV layout; senders: impl-moves (inventory-moves.md §11) |
| 0x7E | Unknown7E | 5 | idonly | - | empty layout complete (id only; bytes 1-4 unwritten) |
| 0x7F | AllyPartyInfo | 10 | generated | `AllyPartyInfo` | TSV layout |
| 0x80 | - | 0 | never | - | size 0 (client expects 4; never receivable, §3.1 r2) |
| 0x81 | AssignMerc | 20 | generated | `AssignMerc` | TSV layout |
| 0x82 | PortalOwnership | 29 | generated | `PortalOwnership` | TSV layout |
| 0x83 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x84 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x85 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x86 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x87 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x88 | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0x89 | UniqueEvent | 2 | built | `UniqueEvent` | quests.md §6.5 |
| 0x8A | NpcWantsInteract | 6 | built | `NpcWantsInteract` | quests.md §6.4 |
| 0x8B | PlayerRelationship | 6 | generated | `PlayerRelationship` | TSV layout |
| 0x8C | RelationshipUpdate | 11 | generated | `RelationshipUpdate` | TSV layout |
| 0x8D | AssignPlayerToParty | 7 | generated | `AssignPlayerToParty` | TSV layout |
| 0x8E | CorpseAssign | 10 | generated | `CorpseAssign` | TSV layout |
| 0x8F | Pong | 33 | generated | `Pong` | TSV layout |
| 0x90 | PartyAutomapInfo | 13 | generated | `PartyAutomapInfo` | TSV layout |
| 0x91 | NpcGossipAct | 26 | built | `NpcGossipAct` | quests.md §6.7 |
| 0x92 | RemoveItemsDisplay | 6 | generated | `RemoveItemsDisplay` | TSV layout |
| 0x93 | Unknown93 | 8 | generated | `Unknown93` | TSV layout |
| 0x94 | BaseSkillLevels | u8@1*3+6;min=9 | unspecified | - | size rule only |
| 0x95 | LifeManaUpdate2 | 13 | generated | `LifeManaUpdate2` | TSV layout |
| 0x96 | WalkVerify | 9 | generated | `WalkVerify` | TSV layout |
| 0x97 | WeaponSwitch | 1 | generated | `WeaponSwitch` | TSV layout |
| 0x98 | Unknown98 | 7 | generated | `Unknown98` | TSV layout |
| 0x99 | SkillTriggered | 16 | generated | `SkillTriggered` | TSV layout |
| 0x9A | Unknown9A | 17 | generated | `Unknown9A` | TSV layout |
| 0x9B | Unknown9B | 7 | built | `Unknown9B` | npc.md §7.3 step 4 |
| 0x9C | ItemActionWorld | u8@2;min=3 | partial | - | header given (inventory-moves.md §11); item bit stream unspecified (inventory.md OQ1); senders: impl-moves |
| 0x9D | ItemActionOwned | u8@2;min=3 | partial | - | header given (inventory-moves.md §11); item bit stream unspecified (inventory.md OQ1); senders: impl-moves |
| 0x9E | MercStatByte | 7 | generated | `MercStatByte` | TSV layout |
| 0x9F | MercStatWord | 8 | generated | `MercStatWord` | TSV layout |
| 0xA0 | MercStatDword | 10 | generated | `MercStatDword` | TSV layout |
| 0xA1 | MercAddExpByte | 7 | generated | `MercAddExpByte` | TSV layout |
| 0xA2 | MercAddExpWord | 8 | generated | `MercAddExpWord` | TSV layout |
| 0xA3 | UnknownA3 | 24 | generated | `UnknownA3` | TSV layout |
| 0xA4 | BaalWave | 3 | generated | `BaalWave` | TSV layout |
| 0xA5 | UnknownA5 | 8 | generated | `UnknownA5` | TSV layout |
| 0xA6 | UnknownA6 | u16@2;min=4 | unspecified | - | size rule only |
| 0xA7 | DelayedState | 7 | generated | `DelayedState` | TSV layout |
| 0xA8 | SetState | u8@6;min=7 | unspecified | - | size rule only |
| 0xA9 | EndState | 7 | generated | `EndState` | TSV layout |
| 0xAA | AddUnit | u8@6;min=7 | unspecified | - | size rule only |
| 0xAB | NpcHeal | 7 | generated | `NpcHeal` | TSV layout |
| 0xAC | AssignMonster | u8@12;min=13 | partial | - | fields listed (init.md §24), not their byte/bit positions |
| 0xAD | - | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0xAE | WardenRequest | u16@1+3;cap=0x1FD;min=3 | built | `WardenRequest` | TSV layout (transport row, out of scope) |
| 0xAF | ConnectionInfo | af | unspecified | - | size rule only |
| 0xB0 | ConnectionTerminated | 1 | generated | `ConnectionTerminated` | TSV layout |
| 0xB1 | GamesInfo | 0 | never | - | size 0: never receivable (§3.1 r3) |
| 0xB2 | GameList | 53 | generated | `GameList` | TSV layout |
| 0xB3 | DownloadSave | u8@1+7;min=8 | unspecified | - | size rule only |
| 0xB4 | ConnectionRefused | 5 | generated | `ConnectionRefused` | TSV layout |

Totals (181 ids): 15 built (14 fixed + 0xAE), 30 generated, 20 partial
(0x50 with its quest form built as `QuestSpecial`), 78 unspecified, 38
never.

## 4. Checks (`cargo test -p d2-proto s2c`)

| Test | Proves | M08 perturbation |
|---|---|---|
| `sizes_match_tsv` | every built type's `SIZE` = the TSV fixed size | 0x50 size + 1 → exactly 0x50 reported |
| `layouts_cover_every_byte` | bytes 1..SIZE covered once by fields, constants, unwritten bytes | 0x2A without `guid` → exactly bytes 7–10; an overlap → byte 3 |
| `audit_matches_tsv_parser_and_types` | audit vs TSV (never ⇔ size 0), parser ids, built type names, generated names | three changed rows → exactly 0x08, 0x63, 0x77 |
| `note_table_matches_audit` | §3 above = `AUDIT` | one changed status → exactly 0x63 |
| `recorded_waypoint_messages` | 0x63 ×3, 0x0D ×2, 0x07 recorded bytes encode and parse | |
| `recorded_npc_and_quest_messages` | 0x2A ×2 (bytes 3–6 masked), 0x28 prefixes, 0x5D recorded + both fixed forms, 0x8A, 0x9B, 0x77 ×3 | |
| `recorded_vector_perturbation_is_reported` | | every single-byte flip of a recorded 0x63 / 0x2A reported at its offset; flips of 0x2A bytes 3–6 not |
| `recorded_unbuilt_messages_size_and_refuse` | recorded 0x15 ×2, 0x51 ×2 have the TSV size and parse as `Unbuilt`; 0x27 prefix, 0x50 mercenary form `Unbuilt` | |
| `built_messages_round_trip_through_the_size_rule` | every built type: encode → size rule = length → parse = same value | |
| `parse_rejects_bad_messages` | empty, size 0, past 0xB4, incomplete, wrong size, constant, unbuilt; unwritten bytes ignored | |
| `warden_request_size_rule` | 0xAE: `AE 10 00` → 19, max 0x200, over-cap length → 3 | |

No coverage claims: the vectors check layouts, while the spec sections
holding them also state sender behavior these tests do not run
(`coverage-claims.md` §1).

Gate (this branch): `cargo fmt --all -- --check`; `cargo clippy
--workspace --all-targets -- -D warnings`; `cargo test -p d2-proto`;
`cargo run -p depcheck`; `python3 tools/spec_index.py --check`;
`python3 tools/methods.py check`; `python3 tools/coverage.py --check` and
`--selftest`. Results in §6.

## 5. Open questions / next

1. Spec sessions (local, Ghidra): layouts of the `partial` rows the
   sim needs first: 0x15 (sender `0x0053BC10`, HANDOFF §7 q8), 0x51
   (`0x0053BD10`), 0x5A (`use.md` OQ9), 0x27's 34-byte text list
   (`0x00661480`), 0x50's mercenary form, 0xAC's bit positions
   (`init.md` §24), the item bit stream of 0x9C / 0x9D
   (`inventory.md` OQ1). Each row moves to `built` with a type and a
   vector from the recordings.
2. `intents-events.md` §6: name the unwritten bytes (0x2A 3–6, 0x58 6)
   as the only masked bytes of the exact-match comparison.
3. `d2-sim` keeps its own byte builders (`world::npc::{transaction,
   service_result, resurrect_message}`, `world::waypoints` 0x63, cube
   0x77, the 0x0D in the waypoint travel). Switching them to `d2_proto::s2c`
   needs `d2-sim` → `d2-proto` as a dependency (depcheck allows it; not
   done here: outside this session's files).
4. The bridge's `bridge-dispatch.tsv` is still all `TBD`; the owner
   rows can now name a `s2c::Message` variant.

## 6. Gate results (this branch, 2026-10-06)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -p d2-proto`: 22 + 8 pass (11 of them the new `s2c` tests;
  the three staleness tests pass again after the regeneration).
- `cargo run -p depcheck`: OK (8 crates, determinism lint clean).
- `python3 tools/spec_index.py --check`, `python3 tools/methods.py
  check` (21 methods), `python3 tools/coverage.py --check` (3259 claims,
  0 errors), `--selftest`: OK.
- Re-run after merging the base at `93b37c8`: all of the above pass
  (coverage 3285 claims, 0 errors).
