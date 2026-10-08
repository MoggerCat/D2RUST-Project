# q-fix-idle-rooms: the inactive room store in play (B2)

Branch `claude/q-fix-idle-rooms` (staging-7 and `claude/q-smoke-travel`
merged in: the B2 repro lives in that branch's `smoke_travel.rs`).
REC-287.

## The break (q-smoke-travel B2)

Tick step 9 frees a room no client has been near for a while
(`drlg/rooms.md` §8). Its units went through `compress` (`units.md`
§3.3), which did nothing because the play host never turned the
inactive store on, and the store's restore seams were no-ops. The units
were only unlinked by the freed room, so after a short game an
unvisited town came back with its tiles but without its NPCs or its
waypoint.

## Links connected

1. `d2-sim` `wiring/action/inactive.rs`, the §3.3 facts from the game's
   tables (seam only when a table is absent):
   - `S`: leveldefs `SaveMonsters` (object state tables) or unit flag
     0x2000000;
   - monster rule 8: monstats2 `restore` via monstats `MonStatsEx`;
   - objects: `Restore`, `RestoreVirgins`, unit byte +0x78 bit 0x2;
   - the monster record's type-flag bits, level, name seed, umods and
     superunique index from the lent world's monster data;
   - the "other" record's object fields (§3.4 r3): the mode-2 switch
     (`CycleAnim1` 0, `Mode2` ≠ 0), a shrine's pending event 5 time,
     the GUID of classes 59 / 60 / 100 or byte +0x78, byte +4, +0xB8.
2. The §3.4 r4 restore runs in d2-sim (the `Pending::restore_monster` /
   `restore_other` seams are removed: a host seam cannot allocate):
   monsters spawned again with the stored GUID in mode 1 / 12 (the
   allocation runs the type init on the lent world), unique / minion
   records get their saved umods and flags; "other" records are new
   units with unit flags 0x3000000 (objects through `create_object`, so
   their init runs, then byte +4, +0x78, +0xB8, shrine event 5, well
   regrowth); kept units (flag-ex 0x100: pets, player bodies, portals)
   are placed again, flag 0x10.
3. An item is freed only when its record was stored (no item writer is
   wired in any host yet: the item stays as before).
4. `d2-client` `app/single_player.rs`: `enable_inactive_store()` in the
   play host; the synthetic build's units get unit flags 0x3000000 (the
   live towns' preset units have them, `population.md` §11.1); the
   synthetic waypoint, chest, stash and tome rows have `Restore` 1.

## Tests

- `smoke_travel::a_town_keeps_its_npcs_and_waypoint_after_a_long_game`
  (B2 repro): un-ignored, passes.
- `smoke_travel::the_act_i_town_comes_back_with_its_npcs_and_waypoint`
  (new): leave the Rogue Encampment, run until the server frees every
  town room, check every NPC and the waypoint are in the store, come
  back: each NPC is there with its GUID at its stored place, the
  waypoint at its place in its mode, the client sees them, the waypoint
  menu opens, Warriv still takes the player east.
- `d2-sim` `wiring/action/tests/inactive.rs`: a stored monster comes
  back with its GUID, mode 1 and place; a stored object comes back as a
  new unit with flags 0x3000000; a kept portal is placed again with
  flag 0x10.

## PROVISIONAL (REC-287)

Restored monster's allied flag from the record's alignment bit (the
restore's region / node-8 work is not run); the umod run and the stored
life are not re-applied; a pet's owner re-link is not run; the well
refill schedule; ground items keep the old behaviour (no item record
writer). Details in `docs/HANDOFF.md` §7 REC-287.

## Left

- Item records (`items/bitstream.md` save form) for ground items in a
  freed room: needs the economy's item writer reachable from the
  action wiring's compress (`Pending::item_record` / `restore_item`).
- The smoke tests' "fresh game per act" stand-ins (B2) could now share
  one game; not changed here.

## Local check

```
cargo nextest run -p d2-client --test smoke_travel
cargo nextest run -p d2-sim -E 'test(inactive)'
```

In `play` (synthetic or live): walk from the Rogue Encampment into the
Cold Plains and stay away a couple of minutes, then walk back: every
NPC and the waypoint are there once, the NPCs talk, the waypoint opens.
With game files also see the §5 queue entry (no duplicates from the
town presets).
