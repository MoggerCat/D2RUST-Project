# Handoff: monster population across Act 1, `claude/q-respawn-monsters`

## Trace (input → draw)
Player walks → server room switch → tick room pass (`tick::room_body`)
→ `WorldSim::populate_monsters` / `spawn_presets` (`wiring/worldgen/dispatch.rs:141-167`)
→ `population::room::populate_room` / `preset::place_presets` → monster init
→ S→C 0xAC (type flags, umods, name seed) → client `bridge` unit model
(`bridge/world.rs:72` already reads champion / unique / superunique / minion).

Every link already existed on staging; nothing in `crates/` needed a change.
What was missing was the **check**: `act1_stream.rs` only covered the Blood
Moor, with no bosses, and the shared synthetic set draws no region entry
(`isSpawn` empty, `play-server.md` §3.3) and no random boss (`MonUMin/Max` 0,
monumod row 0 `constants` empty).

## Tests added (`crates/test-fixtures/tests/act1_stream.rs`, synthetic, no game files)
- `every_outdoor_level_spawns_monsters_with_champions_and_uniques`: walking
  town → Blood Moor → Cold Plains → Stony Field populates each level as its
  rooms activate; uniques, champions and their minions spawn, and at least
  one boss GUID reaches the client as 0xAC.
- `a_preset_superunique_spawns_with_its_group_and_reaches_the_client`: a DS1
  preset monster of a superunique row (Bishibosh's kind) is created with the
  superunique type flag, its MinGrp..MaxGrp group, and a 0xAC.
- Fixture set (`spawning_act1`, this file's own copy; the shared set is
  unchanged so other transcripts do not move): `isSpawn`, `MonUMin/Max`
  4..6 on outdoor levels, monumod row 0 `constants` 20 (the 1.14d champion
  chance, `population.md` §6.2), a superunique DS1 object in the town.

## PROVISIONAL / not done
- Synthetic values above are made up (`d2rs-own, unverified`); no REC needed,
  no spec gap met.
- Dungeon levels (Den of Evil, caves, crypts): maze generation is
  `q-act1-dungeons`; population of them rides that.
- Monsters attacking the player: `q-monster-ai` (AI target seams in
  `LocalSeams` are still defaults, `stitch-combat.md` row 12).
- The play app's synthetic `GameData` has no levels or region data, so a
  headless client test of spawning stays out of reach; the live path builds
  `PopTables` from the user's tables (`world_data/game.rs:125`).
- Unverified against 1.14d (no draw-order trace of room population).

## Local check
`RUST_LOG=info cargo run -p d2-client --release -- play --new amazon Test`,
walk out of the Rogue Encampment into the Blood Moor, then Cold Plains and
Stony Field: the model unit count in the log should grow as each area comes
into view (monster 0xAC messages), with some packs. Not checked: how
champions / uniques are drawn (name colour, tint); that is client art and
not part of this change.
