# q-messages: game messages in play

## Links connected
- `OriginalUi::apply_output` (`ui/msg_ui.rs`): `ChatLine` is no longer a "not applied" skip. `chat_line` gates it (conversion, overhead record), `messages::chat::chat_action` builds the line (`messages.md` §3 table), `GameMessages::add` wraps and queues it (§2 r1–r3).
- S→C 0x5D quest rows 3 and 15 (strings 3708 / 3710) were skipped; they now queue the string id, resolved with the string table at the next draw.
- `MessagesUi` panel (`ui/game_messages.rs`, registered in `OriginalUi::install`) draws the list (§2 r4: font 13, x 15, y 20 + 15 per line, open-mode rules, hidden while the message log is open) with a dark backing, and expires records after 10 s (§2 r5).
- `FontMeasure` gained `wrap`, `width_a`, `width_c` for the metrics.
- Tests (`game_messages::tests`): chat line drawn and expired, named line / whisper formats, quest message string.

## PROVISIONAL (REC-127)
Expiry clock 40 ms per client frame; Latin-1 text; HUD fill tiles as the backing.

## Left
- Overhead bubbles (`more.overhead` records exist; needs the unit's screen position in the world view).
- 0x5A event text lines (no format spec), NPC text / dialog panels, recipe scroll.
- The server sends only the overhead 0x26 form today; "You can't use this" style lines need server sources (no sim path emits 0x26 types 1, 2, 4, 6).

## Local check
`cargo run -p d2-client -- play --new amazon Test` (as in PLAYABLE.md). Nothing in the game sends a chat line yet, so there is nothing to see in play beyond the unit tests: `cargo test -p d2-client --lib game_messages`.
