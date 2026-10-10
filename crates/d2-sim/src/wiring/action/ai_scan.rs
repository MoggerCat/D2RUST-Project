// Spec: specs/monsters/ai.md §5.3 (`0x005DDC30`, scan 6 callback `0x005DCBD0`), §5.4 (scan mode 0), §6
//! The secondary target search `0x005DDC30` on the wired units: the
//! forced target, else room scan 6 (mode 0, callback `0x005DCBD0`) and
//! the alternative choice `0x005DD510`.

use crate::game::Game;
use crate::monsters::ai::{AiTargets, AiWorld};
use crate::units::{RoomId, UnitId, UnitType};

use super::{Pending, View};

/// Scan 6 radius: full-size distance below 49 (`0x31`, §5.3 rule 2).
pub const SCAN6_RANGE: i32 = 49;
/// State 146 (`invis`, §5.3 rule 1).
pub const STATE_INVIS: u32 = 146;
/// Unit flag 0x4 (+0xC4, `0x00451F30(C, 4)`).
pub const UNIT_FLAG_4: u32 = 0x4;
/// Threat class of a player (`0x005DC920`).
pub const PLAYER_THREAT: u8 = 14;
/// `0x005DD510`: an alternative farther than this is refused.
pub const ALT_MAX_DISTANCE: i32 = 5;
/// No distance.
const NONE_D: i32 = 0x7FFF_FFFF;

/// The scan 6 context {main, main d, alt, alt d}.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scan6 {
    pub main: Option<(UnitId, i32)>,
    pub alt: Option<(UnitId, i32)>,
}

impl<X: Pending> View<'_, X> {
    /// Scan mode 0 (`0x005DCE60`, §5.4): every unit of every room of the
    /// unit's room's near-room list (own room included), in list order.
    pub(crate) fn near_room_units(&self, game: &Game, unit: UnitId) -> Vec<UnitId> {
        use crate::path::collision::CollisionRooms;
        let Some(room) = game.lists.unit(unit).and_then(|e| e.room()) else {
            return Vec::new();
        };
        let d = &self.h.drlg;
        let mut rooms: Vec<RoomId> = (0..d.adjacent_count(room))
            .filter_map(|i| d.adjacent(room, i))
            .collect();
        if !rooms.contains(&room) {
            rooms.insert(0, room);
        }
        rooms
            .into_iter()
            .flat_map(|r| game.lists.room_units(r))
            .collect()
    }

    fn living_actor(&self, u: UnitId) -> bool {
        self.units
            .get(u)
            .is_some_and(|r| matches!(r.ty, UnitType::Player | UnitType::Monster))
            && !self.units.is_dead(u)
    }

    /// Filter `0x005DC970` (§5.3 rule 1) for scanner `s` and candidate `c`.
    fn scan6_filter(&mut self, game: &mut Game, s: UnitId, c: UnitId) -> bool {
        if !self.living_actor(s) || !self.living_actor(c) {
            return false;
        }
        let Some(room) = game.lists.unit(c).and_then(|e| e.room()) else {
            return false;
        };
        if self.h.drlg.in_town(game, room) {
            return false;
        }
        if self.units.get(c).map_or(0, |r| r.flags) & UNIT_FLAG_4 == 0 {
            return false;
        }
        if self.stats.has_state(c, STATE_INVIS) {
            let player = self.units.get(c).is_some_and(|r| r.ty == UnitType::Player);
            if player {
                // PROVISIONAL (ai.md §5.3 rule 1; REC-1695): the roll on
                // C's seed is made before the melee-range test.
                let r = self.seed(c).roll(100);
                if r < 80 && !self.in_melee_range(game, s, c) {
                    return false;
                }
            } else if self.in_melee_range(game, s, c) {
                return false;
            }
        }
        self.h.x.may_attack(s, c)
    }

    /// `MeleeRng` (monstats2 +0x0E) of the scanner's class row, 255 read
    /// as 0; 0 without a row (REC-1270).
    fn scanner_reach(&self, unit: UnitId) -> i32 {
        let t = &self.h.tables.combat;
        self.units
            .get(unit)
            .and_then(|r| t.monstats.get(r.class as usize))
            .and_then(|m| t.monstats2.get(usize::from(m.monstatsex)))
            .map_or(0, |m2| match m2.meleerng {
                255 => 0,
                r => i32::from(r),
            })
    }

    /// Room scan 6 with callback `0x005DCBD0` (§5.3): the nearest main
    /// (threat ≥ 2) and alternative (threat < 2) candidates closer than
    /// 49 by the full-size distance; ties keep the earlier unit.
    pub fn scan6(&mut self, game: &mut Game, unit: UnitId) -> Scan6 {
        let mut out = Scan6 {
            main: None,
            alt: None,
        };
        let at = self.h.path_position(unit);
        let size = self.path_size(unit);
        for c in self.near_room_units(game, unit) {
            // 1.
            if !self.scan6_filter(game, unit, c) {
                continue;
            }
            // 2.
            let d = crate::monsters::ai::distance_full_size(at, size, self.h.path_position(c));
            if d >= SCAN6_RANGE {
                continue;
            }
            // PROVISIONAL (ai.md §5.3 rule 1; q-fix-ass-traps, REC-1270): a
            // monster candidate in the scanner's melee range (`0x00622C40`
            // step 3: d ≤ 0, or d ≤ `MeleeRng` + 1; the line is not tested)
            // is skipped, whatever its states; 1.14d: a Lightning Sentry
            // (MeleeRng 0) never picks a Fallen at distance 1
            // (`ass-lightning-sentry-hit`).
            let monster = self.units.get(c).is_some_and(|r| r.ty == UnitType::Monster);
            if monster && (d <= 0 || d <= self.scanner_reach(unit) + 1) {
                continue;
            }
            // 3.
            let threat = match self.units.get(c).map(|r| (r.ty, r.class)) {
                Some((UnitType::Player, _)) => PLAYER_THREAT,
                Some((_, class)) => self
                    .h
                    .tables
                    .combat
                    .monstats
                    .get(class as usize)
                    .map_or(0, |m| m.threat),
                None => 0,
            };
            let main = threat >= 2;
            let held = if main { out.main } else { out.alt };
            if d >= held.map_or(NONE_D, |(_, hd)| hd) {
                continue;
            }
            // 4.
            if self.line_blocked(game, c, unit) {
                continue;
            }
            if main {
                out.main = Some((c, d));
            } else {
                out.alt = Some((c, d));
            }
        }
        out
    }

    /// `0x005DDC30` (§5.3): the forced target (§5.1), else scan 6 and the
    /// alternative choice `0x005DD510`; target, distance (0x7FFFFFFF
    /// when none) and the melee-range flag.
    pub(crate) fn secondary_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
    ) -> (Option<UnitId>, i32, bool) {
        // PROVISIONAL (ai.md §5.1, §5.3; REC-1695): the override is the
        // host's (`Pending::forced_target`, called with a = 0, s = 1).
        let picked = match self.h.x.forced_target(game, unit) {
            Some(f) => Some(f),
            None => {
                let s = self.scan6(game, unit);
                self.alternative_choice(game, unit, s)
            }
        };
        match picked {
            Some((t, d)) => (Some(t), d, self.in_melee_range(game, unit, t)),
            None => (None, NONE_D, false),
        }
    }

    /// `0x005DD510` (§5.3) on a scan's main and alternative: never for a
    /// player scanner or without an alternative; the alternative when
    /// there is no main target; refused when farther than 5; else the
    /// trial path and the scan 7 fallback, which are the host's
    /// ([`AiTargets::choose_alternative`], PROVISIONAL REC-1695).
    fn alternative_choice(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        s: Scan6,
    ) -> Option<(UnitId, i32)> {
        let is_player = self
            .units
            .get(unit)
            .is_some_and(|r| r.ty == UnitType::Player);
        let Some((a, ad)) = s.alt.filter(|_| !is_player) else {
            return s.main;
        };
        let Some((m, _)) = s.main else {
            return Some((a, ad));
        };
        if ad > ALT_MAX_DISTANCE {
            return s.main;
        }
        if self.choose_alternative(game, unit, Some(m), a) {
            Some((a, ad))
        } else {
            s.main
        }
    }
}
