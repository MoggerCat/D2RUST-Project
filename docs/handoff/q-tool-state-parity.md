# q-tool-state-parity hand-back (PARTIAL: code half only)

## Done
- d2rs now writes `own` (state-snapshot.md §2): monsters from the AI
  control minion owner, missiles from `MissileData.owner`
  (`d2_sim::debug::state::owner`), items from the inventory holder
  (`d2-client` `state_dump::overlay_item_owners`). GUID 0 / -1 absent.
  `D2RS_GAPS` is now empty; header `fields` lists `own`.
- d2-sim test `a_monsters_own_is_its_control_minion_owner`; coverage test
  updated. `cargo test -p d2-sim --lib debug::state` 12 pass; clippy
  `-p d2-sim --all-targets -D warnings` clean; spec index check clean.

## Open (not done)
- `d2-client` edit (`overlay_item_owners`) is NOT compiled: Bevy system
  libs are missing in this container (session-setup.sh was blocked by the
  permission classifier). Run `cargo clippy -p d2-client --all-targets`.
- The 39 Wine reruns, `checks-status.md` update, ledger part file
  `docs/handoff/ledger/q-tool-state-parity.tsv`: not done (no Wine).
- `q`: recorder side fixed by PC1 (claude/local-pc1-late); merge it, not
  done here.
- Pets/hirelings: d2rs sets `minion_owner` only where the AI control gets
  one; the reruns will show any owner the control lacks.

## Repro
`python3 tools/scenario-diff/suite.py` (needs Wine, see tools/cloud-game/README.md),
then `python3 tools/scenario-diff/status_md.py`.
