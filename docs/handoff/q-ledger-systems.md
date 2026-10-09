# q-ledger-systems (done)

Done: `docs/handoff/ledger/systems.tsv` (ledger 1 format): 934 rows = one per C2S id (specs/sim/client-messages.tsv),
one per S2C id (server-messages.tsv), one per `###` rule group of every spec under specs/{sim,combat,flows,seams,client,render,audio,ui}
and the d2s/format specs, plus system.perf.budget.
Method: generated, not hand-judged. State comes from (a) spec status "conformance-passing" -> EQUAL unless its checks DIVERGE,
(b) a crate cites the spec (`// Spec:`) or uses the message name -> NO-CHECK, (c) otherwise NOT-IMPLEMENTED;
DIVERGED where a mapped check (checks-status.md at q-fix-check-triage) diverges. State is per spec FILE; rule-group rows share it.
Open: `exercised` is `?` (coverage sessions); sizes are coarse (S for messages, M for specs); check mapping is by name only
(rng/draws/tick, walk, combat); message rows share the whole-stream packets verdict, not a per-id compare.
Repro: `python3 tools/ledger/systems_gen.py` (needs `git fetch origin claude/q-fix-check-triage`).
