# q-move-anims: Leap arc and Whirlwind spin (`claude/q-move-anims`)

> Stitching session, 2026-10-08. Nothing here is verified against 1.14d
> (rule 10). PROVISIONAL points: REC-275 in `docs/HANDOFF.md` §7.

## 1. Links connected

| Link | Was | Now |
|---|---|---|
| start signal | the server sends the client nothing for a skill move | `SkillMotion` (`world_view/skill_motion.rs`) reads the local player's skill mode request (0x15 / 0x16) and the client skills row's `srvdofunc` |
| Leap arc | `ViewSource::unit_offset` always `(0, 0)` in the preview | spec record (`0x004C8726` creator, timed arc, update) stepped each server tick; `ViewFeed::set_motion_offsets` → `ModelFeed` → `unit_offset` |
| Whirlwind spin | no spin | skill's `anim` mode through `UnitArt::pose_mode`; frames loop from frame 3 (`UnitArt::spin`, `spin_frame`) |
| `Bridge::skill_row` | none | additive getter on the client skills table |

Tests: `tests/app_move_anims.rs` (leap draw offsets equal the spec curve
frame by frame; whirl shows mode 7 then stops), unit tests in
`skill_motion.rs`, `unit_rules::tests::a_spinning_unit_loops_its_frames_from_frame_three`.

## 2. PROVISIONAL (REC-275)

Start signal, 0x16 layout, path speed, the `-1 when > 1` reading, the
spin duration: see REC-275.

## 3. What is left

A capture of a leap and a whirl to check heights and frames; the real
start event once the callers of `0x004C8670` are traced.

## 4. The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-move-anims
git checkout claude/q-move-anims
cargo run -p d2-client --release -- play --new barbarian Test
```

Leave camp, bind Leap and Whirlwind on the right button, right-click a
ground point: the Barbarian rises and lands during Leap; during Whirlwind
the spin animation loops. Headless: `cargo nextest run -p d2-client --test app_move_anims`.
