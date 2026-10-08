# Handoff: q-char-panel-full (`claude/q-char-panel-full`)

## Links connected
- Labels: the 15 panel labels already read `StringLookup::get_id`; `play` binds the real tables (`app/strings.rs`). A test now pins that they resolve (`char_feed_tests`).
- Class line: new `ui/char_feed.rs` `class_line` (charstats `class` key through `StringLookup::get`).
- Next level: `ModelCharacter::next_level` reads the HUD's `experience` rows (no second table; `ClientTables` was not touched, the UI never sees it).
- Resist / defense colours: `CharTables` holds the `states` flags per state id; `ModelCharacter::resist_effect` / `defense_color` test the unit's state ids.
- Damage / attack-rating block: `char_feed::damage_block` feeds `char_details::hand_block` for the left and right skill.
- Loader: `app/hud.rs` `char_tables` / `install_char_tables`, called from `app/play.rs` next to the HUD tables.

## PROVISIONAL (REC-269)
See `docs/HANDOFF.md` REC-269: damage values only for `descdam` 1, 7, 18, 19, 20 and `descatt` 1, 2; no skill modifiers; `holyshield` without the shield test; no to-hit popups.

## Left
Skill-specific `descdam` / `descatt` entries (`skills/descriptions.md` §3–§4), skill-state gate, popups (need `monstats`), Shift spend.

## Local check
`cargo run -p d2-client --release -- play ...` (your usual play command), press C: labels, the class name under the portrait line, the experience / next-level numbers with commas, and the damage block for the left and right skill (Attack: its damage range and attack rating). Cast a resist-raising state and see the resist number turn blue.
