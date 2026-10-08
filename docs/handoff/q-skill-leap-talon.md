# q-skill-leap-talon: Leap, Leap Attack, Dragon Talon / Flight, Whirlwind (`claude/q-skill-leap-talon`)

> Stitching session, 2026-10-08. Nothing here is verified against 1.14d
> (rule 10). PROVISIONAL points: REC-249 in `docs/HANDOFF.md` §8.

## 1. Links connected

No sim code changed; the damage paths were already correct. What was
missing was the synthetic game (all in `tests/app_barbarian.rs` and
`tests/app_barbarian/rig.rs`):

| Path | Missing | Now |
|---|---|---|
| Dragon Talon kick, Dragon Flight kick | rows without the `Kick` flag, no strength / dexterity | rows with `kick`, `Param1/2`; player str/dex 100; tests `dragon_talon_kick_hurts_a_monster`, `dragon_flight_kick_hurts_a_monster` |
| Leap flight and landing | the 8-frame animation ended before the flight (the old test passed after one sub-tile) | 24-frame `BASCHTH` animation; `leap_lands_on_the_aimed_point_and_deals_no_damage` |
| Leap Attack | no row | made-up id 100, `leap_attack_leaps_to_the_monster_and_strikes_it` |

Two traps found: the kicks do nothing in a town room (`apply` ignores a town
defender), and a skill row without `hitshift` 8 swings for zero damage.

## 2. PROVISIONAL (REC-249)

- Leap's landing is knockback only (zeroed record, `bodies-2.md` §4.7): no damage to test; the soft hit is not visible at tick resolution.
- Whirlwind's hits along the path: kept as `use.md` §5.2 reads (hits on arrival). The srvdo 76 body hits while moving, so the clause needs a trace (listed in REC-249).
- Test animation and row ids are made up.

## 3. What is left

Whirlwind hits while moving (trace), the client's leap arc and whirl animation.

## 4. The user's local check

```powershell
git fetch origin claude/q-skill-leap-talon
git checkout claude/q-skill-leap-talon
cargo nextest run -p d2-client --test app_barbarian
```

Ten tests pass. In `play`, Leap and Dragon Talon behave as before; no new visible feature.
