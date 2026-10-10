# rc-cov-tables: hand-back

Owner decision (docs/PLAN.md decisions log): `cov.*` rows are checked against
the 1.14d data tables, no written spec needed.

## What changed
- `crates/d2-data/src/crosscheck.rs`: `TableReport.record_diffs` (every differing
  record with its field labels; `~` = explained by a spec rule).
- `tools/data-tool`: `data-tool cov-records [game_dir]` prints that map for every
  runtime / by-product table (compiled `.txt` vs shipped `.bin`, whole record).
- `tools/coord/cov_tables.py` (`--selftest`): maps each `cov.*` row's
  `source_1.14d` to table + record (monstats id = row; Levels/Missiles/Skills/
  States/Objects by `Id`; items by name across weapons/armor/misc; AI rows by
  monai name; "never-exercised" rows = whole table(s)); the "Expansion" separator
  row is not a record. The key name must match the ledger name, else UNKNOWN.
- Ledger part `docs/handoff/ledger/rc-cov-tables.tsv` (329 rows, group
  `cov-tables`, checks `-`, evidence in note). Rows already DIVERGED from a
  behaviour check are not overridden (68; their table data is also EQUAL).

## Result (cov rows, merged ledger)
| | UNKNOWN | NO-CHECK | DIVERGED | EQUAL |
|---|---|---|---|---|
| before | 269 | 61 | 68 | 0 |
| after | 0 | 1 | 70 | 327 |

Whole ledger EQUAL 1488 -> 1815.

## Mismatches by cause
- monstats record 707 NameStr (shipped 5382, compiled 11154; explained in
  field-types.md §10): makes the two whole-table rows `cov.monster.never-exercised`
  and `cov.npc-topic.never-exercised` DIVERGED. Every other record of all 73 tables is identical.
- The 68 behaviour-DIVERGED rows (akara, shamanfire, ...) are unchanged: their
  table data is identical, the divergence is in behaviour.

## Open
- `cov.quest.never-exercised` (1 row): quest flags/slots are d2rs-internal, no
  data table behind them; stays NO-CHECK.
- Scope: this proves the loaded table data is exact, not that the sim reads
  every field as the original does; behaviour stays with the scenario checks.
