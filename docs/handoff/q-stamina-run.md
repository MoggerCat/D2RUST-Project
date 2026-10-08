# q-stamina-run: stamina in the play preview

## Links connected

| # | Link | Was | Now |
|---|---|---|---|
| 1 | Regeneration | Nothing scheduled the player's regen event (`units.md` §6.1, event 3), so life, stamina and mana never regenerated in `play` (only `player_regen` unit tests ran it). | `d2-server` `adapters/session.rs` `enter_game` runs the join step `units::modes::player_join` after the placement: neutral mode start, regen event every frame, refresh event at +250. |
| 2 | Run prediction at zero stamina | The client kept drawing a run after the server turned it into a walk. | `bridge/predict.rs`: a run steps and shows as a walk while the model's stat 10 is 0 (set by 0x95 / 0x96). |
| 3 | Stamina potion | `vps` was not a potion. | `wiring/inventory/potion.rs` `Potion::Stamina`: state 136, stat 28 = 1000 for 250 frames. PROVISIONAL, REC-130. |

Already in place and checked, not changed: the server drains stamina per running tick outside towns and restarts the walk at zero (`path/walk/step.rs` `run_drain`, `d2-server` walk tests); 0x95 / 0x96 set stat 10 in the model; the stamina bar reads stats 10 / 11; the click dispatcher sends a walk, not a run, at stamina 0.

## Tests
- `d2-client/tests/app_stamina.rs` (new, over the real server thread): standing regenerates stamina and the client gets it in 0x95 / 0x96; full stamina is not exceeded. The first fails without link 1.
- `predict.rs` `a_run_with_no_stamina_walks`; `potion.rs` `the_stamina_potion_code_is_classified`.

## PROVISIONAL / left
- Stamina potion amount and duration (REC-130). No end-to-end test of a run outside the town: the synthetic town is the only map, and running in towns does not drain (original rule).
- The stamina bar's blue state (group 24) is still not in the model (stitch-hud).

## Local check
`cargo run -p d2-client --release -- play --new paladin Uther`, leave the town, press R (run) and click far away repeatedly: the stamina bar drains while running, the character turns to a walk when it is empty, and the bar refills while standing (and a little while walking). In town the bar does not drain.
