// Spec: specs/seams/sim-server.md
//! Contract checks of the `d2-sim` ↔ `d2-server` seam that need no game
//! files: the S→C messages the sim wiring builds by hand are the bytes
//! the `d2-proto` layouts (the client's decoder) read back, and the two
//! builders of S→C 0x03 (game entry in `d2-server`, act change in
//! `d2-sim`) agree.

use d2_proto::server::{LoadAct, MapHide, MapReveal};
use d2_proto::FixedMessage;
use d2_server::adapters::session;
use d2_sim::drlg::{act_of_level, TOWN_LEVELS};
use d2_sim::units::messages::map_hide;
use d2_sim::wiring::path::act_change;
use d2_sim::wiring::path::place::map_reveal;

/// Tile origins and level ids a 1.14d act holds (`client/model.md` §9
/// rule 3: room origins in tiles, e.g. (0x3A0, 0x388) of level 1) and
/// the edges of the u16 / u8 fields.
const ROOMS: [(u16, u16, u8); 5] = [
    (0x3A0, 0x388, 1),
    (968, 1120, 1),
    (0, 0, 2),
    (0xFFFF, 0x7FFF, 136),
    (24, 0, 2),
];

// Covers: specs/seams/sim-server.md §2.3
#[test]
fn room_messages_decode_to_the_fields_the_sim_wrote() {
    for (x, y, level) in ROOMS {
        let r = MapReveal::decode(&map_reveal(x, y, level)).unwrap();
        assert_eq!((r.x, r.y, r.level), (x, y, level));
        let h = MapHide::decode(&map_hide(x, y, level)).unwrap();
        assert_eq!((h.x, h.y, h.level), (x, y, level));
    }
}

// Covers: specs/seams/sim-server.md §2.4
#[test]
fn both_load_act_builders_agree() {
    for act in 0u8..5 {
        let (seed, obj) = (0x1038_88C4 ^ u32::from(act), 0x9FE0_D161);
        let entry = session::load_act(act, seed, obj).encode();
        let town = TOWN_LEVELS[usize::from(act)] as u16;
        let change = act_change::load_act(act, seed, town, obj);
        assert_eq!(entry.as_slice(), change.as_slice(), "act {act}");
        let m = LoadAct::decode(&change).unwrap();
        assert_eq!((m.act, m.f2, m.f6, m.f8), (act, seed, town, obj));
        // u16@6 is the act's own town (`client/model.md` §11 rule 1).
        assert_eq!(act_of_level(u32::from(m.f6)), act);
    }
}
