# q-ledger-monsters — done / open

Done: docs/handoff/ledger/monsters.tsv (301 rows): bosses (monstats boss=1), superuniques, AI types (one per ai-functions.tsv row), quest monsters, population/init/minions/corpse systems, umods, monster missiles.
Open: `checks` on monster rows = checks that `poke spawn` the class id or warp into a level whose Levels.txt Mon1-10 list has it; superunique and quest rows have none (no check pokes them). `exercised` is `?`; quest/umod rows not individually verified.
Repro: the generator was a throwaway script over private-repo Patch_D2 monstats.txt/SuperUniques.txt, specs/monsters/*.tsv and checks-status.md (origin/claude/q-fix-check-triage).
