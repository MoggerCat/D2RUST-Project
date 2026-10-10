# q-fix-d5-draws — hand-back (2026-10-09)

Task: ledger first divergence D5 of `draws-town-arrival-ama` (row 113,
the shadow of a critter vs a torch).

## Done

- Cause: the client-made critters (set C monsters, `client/model.md` §5 r6)
  were never drawn: no `ck` shadow or cel row at all in the d2rs list.
- The critters now draw like any monster (REC-1395, PROVISIONAL):
  - `critters::place_critter` links the new unit into its room's list;
    `remove_client_unit` leaves it.
  - `ClientWorld::view_unit` / `view_units` / `view_key`: set S and set C
    number GUIDs apart (set C from 2, server monsters from 1), so a view
    key is the GUID | 0x8000_0000 for a set C monster (an unkeyed first try
    removed the server NPCs 1:2..1:4 from the room list). The model's set C
    keys are unchanged.
  - The near-room list, the draw order, the art loader and the frame builder
    read units through `view_unit(s)`.
- Result at tick 73 (`facts-compare`): first difference **row 113 → row 111**;
  rows 1–110 equal (the NPC shadows `wa`, `rc` and the objects included).
- Test: `critters::tests::the_view_tells_a_set_c_monster_from_a_set_s_one_of_the_same_guid`.
  Spec note: `specs/client/model.md` §5 r6 "Drawing".

## Open (not equal yet; the check stays DIVERGED)

1. **Chicken position and cel frame** (the new first difference, row 111):
   1.14d draws the walk frame 40 at (80, 416) and the neutral frame 32 at
   (48, 516) at tick 73; d2rs draws frame 0 at the creation cell. This is
   REC-742 (the C monster's path and walk end, the 0xAC set-up not run on
   set C): needs a per-tick 1.14d track of the chickens. Queued as a PC 1
   item in `docs/handoff/pc1-data.md` Step 4 ("[q-fix-d5-draws]"). The
   Wine recorder was not set up in this session (`--no-wine`).
2. Row 172: Warriv (`wa`, frame 47) CelDraw x 304 vs 303 (NPC pose; not
   looked at, the NPC owner's area).
3. The `unit` rows shift with the chicken rows; recheck after 1.

## Repro

```sh
export D2_GAME_DIR=/home/user/game WGPU_BACKEND=vulkan
cargo build --release -p d2-client -p d2s-tool
target/release/d2s-tool new --name ScnAma --class ama --expansion -o /tmp/d5/ScnAma.d2s
Xvfb :98 -screen 0 1024x768x24 & export DISPLAY=:98
target/release/d2-client play --save /tmp/d5/ScnAma.d2s --seed 1234 --difficulty normal \
  --dump-draws /tmp/d5/dump --at-tick 73
target/release/d2-client facts-compare facts/render/scenes/a1-town-arrival-ama /tmp/d5/dump --ignore tick
```

(Needs `apt` packages xvfb, x11-utils, mesa-vulkan-drivers; the committed
scene is the 1.14d side, no Wine needed for this compare.)

Ledger rows: `docs/handoff/ledger/q-fix-d5-draws.tsv` (one new row).
