// Spec: specs/world/quests-act1-rest.md §1.2, §2.3, §3; specs/world/quests-act1.md §10.6
//! The quest world calls Cain's quest needs, on the play preview's
//! built game (synthetic fixtures): a monster spawned in a DRLG room
//! (Cain in town), a portal object to a level (the red portal to
//! Tristram), an object looked up by class and a monster removed.
//! Before `HostQuests` answered them they went to `AppRest`, which only
//! logged and returned nothing.

use d2_client::app::single_player::{self, GameData, SYNTHETIC_PORTAL_CLASS};
use d2_server::adapters::handlers::world::{QuestCall, WorldHost};
use d2_sim::units::{RoomId, UnitId};
use d2_sim::world::npc::class;
use d2_sim::world::quests::{QuestControl, QuestWorld};

const TRISTRAM: u32 = 38;

struct Probe {
    room: RoomId,
    at: (i32, i32),
    waypoint: UnitId,
}

#[derive(Debug)]
struct Out {
    cain: Option<UnitId>,
    cain_class: Option<u16>,
    portal: Option<UnitId>,
    found: Option<UnitId>,
    removed_gone: bool,
}

impl QuestCall for Probe {
    type Out = Out;
    fn call<W: QuestWorld>(self, _: &mut QuestControl, w: &mut W) -> Out {
        let (x, y) = self.at;
        let cain = w.spawn_monster(self.room, x, y, class::CAIN5, 1, 0);
        let cain_class = cain.and_then(|c| w.monster_class(c));
        let portal = w.open_portal(
            None,
            self.room,
            x + 6,
            y,
            TRISTRAM,
            SYNTHETIC_PORTAL_CLASS as u16,
            false,
        );
        let found = w.find_object_near(self.waypoint, SYNTHETIC_PORTAL_CLASS as u16);
        let removed_gone = match cain {
            Some(c) => {
                w.remove_monster(c);
                w.monster_class(c).is_none()
            }
            None => false,
        };
        Out {
            cain,
            cain_class,
            portal,
            found,
            removed_gone,
        }
    }
}

// Covers: specs/world/quests-act1-rest.md §1.2, §2.3
#[test]
fn cains_spawn_the_red_portal_and_his_removal_run_on_real_units() {
    let mut g = single_player::build(&GameData::Synthetic, single_player::DEFAULT_SEED).unwrap();
    let s = &mut g.sim;
    let room = s
        .game
        .lists
        .unit(g.waypoint)
        .and_then(|u| u.room())
        .unwrap();
    let at = s.events.action.hooks().path_position(g.waypoint);
    let probe = Probe {
        room,
        at: (at.0 + 3, at.1 + 8),
        waypoint: g.waypoint,
    };
    let out = WorldHost::quests(&mut s.world, &mut s.game, &mut s.events, probe).unwrap();
    assert!(out.cain.is_some(), "{out:?}");
    assert_eq!(out.cain_class, Some(class::CAIN5));
    let portal = out.portal.expect("the portal object");
    assert_eq!(out.found, Some(portal));
    assert!(out.removed_gone, "{out:?}");
    // The portal object keeps its destination (`interact` = level).
    let st = s.events.action.hooks().objects.as_ref().unwrap();
    assert_eq!(
        st.control.data.get(&portal).map(|d| u32::from(d.interact)),
        Some(TRISTRAM)
    );
    // Nothing fell through to the rest's log.
    assert!(
        s.world
            .rest
            .log
            .iter()
            .all(|l| !l.starts_with("spawn monster") && !l.starts_with("remove monster")),
        "{:?}",
        s.world.rest.log
    );
}
