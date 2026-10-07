# Handoff: client S→C handlers for the wired UI and audio — `claude/impl-client-msgs-2`

> Not yet folded into `docs/HANDOFF.md` §1–§4 (only `docs/PLAN.md` is
> edited here); the coordinator folds it.

Cloud implementation session, 2026-10-06, task class: implementation from
specs, medium. Base: `claude/specs-staging` at `d33adcf` (re-fetched at
session start: unchanged). Repo only: no `game/`, no `re/` (M09). Read:
`docs/handoff/wire-client-staging.md`, `specs/client/bridge.md` §6,
`specs/client/bridge-dispatch.tsv`, `specs/client/msg-stats-items.md`,
`specs/audio/triggers.md` §2, §11, `specs/render/lighting.md` §9.2,
`specs/ui/panels.md` §8, §11–§13, `specs/world/waypoints.md` §5.3,
`specs/world/quests.md` §6.3, `specs/world/cube.md` §1,
`specs/sim/stats.md` §4.2.

## 1. Result: no handler implemented

The task allows a handler only for a row whose `owner` in
`specs/client/bridge-dispatch.tsv` is a written spec. On this base all six
requested rows are `TBD`:

| Id | Name | Owner on `d33adcf` |
|---|---|---|
| 0x2C | PlaySound | TBD |
| 0x53 | Darkness | TBD |
| 0x5D | QuestItemState | TBD |
| 0x63 | WaypointMenu | TBD |
| 0x77 | TradeAction | TBD |
| 0x94 | BaseSkillLevels | TBD |

`bridge.md` §6 r2 says a spec takes an id by setting its own path as
owner, and §6 r5 checks this both ways in code
(`bridge::dispatch::check`: an owner other than `TBD` if and only if a
handler is registered, with the same owner). So no handler can go in
without a spec edit first, and owners are assigned only in spec work. Code,
tests, `ui::original::PENDING` and `audio::driver::PENDING` are unchanged.
Nothing was added to the HANDOFF §5 queue (nothing new is unverified).

The character panel totals (`ui/panels.md` §8 r7: value = unit total
`0x00625480`, base = `0x006253B0`) are also not implemented. The client
model holds only layer-0 base stats (`msg-stats-items.md` Outputs, §1
r2). The total reads the player's full array (`sim/stats.md` §4.2: "full
array if extended"), which on the client depends on the items and states
linked into the player's list. No client spec says how that list is
extended on the client: item stat lists from the item stream (0x9C /
0x9D, `msg-stats-items.md` OQ 3), state stat lists, or the 0x21 / 0x22
skill stats. So no spec states how the client gets the totals. Showing
the base as the total would be a guess.

## 2. What each row lacks (for the spec session)

Each row below has its byte layout, and for most rows the client effect,
already written in another spec. What is missing is a client spec that
**takes the id** (sets the TSV owner) and states the client model state
and the handler. A dispatch owner is the spec that owns "what the message
means for the client model" (§6 r1). For 0x2C, 0x5D and 0x77 that meaning
is mostly UI or audio output, not model state, so the spec session also
has to decide how a bridge handler hands outputs to the UI or sound layer.
Today a handler only writes `ClientWorld`. Neither `bridge.md` nor
`model.md` has an event or out-queue for UI or sound requests.

| Id | Already written | Missing (spec to write or extend) |
|---|---|---|
| 0x2C | Layout and the full client event table: `audio/triggers.md` §2 r1–r3 (implemented as `audio::triggers::events::server_event`, waiting for a caller) | Owner. The bridge → sound handoff (event queue, or a model field the driver drains, as above). Event 12's `stsound` (triggers OQ 4). Event 18's greeting record (NPC interaction, not in the model). Events 84–87's overhead text (`client/ui.md`). |
| 0x53 | Layout and setter: `render/lighting.md` §9.2 r2 (only when the message act equals the player's act; fatal cases; eclipse) | Owner (`render/lighting.md` or `client/model.md` §11 taking it). The act-environment record as client model state (`model.md` §1 has no environment field). The player's act is in the model, but the period table, speed and eclipse table inputs need a named data source on the client. |
| 0x5D | Layout: `world/quests.md` §6.3 (u8 chain @1, u8 flags @2, u8 status @3, u16 extra @4). Client effects described in three places: sound `audio/triggers.md` §11 (`0x004A2CB0`; implemented as `audio::triggers::ui::ui_action`), eclipse `render/lighting.md` §9.2 r3, quest state `world/quests.md` | Owner, and the full `0x004A2CB0` dispatch in one place (triggers OQ 9: "the client treats it as UI/sound actions"; field names differ: triggers says code / flags / value i16@4, quests says chain / flags / status / extra u16). The client quest-state store, if any. |
| 0x63 | Layout: `world/waypoints.md` §5.3. Client: `ui/panels.md` §13 r1 (`0x0049CF90` stores the 16-byte record with the load copy `0x00661030` and calls `SetUIState(0x14, on, jump 1)`) | Owner. Where the stored record lives in the client model. The load-copy rule `0x00661030` (byte order and validation of the 16-byte record; waypoints §2 gives the record layout server side). The tab reachability inputs of §13 r3 (quest checks `0x004B32D0` / `0x0065C310`: client quest state, see 0x5D). |
| 0x77 | Server senders: `world/cube.md` §1 (codes 0x0C, 0x11, 0x15 in single player) | Owner. The client handler `0x0045E800` itself: the code → action table (which code opens the stash `0x00489E00`, the cube `0x0048A460`, closes, refuses; `ui/panels.md` §11 r1 and §12 r1 name the openers but not the codes). The TSV marks the row `out` (trade), but single player sends it too. |
| 0x94 | Only the size formula in `sim/server-messages.tsv` (`u8@1*3+6; min=9`) and the handler address `0x0045DD60` | Everything: field layout, the client skill list in the model (`model.md` §1 has none; `msg-stats-items.md` OQ 5 leaves 0x21, 0x22, 0x23, 0x94 unowned), and the skill-icon inputs of the skill tree (`ui/panels.md` §10 r3; the icon file prefix `CC` is open, `wire-client-staging` §2). |

Suggested owners (the spec session decides): 0x2C → `audio/triggers.md`
or a new `client/msg-ui-audio.md` that also takes 0x5D and 0x77. 0x53 →
`render/lighting.md` §9.2. 0x63 → `ui/panels.md` §13 or `world/waypoints.md`
§5.3. 0x94 → a new `client/msg-skills.md` with 0x21–0x23. Once a spec
sets the owner, the code side is small. Register the handler in
`bridge::msg::HANDLERS`. Hand its output to `ui::original` (0x63, 0x77:
`UiStates::set` through the `after_event` path) or `audio::driver` (0x2C,
0x5D: a new feed beside the UI sounds). Then remove the matching
`PENDING` rows.

## 3. Gate

No code changed (docs only). Run on this branch:

- `cargo test -p d2-client -p d2-proto`: every target passes, 0 failed
  (d2-client lib 822 passed, 8 ignored).
- `cargo clippy -p d2-client -p d2-proto --all-targets -- -D warnings`:
  clean. `cargo fmt --all --check`: clean.
- `python3 tools/coverage.py --check`: 8,087 claims, 0 errors.
  `python3 tools/spec_index.py --check`: ok. `python3 tools/methods.py
  check`: 21 methods OK.

## 4. Next steps

1. Spec session (local, needs `re/`): take the six ids as in §2,
   starting with 0x63 and 0x2C (their client effects are already written
   and partly implemented), then 0x77, 0x5D, 0x53, 0x94. Add a bridge →
   UI / sound output channel to `bridge.md` (§5 or a new section).
2. Then rerun this task on the new base.
3. Character totals: a client stat-list spec (how the client extends the
   local player's list with item, state and skill lists), then the
   character panel values and colors (`ui/panels.md` §8 r7–r9).
