# q-strings: string tables in the play app

## Links connected
- `crates/d2-client/src/app/strings.rs` (new): `TableStrings` loads `string.tbl`, `patchstring.tbl` and `expansionstring.tbl` through `d2_data::strings::StringTables::load_from`, with the expansion table loaded only when `lod()`. It holds every element as UTF-16 by id (base `n`, patch `10000+n`, expansion `20000+n`) and by key. It implements `StringLookup` (`get`, `get_id`) and gives `by_id(u16) -> Vec<u16>`, the closure form that the HUD, NPC and message panels take.
- `install_strings` replaces `NoStrings` in the `WorldViewUi`. `app/play.rs` calls it right after `add_original_ui` (3 lines).
- Consumers that already read `ctx.strings`: the character panel, shop tabs, waypoint rows, stash gold-max text and the hire list.

## PROVISIONAL (`// d2rs-own, unverified`)
- The whole id → text map is built once at load (about 10k strings), not read per call as the original does. This is needed because `get_id` returns a borrow.
- A key present in several tables resolves as patch, then expansion, then base, the same order as `StringTables::id`.
- Language is fixed to `eng`.

## What's left
- Panels that take a `Fn(u16) -> Vec<u16>` closure (npc_menu, control/*, messages/*, item names) get it from `TableStrings::by_id`. The stitch-hud, stitch-npc and stitch-items sessions bind these where they draw. I did not edit those shared files.
- An absent id returns an empty string on the closure path and `None` on the `get_id` path.

## Local check
`D2_GAME_DIR=<game> cargo run -p d2-client -- play`, then open the character panel (`C`). You should see the stat labels (Strength, Dexterity, …) and the class line, which were blank with `NoStrings`.
Tests: `cargo test -p d2-client --lib app::strings` (2 synthetic-table tests).
