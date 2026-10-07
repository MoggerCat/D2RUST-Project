# impl-c-client: client bucket-C wiring (2026-10-07)

Branch `claude/impl-c-client`, base `claude/specs-staging-7` @ `0c70873`.
Cloud implementation session; repo only. Everything below is
implemented, unverified (M02).

## 1. What landed (commit 8adeace4)

1. **Automap wiring** (`ui/automap.md` §5 r1, §7, §8 r2, §14):
   `ui::automap::session::AutomapSession`. It opens the act's files at
   act load (§7 r1, `open_files_in` with the `.map` sub-directory
   fallback, §7 text, which is no longer exempt), runs the per-frame
   reveal on the world view's near rooms after the draw, toggles UI
   state 0x0A on `ToggleAutomap`, and saves and writes at teardown.
   `WorldViewState::automap` holds it (`None` until the app supplies
   tables).
2. **World-click pipeline** (`ui/controls.md` §6 r1–r10): pure
   dispatcher `controls::click` (kinds, latch, filter, per-kind steps,
   decision order r8, senders r9, walk clamp, skill codes, re-pick, held
   repeat) behind the `ClickWorld` trait; `bridge::click` answers it from
   the model and applies codes as C→S bytes (0x13 through
   `objects::interact::send`); `world_view::ui_bind::world_clicks` is fed
   from `UiFrame::unhandled` each frame. A left click on the ground now
   walks (C→S 0x01 / 0x03).
3. **Mode machines** (`client/model.md` §8 r1–r6): `bridge::modes`
   covers the player code table (modes, town walk/neutral, +0xB0
   `hit_class`, path stop, the per-code position checks, fatal 0x432);
   object code 3 (mode := r1, object light, mode sound inside the
   change; other codes fatal 0x39C); item code 2; the monster mode
   inverted from the server mode table. The "change no model field" note
   in `update.rs` now only covers the per-type updates (OQ1/OQ2).
4. **Audio feed**: `ObjectSound` (mode / request / player event) and
   `ShrineSound` outputs become `SoundRequest`s (`present::audio_request`).
   The driver runs `object_mode` with per-unit sound fields, dropped
   when the unit goes (`model.md` §18 r1–r2). `ObjFx::Light` is applied
   to the model's light list in the update pass.
5. **Passive-state stat lists** (`msg-skills.md` §2 r4,
   `stat-lists.md` §1 r2): `bridge::passive`, the refresh over a d2-sim
   `SkillUnits` view with `eval_skill`. List ops now record `SkillFx`
   (state on/off, refresh) instead of failing with `PassiveState`.
   0x93 rule 4 runs `refresh_all`. `ClientWorld::total` includes the
   attached state lists. The app sets
   `Bridge::set_skill_tables(SkillTables::from_bin)`.
6. **0xAC monster set-up** (`msg-units.md` §1.2 r6.1, r6.6, r6.8): base
   stats with classic scaling, flags `isSel` → flag_2 and `isAtt` →
   flag_4 (the first writer of flag_4), and `Skill1–8` at level + the
   `difficultylevels` `MonsterSkillBonus`, with mode bytes. Rows come
   from `single_player::client_unit_rows` (`MonsterClass::setup`).

Gate: `CARGO_INCREMENTAL=0 sh tools/gate.sh` → **GATE: PASS** on
8adeace4.

## 2. Coverage (units covered / claimable)

model 75→80/103, msg-units 56→57/62, stat-lists 4→5/18, automap 62/63→63/64
(§7 text un-exempted), controls 0→10/86; total 7650→7668.

## 3. Changed test expectations

- `audio::driver::tests::the_model_sound_world`: `PENDING.len() >= 7`
  → `>= 6`. The object-mode-sound feed is wired, so its PENDING row is
  gone.
- `bridge::msg::tests_units_more::shrine_on_mode_overlays`: 0x0E code
  3 now outputs `ObjectSound::Mode` (mode 1) before the `ShrineFx`,
  because the object mode change makes its sound call inside the
  change (triggers-2 §20 r3).
- `bridge::objects::tests` (ClientFn 0): `ObjSound::Mode` has the new
  field `local_dist`.
- `bridge::skills::tests::a_passive_skill_*`: a passive assign used to
  return `Err(PassiveState)`. It now completes (base set) and owes
  `[StateOn(5), Refresh(1), Refresh(1)]`; a remove owes
  `[StateOff, Refresh]`.
- `client_world_holds_only_stated_fields`: names `hit_class`.

## 4. PROVISIONAL (M22)

| Code | Choice | Settled by |
|---|---|---|
| `bridge/modes.rs` `monster_mode` | monster mode = first mode of the server mode table whose code matches (12/13 → S1) | REC-51 |
| `bridge/modes.rs` codes 0/1 | path helpers succeed (no client path) | REC-51 |
| `bridge/modes.rs` code 0x12 | inventory/COF test read false (mode unchanged) | REC-51 |
| `bridge/update.rs` | per-type updates: no mode steps, no seed draws, no walk prediction (OQ1/OQ2) | REC-51 |
| `bridge/objects/mod.rs` `NO_LOCAL_DISTANCE` | no local player → far | REC-51 |
| `bridge/click.rs` `can_act` | mode 18 → cannot act | controls-0001 |
| `bridge/click.rs` `screen_to_world` | inverse unit draw + static projection | controls-0001 |
| `bridge/click.rs` `walk_path` | path end = clamped target | controls-0001 |
| `bridge/click.rs` `apply` | click mode request not applied locally | REC-51 |
| `world_view/present.rs` `skill_y_limit` | `0x00454970()` = H − 40 | controls-0001 |

Readings: `controls::click` release kinds run their step after the
filter; `automap::session::open_files_in` treats "cannot be opened" as
"the sub-directory is not a directory".

## 5. Left

1. **Overlay list on ClientUnit** (`render/overlay.md`) and **skill
   descriptions** (`skills/descriptions.md`): handed to two subagents,
   but neither committed before the deadline. Their uncommitted
   worktrees are local only. The work needs a new session.
2. **Stat lists §3 r6** (state on / hooks / off model writes, the
   `StateFx` emission, setfunc/remfunc bodies), which r6.9 (monster
   re-init with seed draws), r6.10 (blood spray) and r6.11 (skill item
   test) depend on. `StateRow` needs the setfunc/remfunc/flag columns.
3. **0xAC r6.5 / r6.9 seed draws** (frame draw with range +0x48,
   direction draw). They need the mode's animation frame count and
   `0x0046C140` in the client tables. Without them the client unit
   seed lags 1.14d (RNG order, high priority).
4. **0x15 r4.5 nearest free point**: needs the client collision maps.
5. **Click inputs not in the model**: hover `0x00467A10`, the skills
   flag columns / `range`, use state, cursor state, hireling, the
   searches of r11, and the Stand Still / Run modifiers. The dispatcher
   works with them; the adapter answers none, so clicks are ground
   clicks.
6. **Automap still open**: no app construction of `AutomapSession`
   (picker from `automap.txt` rows, leveldefs `Layer` / `LevelType`,
   save dir and name); no draw sink for `AutomapDraw`; §5 r4 countdown
   hook on 0x15; DRLG callbacks +0x454 / +0x488; unit drawn/automap flags
   are not persisted per unit.
7. **ObjFx consumers** other than `Light` (gfx refresh/load, overlays,
   skill start): no effects layer.
8. **Local run queue**: none new (all checks are unit tests).
