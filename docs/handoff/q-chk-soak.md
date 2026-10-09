# q-chk-soak hand-back

Done: setup (`--no-wine`), one 60-min soak campaign on integ-r6 across all
starts and checkpoints (107 runs, 2 signatures, see `soak-findings.md`), both
reduced; soak log parser bug fixed (negative click coordinates).
Note: crates/d2-client/tests/app_boss_death.rs does not compile on integ-r6 (DumpArgs missing `save_out`), so clippy --all-targets fails there; not mine.
Open: route the two findings to their owners (desync -> walk desync owner;
item outside room -> items owner); more campaigns with other seeds; no ledger
rows settled (`ledger/q-chk-soak.tsv` not created: soak has no 1.14d
comparison). No new unowned build-queue rows.
Repro: `python3 tools/soak/soak.py campaign --minutes 60 --out target/soak`;
reduced logs in `tools/soak/repro/`.
