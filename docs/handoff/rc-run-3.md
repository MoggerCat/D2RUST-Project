# rc-run-3 hand-back (runner, first half of gen-obj-*)

- Scope: first 286 of 571 gen-obj-* (sorted by name, gen-obj-0 .. gen-obj-355); the second half is rc-run-3b.
- Result: 277 checks EQUAL under DECIDED REC-2055/2056 (every channel 0 differences; state PARTIAL only for the d2rs client gap; items PARTIAL only for no item created). 9 checks not EQUAL: gen-obj-256, 257, 258, 273, 285, 286, 287, 296, 297 (items PARTIAL, 1 item equal on both sides, 0 differences, but the 1.14d recording ends at frame 20 vs d2rs 80; outside REC-2056).
- gen-obj-346: first run had a transient 1.14d recorder failure (no orig.state.jsonl); re-run clean, EQUAL.
- Ledger: docs/handoff/ledger/rc-run-3.tsv (262 rows touching these checks; 9 left not-EQUAL). Causes: docs/handoff/rc-run-3-causes.tsv.
- Open: the 9 need a decision (extend REC-2056 to a short 1.14d recording with equal items, or make the recorder run to frame 80) - S.
- Note: no combined suite JSON (container restarts killed background runs); ran in three foreground chunks.
- orig-cache files changed locally and were not committed.
