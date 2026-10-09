# q-chk-skills-bda: barbarian, druid, assassin skill checks (2026-10-09)

Branch `claude/q-chk-skills-bda` (staging + integ-r3 + integ-r4). Methods M22, M23, M25.

## Done

- Reran the 16 existing bar/dru/ass checks under Wine (the brief says 22; 16 exist): same
  verdicts as before (bar-whirlwind sp 304 vs 256, dru-tornado, dru-volcano, ass-fire-blast,
  ass-shadow-master DIVERGED; the rest PARTIAL). `ass-burst-of-speed` had ERRORed once, a race
  building the `blood-moor-empty` variant while the excel view was empty; it is PARTIAL on rerun.
- `tools/scenario-diff/gen_skill_checks.py` writes one check per remaining non-passive skill
  (the existing pattern: level-30 save, every skill 20, right skill, Blood Moor, a cow for skills with reach)
  and one `<class>-passives` check per class (all passives/synergies at 20, left-click attack on a cow).
  64 new checks (80 total). Check-gen (q-tool-check-gen) had not landed; adopt it when it does.
- Suite result (state channel only): 80 checks, **30 DIVERGED, 50 PARTIAL, 0 MATCH**, 82.4% of ticks equal.
  Full table: `docs/handoff/q-chk-skills-bda-suite.md`.
- `docs/handoff/ledger/q-chk-skills-bda.tsv`: 90 `skill.{bar,dru,ass}.*` rows (validates: 0 format errors).
  Same areas as `ledger/skills.tsv`; it replaces those rows (the 60 passive/PARTIAL ones become NO-CHECK with a
  check, 30 DIVERGED).

## First divergences (routed)

| Check | First divergence | Owner |
|---|---|---|
| ass-blade-fury | frame 40 player 0:1 class 6, field fr: 1.14d 7168 vs d2rs 7424 | q-diff-skills-1/2 |
| ass-blade-sentinel | frame 27 monster 1:9 class 413, field m: 1.14d 8 vs d2rs 2 | q-fix-ass-traps |
| ass-charged-bolt-sentry | frame 52 monster 1:9 class 411, field m: 1.14d 1 vs d2rs 14 | q-fix-ass-traps |
| ass-death-sentry | frame 27 monster 1:9 class 416, field m: 1.14d 8 vs d2rs 2 | q-fix-ass-traps |
| ass-dragon-flight | frame 20 player 0:1 class 6, field tx: 1.14d 0 vs d2rs 5151 | q-diff-skills-1/2 |
| ass-fire-blast | frame 28 game, field seed: 1.14d [3184650063, 1412234640] vs d2rs [367684955, 1328293240] | q-diff-skills-1/2 |
| ass-inferno-sentry | frame 52 monster 1:9 class 415, field m: 1.14d 1 vs d2rs 14 | q-fix-ass-traps |
| ass-psychic-hammer | frame 20 player 0:1 class 6, field tx: 1.14d 0 vs d2rs 5151 | q-diff-skills-1/2 |
| ass-shadow-master | frame 48 monster 1:8 class 418, field m: 1.14d 7 vs d2rs 1 | q-diff-skills-1/2 |
| ass-shadow-warrior | frame 48 monster 1:8 class 417, field m: 1.14d 2 vs d2rs 1 | q-diff-skills-1/2 |
| ass-shock-field | frame 28 game, field seed: 1.14d [204979522, 1679869131] vs d2rs [3798855888, 1490448036] | q-diff-skills-1/2 |
| ass-wake-of-fire-sentry | frame 52 monster 1:9 class 410, field m: 1.14d 1 vs d2rs 9 | q-fix-ass-traps |
| bar-battle-command | frame 28 player 0:1 class 4, field st: 1.14d 133196 vs d2rs 128550 | q-diff-skills-1/2 |
| bar-whirlwind | frame 20 player 0:1 class 4, field sp: 1.14d 304 vs d2rs 256 | q-fix-class-rows |
| dru-arctic-blast | frame 39 game, field seed: 1.14d [402943041, 1392284648] vs d2rs [4019089901, 168064467] | q-diff-skills-1/2 |
| dru-armageddon | frame 37 game, field seed: 1.14d [3385903807, 408973908] vs d2rs [980535581, 1085454190] | q-diff-skills-1/2 |
| dru-cycle-of-life | frame 31 monster 1:8 class 426, field tx: 1.14d 0 vs d2rs 5142 | q-diff-skills-1/2 |
| dru-firestorm | frame 30 missile 3:2 class 458, field xf: 1.14d 32768 vs d2rs 25052 | q-diff-skills-1/2 |
| dru-heart-of-wolverine | frame 58 monster 1:8 class 423, field m: 1.14d 2 vs d2rs 1 | q-diff-skills-1/2 |
| dru-oak-sage | frame 31 player 0:1 class 5, field hp: 1.14d 56736 vs d2rs 25216 | q-diff-skills-1/2 |
| dru-plague-poppy | frame 31 monster 1:8 class 425, field tx: 1.14d 0 vs d2rs 5142 | q-diff-skills-1/2 |
| dru-raven | frame 29 monster 1:8 class 419, field s: 1.14d [1601510352, 1471677176] vs d2rs [3528420285, 1166992223] | q-diff-skills-1/2 |
| dru-shock-wave | frame 20 player 0:1 class 5, field m: 1.14d 5 vs d2rs 10 | q-diff-skills-1/2 |
| dru-spirit-of-barbs | frame 58 monster 1:8 class 422, field m: 1.14d 2 vs d2rs 1 | q-diff-skills-1/2 |
| dru-summon-fenris | frame 58 monster 1:8 class 421, field m: 1.14d 2 vs d2rs 1 | q-diff-skills-1/2 |
| dru-summon-spirit-wolf | frame 58 monster 1:8 class 420, field m: 1.14d 2 vs d2rs 1 | q-diff-skills-1/2 |
| dru-tornado | frame 30 missile 3:1 class 478, field xf: 1.14d 32768 vs d2rs 57170 | q-diff-skills-1/2 |
| dru-twister | frame 30 missile 3:1 class 477, field xf: 1.14d 32768 vs d2rs 23123 | q-diff-skills-1/2 |
| dru-vines | frame 31 monster 1:8 class 427, field tx: 1.14d 0 vs d2rs 5142 | q-diff-skills-1/2 |
| dru-volcano | frame 34 missile 3:1 class 479, field s: 1.14d [2238081736, 1028392181] vs d2rs [3365183618, 277] | q-diff-skills-1/2 |

Clusters: summons/pets (`m` mode, `tx`) in the druid and assassin summon checks; missile
`xf`/`s` in tornado, twister, firestorm, volcano; game `seed` (an extra RNG draw) in fire-blast, shock-field,
arctic-blast, armageddon; the sentries' monster `m` at 51–52; dragon-flight / psychic-hammer / shock-wave
at frame 20 (cast start). Routing used `tools/coord/owners.tsv` (skills: q-diff-skills-1/2; class rows:
q-fix-class-rows; traps: q-fix-ass-traps). No unowned rows.

## Open

- PARTIAL checks compare only the state channel; draws/rng/packets are not added (each costs a recording).
- `--right-skill` only: the passives are not exercised in isolation; the three `-passives` checks run the combined save.
- Skills needing targets other than a cow (Find Potion/Find Item need a corpse; Taunt/Howl need a pack) use the cow only.
- Interact pokes (`poke msg 0x13`) were not needed.

## Repro

```
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/gen_skill_checks.py          # idempotent, keeps existing files
python3 tools/scenario-diff/suite.py --area bar,dru,ass --no-playthrough --md out.md --json out.json
```
