# rc-pkt-misc hand-back (REC-2900)
Checks before -> after (packets channel, with the state channel equal/PARTIAL-gap):
- gen-skill-ama-*: 14 of 30 diverged on "c2s extra 0x0C at frame 20" -> 0; now 28/30 packets MATCH.
- gen-sysc-client-msg-ui-2 / interact-operate-waypoint: frame 4 s2c 0x07 vs 0x15 -> MATCH.
- Ledger part rc-pkt-misc.tsv: 14 amazon skill rows + system.client.msg-ui.2 EQUAL (REC-2055).

## Causes fixed
1. Extra C->S 0x0C: the client use state lacked the item type test (use.md §2 test 5, `0x00643F80`):
   Fire Arrow etc. with no bow worn are refused (state 2), 1.14d sends nothing. Added
   `bridge::use_state::item_type_test` (+ skills `itypes/etypes` columns, `StreamProps::item_is_type`);
   `state-dump` now installs the item tables (headless had none, so the test passed).
2. Frame 4 S->C 0x15 after a `pos` poke: d2rs-own reassign mark removed for `pos` (poke.rs); `hop` keeps it.

## Open (sizes)
- gen-skill-ama-28 / -32 (Decoy, Valkyrie): frame 30 1.14d sends 0xA7 (state 121 skilldelay, no stats) and
  a longer 0xAC (18 vs 14 bytes) plus 7 x 0x9D item records for the summon; d2rs sends none (M).
- talk scenarios (msg-ui 5, 9, 10, 11, 16, 17, 18, interact-talk-akara, items-vendor-akara-buy): now first
  differ at frame 16 s2c 0x9C byte 37, 1.14d 161 vs 160 (store item record; owner of items) (S-M).
- act scenario (msg-ui-20, act-change 1, 3): fixed the state (Cain act change `0x00597310` now runs in the
  level warp's town-leave refresh `0x00537340`, state 9% -> 100%); packets still differ in ORDER only: 1.14d's
  0x5D comes first in frame 9, d2rs sends it after the act-change messages because quest sends go to the rest
  outbox, drained after the sim's (`wired.rs collect_sent`) (S-M). The vendor part of `0x00537340` is still unwired.
- PROVISIONAL (REC-2900): item type test lacks the matched-item rules (flags, `shoots`) and skills 4/5 weapon.
