# q-fix-skills-bda: Barbarian / Druid / Assassin skill exactness (2026-10-09/10)

Branch `claude/q-fix-skills-bda` (merged: integ-r7, specs-staging-7, q-fix-ass-traps,
q-fix-skills-4cls). Input: q-chk-skills-bda (80 checks, 30 DIVERGED). Methods M01, M22,
M23, M25. REC ids used: 1651, 1652 (both settled by 1.14d reads); 1650 not used.
Owner row taken over from q-diff-skills-1 (`tools/coord/owners.tsv`).

## Result

`python3 tools/scenario-diff/suite.py --area bar,dru,ass --no-playthrough --orig-cache`,
state channel, compared with `state_diff.py --ignore q` (see Open 1):
**82 checks, 9 DIVERGED, 73 PARTIAL, 0 MATCH, 95.6 % of ticks equal**
(start: 80 checks, 30 DIVERGED, 82.4 %). The 1.14d side of every bar/dru/ass check
is now in `traces/orig-cache/` (state; dru-raven also rng).

## Fixes (first divergence → cause → fix)

| Checks moved to PARTIAL | Cause | Fix |
|---|---|---|
| ass-fire-blast, ass-shock-field | lob missiles (flags 0x420) read `target_distance` from the Pending stub (0): one-frame life, an extra game-seed draw | `0x006417F0` on the path provider (`wiring/action/missiles.rs`; staging made the same fix) |
| dru-arctic-blast, ass-blade-fury | the Inferno / Blade Fury expiry callbacks (`0x005C8BF0` / `0x005D69B0`: state off, flags \|= 0x40) were never queued: the channel never ended | queued and run in `lists_expired` (`wiring/action/units.rs`) |
| dru-armageddon | `BodyWorld::point_collides` was Pending's "always collides": all five tries failed (8 extra unit-seed draws) | `0x0064CB30` on the rooms (`skill_rooms.rs`) |
| dru-summon-spirit-wolf, dru-summon-fenris, dru-spirit-of-barbs, dru-heart-of-wolverine | d2-server's provisional hireling stand-in also drove linked pets and idled them every 10 frames | linked pets left to their own AI (`hireling_drive.rs`; 4cls did the same) |
| dru-oak-sage | the summon's aura `sumskill` select (`bodies.md` §6.5 step 6) fell to a no-op seam; `0x00554DE0` did not resolve the minion owner | aura summons get a skill list and `use.md` §7's assignment (`summon.rs`); `allied` resolves `0x0058F0D0` (`skill_use.rs`) |
| dru-raven | summon spawn step 8 (`0x005B0E00`, AI install again) was a no-op seam: the Raven init's draw ran once, 1.14d twice (rng trace, site `0x005ECB9C`) | `set_ai_state` installs the AI on the wiring (`skill_use.rs`) |
| dru-raven (later frame) | mode-end neutral request target | `0x005A8030` read: record target = path target unit (`0x00553540`), point (0, 0) (`monsters/ai/mod.rs`, ai.md §1.4; REC-1651 settled) |
| ass-shadow-warrior (walk) | velocity base from monstats (0) | `0x00623F50` → `0x00621360` read: base of the **draw identity** (`WalkUnits::velocity_base`, pathing.md §8.1; REC-1652 settled) |
| dru-twister, dru-tornado, dru-firestorm | charged-bolt path's first step | `0x00650090` read: the point snap applies to every path type but 4 (REC-1391, done on 4cls/staging; my read sent to them) |
| ass-psychic-hammer, ass-dragon-flight | client cast start | q-fix-skills-4cls (merged) |

## Still DIVERGED (first divergence)

| Check | First divergence | Note / owner |
|---|---|---|
| bar-battle-command | f28 player st 133196 vs 128550 | the server stat callback `0x0055B800` (`skills/levels.md` §7) is not wired: `StatHost::skill_stat_changed` has no implementation, so +skills (stat 127) never refreshes the passives (Increased Stamina). `skills::stat_cb` exists but nothing calls it; needs a queue on `ActionHooks` drained where a `UseView` exists (after skill events and expiry). Mine, next. |
| bar-whirlwind | f20 sp | q-fix-class-rows |
| dru-shock-wave | f20 m 5 vs 10 | 4cls cast-start area (client) |
| dru-vines, dru-plague-poppy, dru-cycle-of-life | f67 tx 5143 vs 5144 (was f31) | the pet's walk target late in the run; not looked at |
| ass-shadow-warrior, ass-shadow-master | f28 item x (4 vs 0, 2 vs 0) | equipment items made by the 4cls merge carry a different `x` (item place) than 1.14d; Shadow Master also starts mode 7 at f48 in 1.14d (PC 1 question) |
| ass-blade-sentinel | f28 game seed | q-fix-ass-traps |

## Open

1. **`q` field**: on staging the d2rs state dump writes `"q":[]`; the coordinator says PC 1
   fixed the recorder's 1.14d `q` on `claude/local-pc1-late`. That branch conflicts in eight
   files outside this area, so it is not merged here; compare with `--ignore q` until staging
   has it, then re-record the caches' state if `q` is still wrong.
2. **d2-client tests**: the lib tests pass (2286) on 6768b1e1; the integration-test binaries
   did not fit the disk (link failures from lack of space, not test failures).
3. PC 1 question left in `docs/handoff/pc1-data.md` Step 4 (`[q-fix-skills-bda]`): the
   Shadow Master's first think (mode 7 at f48). The type-10, REC-1651 and REC-1652 ones were
   answered by the re/ reads above and removed.

## Repro

```
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/suite.py --area bar,dru,ass --no-playthrough --orig-cache
for d in traces/raw/suite/{bar,dru,ass}-*; do python3 tools/trace-recorder/state_diff.py $d/orig.state.jsonl $d/d2rs.state.jsonl --ignore q --next 0; done
```
