# q-ledger-monsters — done / open

Done: docs/handoff/ledger/monsters.tsv (301 rows): bosses (monstats boss=1), superuniques, AI types (one per ai-functions.tsv row), quest monsters, population/init/minions/corpse systems, umods, monster missiles.
Open: `checks` column is a case-insensitive word match of the monster name inside traces/checks/*.check (generic names such as npc/smith/trap-melee over-match); `exercised` is `?`; sizes are coarse; implemented-ness of quest/umod rows not individually verified (state NO-CHECK = spec'd and ported, nothing compares).
Repro: the generator was a throwaway script over private-repo Patch_D2 monstats.txt/SuperUniques.txt, specs/monsters/*.tsv and checks-status.md (origin/claude/q-fix-check-triage).
