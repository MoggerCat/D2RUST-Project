# c2-client-audio — coverage session note

Branch `claude/c2-client-audio`. Scope: `specs/client/*.md`, `specs/audio/*.md`.

## Counts (unclaimed, non-exempt rules)

| | before | after |
|---|---|---|
| client + audio specs | 131 | about 90 (after) |

(Before = 131 unclaimed leaves at session start; the final figure is in the
last commit message / coordinator gate.)

## What was done

- Claims added to existing tests: triggers-2 §16 (text, rows 1–4), §14 edge
  case 1; stat-lists §1 r3, §4 r1/r3/r4; bridge edge cases 1–3; msg-units
  §1.2 r7; msg-ui §11 r3.
- New code: `crates/d2-client/src/rules/unit_visibility.rs` (model.md §13
  r1–r5: screen point, COF box via `cof_box_visible`, cel box test) with
  tests. No-op handler tests in `bridge/tests_c2cli.rs` (msg-units §8 r11,
  model §7 r12).
- No code fixes to existing behaviour were needed.
- The local cloud container needed `apt-get update` before the system
  libraries (wayland, alsa, udev, xkbcommon) would install; the d2-client
  build is slow (Bevy), so new tests were not all run before the deadline —
  see "Unverified".

## Exempt candidates

```
specs/audio/triggers-2.md	§21 row1	inputs table: owner pointers only (rows 1–18)
specs/audio/triggers.md	§1 r10	definition of type and class terms
specs/audio/triggers.md	§1 r13	pointer to triggers-2 §18–§21
specs/client/msg-units.md	§1.2 r8	pointer to model.md §14 rule 4
specs/client/stat-lists.md	§4 r2	pointer to client/msg-skills.md open question 2
specs/client/ui.md	§a1-why-not-bevy-ui	rationale narration
specs/audio/sound-table-2.md	§16 r5	d2rs modelling note, no behaviour of its own
specs/client/audio.md	§b-original-behavior-to-reproduce-owners	pointers to other specs
```

(§21 rows 2–18 are the same reason as row 1.)

## Rules left (need code or game data)

- stat-lists §1 r1/r4 (client callback 0x004609F0), §2 r1–r4 (item list,
  equip, level change, detach), §3 r6 (StateFx emission).
- model §17 r1–r6 (UI-requested model writes through the bridge,
  bridge.md §10 r10), §12 r4, §18 r1/r3.
- msg-ui §8 r4 (socket dialog), §16 r6; msg-skills §7 r4; msg-stats-items
  §2 r6 (belt ready bytes); msg-units §3 r3, §5 r4.
- assets §a4 r1 (stall metric), render-pipeline §a7 r1, triggers-2 §14 r3
  (call-site table), triggers.md §1 r11/r12.

## Unverified

The new tests (`unit_visibility`, `tests_c2cli`) were compiled and run: pass; clippy -D warnings clean for d2-client.
written against the spec but compiled/run only if the final commit message
says so.
