# q-prov-recording: settle the `recording` provisional points under Wine

Cloud session, 2026-10-09 (day run, REC block 550–559). Branch
`claude/q-prov-recording`. Implementation-side session (CLAUDE.md rule 3):
reads `specs/`, `docs/`, `crates/`, `facts/`, `traces/`, `tools/`; never
`re/` or `../refs/`. The oracle is 1.14d under Wine 9.0 + Xvfb
(`tools/cloud-game/`, REC-290: RNG equal to PC 1, Wine runs byte-repeatable).

Input: `docs/handoff/provisional-index.tsv`, 273 rows with `settle_kind =
recording` (a keyword sort of the REC text, not a verdict).
`tools/cloud-game/prov_recording_groups.py` maps each row to the
scenario that would settle it and writes `q-prov-recording.tsv` (group,
rec, location); re-run it after `tools/provisional_index.py`.

## Groups, in work order

Order: impact first (wrong game outcomes: wire bytes, items, drops, RNG >
timing > cosmetics), then points per recording, then what the cloud can
run unattended (`--auto` + `--input`; `goto` walks to a unit; `d2s-tool`
makes characters with items, skills, waypoints, quests).

| # | Group | Points | Impact | Recording (cloud) | Cloud status |
|---|---|---|---|---|---|
| G1 | Join stream: packets at game start (fresh / existing character, start skills, hot keys, equipped item lists, hireling, corpse) — REC-02, REC-188, REC-405, REC-241, REC-282, REC-46, REC-44 | 19 | wire | `record_packets.py --auto ScnAma --seed 1234 --input "wait 5; end"`; again with `TestSor`; a `d2s-tool` character with equipped magic / set items | auto. REC-46 (classic, menu-made) and REC-44 (legacy saves; deferred by the user) are manual |
| G2 | Item moves in town: cursor drop / pick-up / drop, equip, belt potion, identify, socket, charm, stash — REC-289, REC-281, REC-102, REC-113, REC-121, REC-163, REC-158, REC-266, REC-34, REC-177 | 33 | items | `record_packets.py` on a `d2s-tool` character holding the items, clicks on fixed inventory / ground pixels | auto with an input script |
| G4 | Blood Moor field: kill, monster drop spot, chest, shrine, hit reaction, level-up, death — REC-108, REC-260, REC-141, REC-124, REC-96, REC-93, REC-08, REC-07, REC-239, REC-263, REC-126, REC-291 | 27 | drops, combat | `record_packets.py` (+ `record_rng.py` twin), high-level character, `goto` to monsters / chests | auto for kill / drop / chest (goto); death and shrine depend on the map |
| G5 | Warp tile and interact range: Den of Evil entrance, waypoint, chest from 1 / 3 / 5 / 8 subtiles — REC-99, REC-94 | 12 | movement, warp | `record_packets.py` + hooks `0x00548C32`, `0x00623660` (new hooks) | needs two new hooks |
| G3 | Town Portal: scroll / tome, field cast, in-town cast, portal pair, rejoin — REC-117, REC-243 | 5 | items, objects | `record_packets.py`, a character with `tsc` / `tbk` | auto |
| G7 | Protocol hook sittings: overlay 0x11, leap 0xA5, heal 0xAB, item cast 0x99, missile 0x73, chat 0x15, skill-mode 0x4C / 0x4D (two clients), sentry, `0x00476A80`, Leap / Whirlwind — REC-410..414, REC-402, REC-95, REC-461, REC-62, REC-275 | 25 | wire | `record_packets.py` with skill characters; R-OVL11-1, R-HEAL-1, R-CHAT15-1 need hooks; REC-95 needs two clients (TCP/IP) | partly auto; two-client games unproved under Wine |
| G8 | DRLG / population hooks: border substitution file, Trees DS1, waypoint-walk probes, Radament — REC-404, REC-35, REC-04, REC-81 | 9 | world gen | `record_rng.py` / `record_tick.py` + new hooks | needs new hooks |
| G6 | Town walk / run and NPC interaction: local vs server position, walk prediction, shop open, visibility, char panel, item tips — REC-51, REC-277, REC-288, REC-286, REC-269, REC-268, REC-242 | 42 | client timing / cosmetics | `record_frames.py` + `record_packets.py`; REC-51 needs hooks on `0x004AFF60`, `0x00464810`, `0x004647D0`, `0x00463390` | needs new hooks; REC-51's client seed draws overlap q-fix-real-unit-seed-order |
| G9 | Later-act and quest sittings (Act II–V saves) — REC-45, REC-27, REC-128, REC-129, REC-136, REC-148, REC-167, REC-235, REC-246, REC-254, REC-11 | 24 | quests | `d2s-tool` saves at the quest point, `record_packets.py` | mostly manual play |
| G11 | Audio: UI request sites, async loads, footstep frames — REC-19, REC-430 | 8 | timing / cosmetics | `record_sound.py` (not written), R-ANIM-1 hook | sound device absent under Wine (`-ns`); deferred |
| G10 | Front end: menus, create, controls, credits, cinematics, loading, Esc menu, exit target — REC-181..186, REC-200..228, REC-236 | 31 | cosmetics / timing | `record_frames.py` without `--auto` + menu clicks | needs menu driving (no `--auto`) |
| G12 | Render-only captures: summit clouds, state tint, camera, x87 control word, weather, automap, sprite limits — REC-420, REC-245, REC-21, REC-97 | 18 | cosmetics | `record_frames.py` | low priority |
| SKIP | Owned by running sessions: client missiles (REC-450..452, `client-bodies-2.md`), scenes compare (REC-440, REC-441), preset objects (fixture migration) | 13 | — | — | not worked here |
| DONE | REC entry already settled from the binary (REC-60, REC-82, pc1-s8); the PROVISIONAL line names it by proximity or awaits clean-up | 7 | — | — | line clean-up only |
| | **Total** | **273** | | | |

## Rules for each group

1. Record under Wine (`tools/cloud-game/run.sh --python`), private data
   repo install per its README (`fetch.sh`).
2. Commit what a tool writes from the recording (our own observations,
   CLAUDE.md rule 1): facts / digests with format version and command, in
   `facts/` or `traces/`; raw captures stay in `traces/raw/` (gitignored).
3. Compare with d2rs. Per point: **settled (confirmed)** in HANDOFF, or a
   `q-fix-*` row in `build-queue.tsv` with the measured difference; small
   local fixes made here.
4. Re-run `python3 tools/provisional_index.py` and this grouping.
5. Report to the coordinator after each group (commit, settled / rows,
   remaining).

## Results

(filled per group below)

### G1 — join stream (2026-10-09)

Recordings (Wine, `record_packets.py`; facts by `tools/trace-recorder/facts_join.py`, `join-facts-1`):

| Facts | Run |
|---|---|
| `facts/join/a1-save-ScnAma.tsv` | `--auto ScnAma --seed 1234 --input "wait 5; end"` (REC-02 run A) |
| `facts/join/a1-save-TestSor.tsv` | `--auto TestSor --seed 644409375` (run C) |
| `facts/join/a1-new-ama.tsv`, `a1-new-sor.tsv` | `--seed 1234 --menu "wait 70"` + `tools/cloud-game/xinput.sh "15 400 307; 19 117 498; 23 <hero x> 300; 28 text <name>; 31 key Return"`: a character made in the create screen (the stub path) |
| `facts/join/a1-new-classic-ama.tsv` | as above with the Expansion box unchecked (`27 326 532`) |

Front-end input: `PostMessage` clicks do not reach the 1.14d front end under
Wine; X input (`xdotool`, `tools/cloud-game/xinput.sh`, window at 112, 98 on
the 1024×768 Xvfb screen) does. Under the recorder the menu is about 4 s
slower than a plain run. `autostart.py --menu SCRIPT` (0.2.0) keeps the
recorder from forcing the start, so the menu path runs.

| Point | Result |
|---|---|
| `d2-client` `app/single_player.rs` `CREATE_FLAGS_CLASSIC`, `tests/app_single_player.rs` (REC-46) | **settled (confirmed)**: classic 0x67 u32@0x27 = 0x4; S→C 0x01 u32@2 = 0x4, u8@6 = 0 |
| `d2-server` `handlers/player.rs` `HotKey::UNBOUND` (REC-02) | **settled (confirmed)**: no 0x7B in a new character's join |
| `d2-server` `session.rs` `PlayerRecord::new_character`, `d2s-load.md` §8 r3, `synthetic_game.rs:1375` (REC-02) | **differs**: 1.14d sends item 0 in both hand 0x23 → `q-fix-real-newchar-hand-item` |
| `d2-server` `character.rs:373` start skill (REC-02) | outcome confirmed (Sorceress right skill 36, three 0x23); the start items / 0x21 / 0x22 d2rs lacks → `q-fix-real-newchar-start-items` |
| `world/quests.md` OQ4 R-PQ-3 (not an index row) | confirmed: 0x5E / 0x28 / 0x29 twice for a new character, once for a save |
| REC-405 (2) | not a recording point ("nothing to record": a host refactor) |
| REC-44 (3) | deferred by the user (legacy saves) |
| REC-188, REC-241 (2), REC-282, `msg/items.rs`, `unit_misc.rs`, `world.rs` hireling | open: need set / magic items, a weapon-swap save, a corpse save or a hireling, which `d2s-tool` cannot write yet |

### G2 / G3 — item moves and Town Portal (2026-10-09)

Characters by `d2s-tool new` (new `--item CODE#Q` quantity suffix for part
stacks). Facts (`facts_join.py` 0.2.0 with `--from 2 --frames 0 --skip`):

| Facts | Run |
|---|---|
| `facts/items/a1-town-item-moves-full.tsv` | ItmAma, full stacks: key overflow merge, scroll onto a full tome (nothing sent) |
| `facts/items/a1-town-item-moves.tsv` | ItmAmb: keys 3 + 4, quivers 30 + 40 (0x21); `tsc` onto `tbk` of 5 (0x29); hp1 belt use at full life; in-town `tsc` use; mp1 use; drop (0x17) and pick-up (0x16) |
| `facts/items/a1-town-potions-low.tsv` | ItmAmc (`--stat 6=2560 --stat 8=256`): hp1 from the belt, mp1 from the inventory |
| `facts/items/a1-town-portal-cold-plains.tsv` | TpAmb (level 30, high life): waypoint to Cold Plains, `tsc` cast, field portal to town, in-town `tbk` cast, town portal back |

| Point | Result |
|---|---|
| `items/moves/handlers.rs:986`, `wiring/inventory/tests/stack.rs:83`, `inventory-moves.md` §7.12 (REC-289) | **settled (confirmed)**: items without durability merge |
| (not an index row) §7.12 / §7.20 S→C 0x42 | **differed, fixed here**: 0x42 names the player (`42 00 <GUID>`); d2rs named the source item (type 4), which the client ignores. The cursor scroll put into its tome now also sends it (d2rs sent none) |
| `wiring/action/switch.rs:460` (0x82 fields) | **settled (confirmed)**: (this, pair) on both portals |
| `wiring/inventory/potion.rs:5` (REC-102) | **differs** → `q-fix-real-potion-effect` (170-frame hp1 state; ends at once at full life) |
| `wiring/action/town_portal.rs:12`, `:23` (REC-117, REC-243) | **differs** → `q-fix-real-tp-town-cast` (in-town cast refused, no cost) |
| `tests/app_town_portal.rs:9`, `town_portal.rs:192` (REC-117) | partly: links and the pair's removal on the way back confirmed; positions and the removal notice need a d2rs replay |
| REC-281 drop / pick-up (`moves.rs:525`, `item_approach.rs`, …) | recorded (one 0x9C action 2 on the drop, nothing while it lies, a potion picked up goes to the belt, 0x9C action 0xE); the d2rs replay (`traces/scenarios/belt-potion-drop.scenario`) is still to run (a draft scenario of the steps is not committed until `scenario-run check` passes on it) |
| REC-113 identify, REC-121 sockets, REC-163 charms, REC-188 | open: `d2s-tool` writes identified normal items only |

### G10 — front end, first pass (2026-10-09)

Plain `run.sh` (no recorder) with `tools/cloud-game/xinput.sh`, X
screenshots (`--shot-at`); the first click is repeated (8, 10, 12 s)
because the menu appears between 7 and 11 s.

| Point | Result |
|---|---|
| `frontend-menus.md` §F3.4 r2 (REC-209) | **settled (confirmed)**: a leading `-` is rejected, `a-` accepted |
| `frontend-menus.md` §F2 slot text (REC-207) | **settled (confirmed)**, text only: `Level 30 Amazon`, `Level 1 Sorceress` |
| `frontend-menus.md` §F1.3, `save-exit.md` §4 r2, `app/front_start.rs:66`, `app/save.rs:579` (REC-200) | **differs**: Save and Exit shows the main menu → specs updated, `q-fix-real-exit-target` |
| `frontend-credits.md` (REC-226) | **settled (confirmed)**: with registry `Resolution` = 0 the front end, Credits and Cinematics stay in the 800 × 600 frame; main-menu buttons pixel-equal (AE 0) to the default run |
| `frontend-menus.md` §F2 r2 (REC-206) | **settled (confirmed)** for the folder choice (a legacy folder with a save is kept; an empty one → default, written as `NewSavePath`); new PROVISIONAL: no `NewSavePath` written in the legacy case |
| REC-205 dead hardcore figure | observed (two `d2s-tool` hardcore saves with status bit 0x08, Amazon and Barbarian): both slots show a grey hooded figure that looks alike; the backgrounds differ, so pixel equality of the two figures is not established: open |
| REC-212 Esc menu | screenshot taken; the pentagram pixels need the cel geometry for an exact check: open |

### REC-290 tick half (2026-10-09)

Wine: `record_tick.py --auto ScnAma --seed 1234 --ticks 600` (same command as PC 1's
`traces/sim/tick/sim-0009.json`), `check_tick.py` 0 errors, converted with `convert_tick.py`
(not committed). Ticks 0–60: **equal** to sim-0009 (340 inputs, 201 expected records). At tick 61
the Windows trace has a client message in the drain (step `pre`: a timer on unit (1, 7), a player
queue entry) and the player then changes rooms at ticks 121, 241, 351; the Wine run has no input.
The PC 1 window received input (likely the mouse over the window): **re-record sim-0009 on PC 1
with the mouse outside the window**; until then REC-290 ticks are equal over 60 ticks only.

### Blocker tools (2026-10-09, coordinator's order)

1. **d2s-tool item writer** (`--item …/q=/idx=/ilvl=/sock=/unid/eth/body=/belt=`; `dump --items`):
   checked byte for byte against 1.14d's own re-save (`facts/saves/gear-roundtrip.tsv`, 14 of 15
   equal, the 15th gets the game's 0x4000 "requirements not met" flag; `tools/d2s-tool/tests/item_roundtrip.rs`).
   Found on the way: 1.14d writes an equipped item's x as its body location; it cuts sockets to
   `gemsockets`; it drops an item equipped at a location its base does not fit.
2. **Item creation on the d2rs side** (q-tool-poke: "b", the character layer belongs to scenario-run):
   `scenario-run run --save-dir DIR` now loads `char save <name>.d2s` on the d2rs side through
   `d2_server::adapters::session::load_save`, so both sides start from the same save; steps the
   scenario host cannot apply are gaps. Today: `header`, `quests`, `npc fields`, `items`, `item
   indices`, `quest entry` are unapplied, because the host is `ActionWorld`; the items need
   `WiredWorld::load_items` (the play app's host). Next: run the scenario host on `WiredWorld`.
3. **Pokes in `record_packets.py`** (0.2.0, `--poke` / `--poke-file`, at its 0x0052FD1E hook): the
   first poke run on 1.14d: `spawn 19` next to the player in Cold Plains works (5 of 10, the others'
   spots taken). G4 then gave REC-108: drops land beside the death point →
   `q-fix-real-monster-drop-spot`.

### G2 with the new item writer (2026-10-09)

| Point | Result |
|---|---|
| `wiring/inventory/identify.rs:6`, `pending.rs:174`, `pending.rs:510` (REC-113) | **settled (confirmed)** (`facts/items/a1-town-identify.tsv`) |
| `wiring/inventory/units.rs:226` (REC-121) | **settled (confirmed)** (`facts/items/a1-town-socket-fill.tsv`) |
| REC-188 server half (not an index row) | confirmed: the join's 0x1D–0x1F carry base stats only (dex 25 with a +1 dex charm), and moving a charm or an equipped magic ring sends no stat message (`facts/items/a1-town-charm-ring.tsv`, raw g2charm); the client's list attach (`item_lists.rs:14`) and REC-163's server link are not visible on the wire: open |

Chests by poke (`object 5 @x+4 @y+4`, Cold Plains): created and opened (C→S 0x13 → S→C 0x0E
mode change), but nothing drops: the `object` directive's allocator path does not run the chest's
init (`objects.md` init 3), so a poked chest is not a real chest. REC-260 / REC-93 need a preset
chest (a route to one, or a poke that runs the object init).

## Where the rest is blocked (2026-10-09, end of this session)

Remaining `recording` rows after the runs above: see the last
`prov_recording_groups.py` count in the coordinator report. What stops
each group in the cloud, and the tool work that would unblock it:

| Group | Blocker | Unblocks it |
|---|---|---|
| G1 rest, G2 rest (REC-188, REC-113, REC-121, REC-163, REC-241, REC-282, hireling) | `d2s-tool` writes identified normal items on storage pages only: no magic / set / unidentified items, sockets, body or belt slots, corpses, hirelings | an item writer for `items/bitstream.md` in `d2s-tool` (`scenario.md` OQ 3) |
| d2rs side of every item comparison (REC-281, REC-117 positions) | `scenario-run run` reports `char item: … not created` and stages waypoint objects only | item creation from a code in the d2rs scenario runner |
| G4 field (REC-108, REC-260, REC-141, REC-124, REC-96, REC-93) | kills work (`goto 1 -1`, level-30 character with high life in Cold Plains) but drops are rare with fists, chests / shrines are not near the waypoint, and a death needs a weak character far from the portal | `poke.py` directives inside `record_packets.py` (spawn a monster / object next to the player after arrival; REC-590 proves poke on 1.14d first) |
| G5 warp tiles (REC-99, REC-94 range) | no type-5 client unit and no S→C 0x09 near the arrival points tried (Cold Plains waypoint, Lut Gholein start); the client sends 0x02 every 6 frames then 0x13 on arrival for a waypoint (recorded in `facts/items/a1-town-portal-cold-plains.tsv` frames 85–105) | a route script to a known tile (Den of Evil entrance) or `poke` `warp`; the `0x00548C32` / `0x00623660` hooks in `record_packets.py` |
| G6 (REC-51 and the prediction rows) | needs hooks on `0x004AFF60`, `0x00464810`, `0x004647D0`, `0x00463390` in `record_frames.py` | those hooks (addresses in HANDOFF REC-51) |
| G7 (R-* hooks), G8 (DRLG hooks) | new hooks per REC entry; REC-95 needs two clients | the hooks; a two-client TCP/IP run under Wine is unproved |
| G9 later acts | quest-state saves need `--quests` slot bits per quest and long scripted play | per-quest `d2s-tool` set-ups + `poke` |
| G10 rest (REC-212, REC-228, REC-205, REC-221–225, REC-201) | pixel checks need the cels drawn (not eye reads); timing points need hooks | `facts_render.py` captures of the menus (`record_frames.py` in the front end) |
| G11 audio | `-ns` and no sound device under Wine | an ALSA dummy device or a sound-call hook |
