# Local buddy LB1-LB3 (impl-bitstream-vitals.md section 6)

## LB1 - itemstatcost Save Bits / Save Add: PASS

Source: game\extracted\patch_d2\data\global\excel\itemstatcost.txt (1.14d patch_d2; read directly, `data-tool tables` not run, not needed).
Cross-check: d2exp ItemStatCost.txt gives identical values.

| ID | row name | Save Bits exp/actual | Save Add exp/actual |
|----|----------|----------------------|---------------------|
| 22 | maxdamage | 7 / 7 | 0 / 0 |
| 60 | lifedrainmindam | 7 / 7 | 0 / 0 |
| 75 | item_maxdurability_percent | 7 / 7 | 20 / 20 |

## LB2 - NOT RUN
Recordings 20261006-015956-packets.jsonl and 20261006-022633-packets.jsonl are not on this PC (D2test searched recursively).

## LB3 - NOT RUN
No replay test exists and no recording with damage/potions/running is on this PC.
