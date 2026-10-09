# Handoff: q-fix-input-lock (`claude/q-fix-input-lock`)

Queue row `q-fix-pt-cast-input-lock` (the #1 playability blocker): after
one attack or cast the client sent no more input. REC block 1000–1009
(1000–1002 used).

## Cause

The server sends a player's own client nothing when the player's mode
ends (`sim/pathing.md` §10 r2: the NU row skips the own client), and the
1.14d client ends the mode itself in its player update `0x00463390`
(`skills/sequences.md` §3, the client mode-18 machine: advance, then the
neutral end when `0x006217C0` says the animation is complete). d2rs had
no client player step: the server's skill echo (S→C 0x4D / 0x4C, still
sent to the own client, PROVISIONAL REC-95) put the local player in mode
7 / 10 / 14 for good, and `bridge::click::can_act` (modes 7–12 refused)
dropped every later click.

## Fixed

- `bridge/player_anim.rs` (new): the player mode set restarts the
  animation (frame := frame bonus · 256, count := AnimData frames · 256,
  speed := the rate `0x00623F50`; the same mode restarts nothing), and the
  player update ends GH 4, A1 7, A2 8, BL 9, SC 10, TH 11, KK 12, S1–S4
  13–16 in neutral (1, 5 in town) when complete. Run from the update pass
  before each player's drain (`bridge/update.rs`); every mode set of the
  player machine goes through it (`bridge/modes.rs`).
- The animation lookup is a host input (`ModelInputs::player_anims`,
  `Bridge::set_player_anims`): `app::anim_names::ClientPlayerAnims` reads
  the user's `AnimData.d2` by the COF name the server's composer uses
  (`anim_key`), the weapon class from the items the model shows on body
  locations 4 / 5 (`app::weapons::cof_class_of`, the server's resolver).
  Installed by `play` and `state-dump`.
- A skill click runs the local mode request at the click
  (`bridge/click.rs` `skill_request`: codes → 0x15 / 0x16, record {side's
  skill, owner, a, b}), so the cast mode starts at frame 0 on the click as
  in 1.14d (local player rules 1–4); the server echo then finds the same
  mode and restarts nothing.

PROVISIONAL (spec lines in `skills/sequences.md` §3, entries in
`docs/HANDOFF.md` §7): REC-1000 (the ending modes run mode 18's row on
their AnimData; `0x00711E00` rows 4, 7–16 unread), REC-1001 (rate inputs:
no used skill, no item rate stats), REC-1002 (gate `0x00480BA0` not run).

## Checks

- `python3 tools/playthrough/playthrough.py traces/playthrough/classes.play --build --class all --difficulty normal --only cast-then-walk`:
  1/1 on all 7 classes (before: stuck, `player moved=0`, ama and sor
  re-run on the stashed tree).
- `main-skill-kill` (five right clicks 30 frames apart): every class now
  sends all five casts (C→S 0x0D on the Quill Rat, the hover pick; frames
  39, 69, 99, 129, 159) and the server player casts each time (mode 7 /
  10 at 40, 70, 100, 130, 160, neutral between). The milestone itself
  failed on the branch's first commit because the rat came back from death
  with hp 0 (`q-fix-p4-death-cleanup`); after merging staging `0a297048`
  it is reached for ama, sor, nec, dru. Still stuck (rat untouched, hp 256,
  every cast sent): pal Blessed Hammer (`q-fix-pt-blessed-hammer`), bar
  Whirlwind (`q-fix-pt-whirlwind`), ass Lightning Sentry (the sentry at
  the click point does not reach the rat; `trap-kills` is reached).
- Gate on the merged tree: d2-client nextest 2,532 passed; before the
  merge d2-sim / d2-server / d2-client / test-fixtures 7,691 of 7,692 (the
  one failure, `d2-server::prop_unified_items item_moves_keep_one_place`,
  `seed = 0`, is deterministic and outside this diff, which touches no
  d2-server or d2-sim file).
- 1.14d: `python3 tools/scenario-diff/scenario_diff.py traces/checks/sor-frost-nova-twice.check`
  (new; two Frost Nova clicks at frames 20 and 60): PARTIAL, "no
  difference in what was compared" over 100 frames; both sides cast at 20
  and 60 and are neutral at 33 and 73.
- Tests: `bridge::player_anim::tests` (2), `bridge::click::tests::a_cast_ends_in_neutral_and_the_next_right_click_casts_again`
  (click → mode 10 locally, a second click refused, three update passes
  end it, the next click casts again).

## Left

- The server still echoes the skill message to the own client
  (`d2-sim` `wiring/path/walk.rs`, REC-95); 1.14d skips it unless E flags
  bit 0x4 (`sim/pathing.md` §10 r2). The client's missiles of its own
  casts may depend on that echo, so it was not changed here.
- Mode 18 (sequence skills) keeps no client frame count, so a sequence
  skill's client mode does not end yet (`q-fix-pt-whirlwind` reports
  Whirlwind never enters a mode at all).
- The client end tick against 1.14d's client unit is unmeasured (the
  state channel reads server units): REC-1000.

## Local check

`D2_GAME_DIR=<install> cargo run --release -p d2-client -- play` with any
character: in the Blood Moor right-click a skill, then right-click again
after the cast animation: the second cast happens; left-click the ground
after a cast: the player walks.
