// Spec: specs/sim/intents-events.md §7.2, §7.8, §7.9; specs/sim/tick.md §6 rule 6 (wiring of the room switch)
//! The client room switch `0x00537B50(client, new room)` on the action
//! wiring (`intents-events.md` §7.8): the DRLG bookkeeping
//! ([`super::DrlgWorld::client_switches_room`]), then, for every room
//! the client joined (new array order), S→C 0x07, the AI wake-up of its
//! monsters when the client is the room's only one, and the add messages
//! (`0x00571F90`, §7.2) of its units; then, for every room it left (old
//! array order), S→C 0x0A for its units, S→C 0x08, and the player update
//! when the room was the client's. Callers: the per-client update
//! (`tick.md` §6 rule 5, [`super::ActionSim`]'s `client_level_change`)
//! and game entry (`path-placement.md` §11, through
//! `crate::wiring::path::place`).
//!
//! Monster add messages: [`super::monster_add`].
//!
//! Also the room-ready test `0x0061A460` (`tick.md` §6 rule 6) and the
//! session state the join reads ([`SessionState`]).
//!
//! Every message goes to the client's player ([`Pending::send`]).
//!
//! Not sent, because no spec gives them (named, not guessed):
//! - missile 0x73 (`0x0059FEE0`), item 0x9C (the item world is not
//!   reachable from the action wiring, as for §7.1);
//! - player part B for another player (`0x005489F0`, `0x005484B0`,
//!   multiplayer only, §7.9 rule 5), the corpse 0x74 and the inventory
//!   messages `0x00534F80`;
//! - the leave side's `0x005738D0` (monsters of a room without clients)
//!   and rule 4's portal-flag record update (`0x0061AE30`).

use std::collections::BTreeMap;

use crate::game::Game;
use crate::monsters::ai;
use crate::units::messages::{self, SendStat, SentState};
use crate::units::{ClientId, RoomId, UnitId, UnitType};

use super::rooms::RoomSwitch;
use super::{Pending, View, WiringError};

/// The session facts of the players a client join brings in (the save,
/// `path-placement.md` §13 rule 2): read by the add messages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionState {
    /// Player data names (0x59 bytes 6..22), zero-padded.
    pub names: BTreeMap<UnitId, [u8; 16]>,
    /// The overhead records (unit +0xA4; `intents-events.md` §9 rule 3,
    /// written by the C→S 0x14 handler through
    /// [`View::replace_overhead`]): text (`0x006611E0`) and byte +8. The
    /// record is live while the unit's `hover` frame is set.
    pub overheads: BTreeMap<UnitId, (Vec<u8>, u8)>,
}

/// S→C 0x59 AssignPlayer (`0x0053E8F0`, 26 bytes, §7.2 part A): GUID
/// u32@1, class u8@5, name 16 bytes @6, x u16@0x16, y u16@0x18.
pub fn assign_player(guid: u32, class: u8, name: &[u8; 16], x: u16, y: u16) -> [u8; 26] {
    let mut b = [0u8; 26];
    b[0] = 0x59;
    b[1..5].copy_from_slice(&guid.to_le_bytes());
    b[5] = class;
    b[6..22].copy_from_slice(name);
    b[22..24].copy_from_slice(&x.to_le_bytes());
    b[24..26].copy_from_slice(&y.to_le_bytes());
    b
}

impl<X: Pending> View<'_, X> {
    /// The room switch `0x00537B50(client, new)` (module docs). Nothing
    /// when the client is unknown or `new` is its room; a switch between
    /// acts (the act change, `waypoints.md` open question 1) is not
    /// handled: the client keeps its room.
    pub fn room_switch(&mut self, game: &mut Game, client: ClientId, new: Option<RoomId>) {
        let Some(e) = game.lists.client(client) else {
            return;
        };
        let (old, player) = (e.room, e.player);
        if old == new {
            return;
        }
        let act_of = |r: Option<RoomId>| r.and_then(|r| game.lists.room(r)).map(|r| r.act);
        let act = match (act_of(old), act_of(new)) {
            (Some(a), Some(b)) if a != b => return,
            (Some(a), _) | (None, Some(a)) => a,
            (None, None) => return,
        };
        let switch = self
            .h
            .drlg
            .client_switches_room(&mut game.lists, act, client, old, new);
        match switch {
            Ok(s) => {
                if let Some(player) = player {
                    self.switch_messages(game, client, player, old, &s);
                }
            }
            Err(e) => self.h.errors.push(e),
        }
        if let Some(e) = game.lists.client_mut(client) {
            e.room = new;
        }
    }

    /// §7.8 rules 2–3 for one switch, to `player`'s client.
    fn switch_messages(
        &mut self,
        game: &mut Game,
        client: ClientId,
        player: UnitId,
        old: Option<RoomId>,
        s: &RoomSwitch,
    ) {
        // Rule 2: joins (`0x0053A8E0`).
        for r in &s.joined {
            let reveal = crate::wiring::path::place::map_reveal(
                r.tile_x as u16,
                r.tile_y as u16,
                r.level as u8,
            );
            self.h.x.send(player, &reveal);
            if r.clients <= 1 {
                self.wake_room_monsters(game, r.room);
            }
            for u in game.lists.room_units(r.room) {
                if u != player {
                    self.add_messages(game, player, u);
                }
            }
        }
        // Rule 3: leaves (`0x0053A9B0`).
        for r in &s.left {
            for u in game.lists.room_units(r.room) {
                let Some(e) = game.lists.unit(u) else {
                    continue;
                };
                if e.ty != UnitType::Missile {
                    let msg = messages::remove_unit(e.ty as u8, e.guid);
                    self.h.x.send(player, &msg);
                }
            }
            let hide = messages::map_hide(r.tile_x as u16, r.tile_y as u16, r.level as u8);
            self.h.x.send(player, &hide);
            if Some(r.room) == old && self.h.paths.is_some() {
                crate::wiring::path::walk::update_messages(self, &*game, client, player);
            }
        }
    }

    /// §7.8 rule 2.3: every monster of the room gets `0x00573780`
    /// ([`ai::client_entered_room`]). No AI store: nothing.
    fn wake_room_monsters(&mut self, game: &mut Game, room: RoomId) {
        let Some(mut store) = self.h.ai.take() else {
            return;
        };
        let t = self.h.tables.clone();
        let info = self.h.ai_info;
        {
            let mut v = View::of(&mut *self.units, &mut *self.stats, self.data, &mut *self.h);
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                    skills: &t.skills.skills,
                    missiles: &t.skills.missiles,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            ai::client_entered_room(game, &mut cx, room);
        }
        self.h.ai = Some(store);
    }

    /// The add messages `0x00571F90(game, unit, client)` of `unit` to the
    /// client of `receiver` (§7.2; module docs for what is not sent).
    pub fn add_messages(&mut self, game: &Game, receiver: UnitId, unit: UnitId) {
        let Some(e) = game.lists.unit(unit) else {
            return;
        };
        let (ty, guid) = (e.ty, e.guid);
        let Some(r) = self.units.get(unit) else {
            return;
        };
        let (class, mode) = (r.class, r.mode);
        let (x, y) = self.h.path_position(unit);
        match ty {
            UnitType::Player => {
                let name = self.h.session.names.get(&unit).copied().unwrap_or_default();
                let m = assign_player(guid, class as u8, &name, x as u16, y as u16);
                self.h.x.send(receiver, &m);
                self.player_part_b(game, receiver, unit);
            }
            UnitType::Object => self.object_add(receiver, unit, guid, class, mode, (x, y)),
            UnitType::Tile => {
                let m = messages::assign_warp(ty as u8, guid, class as u8, x as u16, y as u16);
                self.h.x.send(receiver, &m);
            }
            UnitType::Monster => self.monster_add(game, receiver, unit),
            // Module docs: not specified far enough.
            UnitType::Missile | UnitType::Item => {}
        }
    }

    /// Player part B as far as it is specified (§7.2, §7.9): the unit's
    /// states (0xAA, `0x00570E30`), its pending event records
    /// (`0x00571CD0`: d2rs keeps none, so nothing) and the overhead text
    /// (`0x00571620`).
    pub fn player_part_b(&mut self, game: &Game, receiver: UnitId, unit: UnitId) {
        let Some(e) = game.lists.unit(unit) else {
            return;
        };
        let (ty, guid) = (e.ty as u8, e.guid);
        let states = self.unit_states_message(ty, guid, unit);
        self.h.x.send(receiver, &states);
        self.overhead_message(receiver, unit, ty, guid);
    }

    /// §7.9 rule 3 (`0x00571620`): unit +0xA4 = 0 → S→C 0x76. Else,
    /// unless the unit is a player the receiver relates to
    /// (`0x0055B300(P, unit, 4)` or `0x0055B300(unit, P, 2)`), the
    /// overhead chat 0x26 form 5 with the +0xA4 record's text
    /// (the record [`View::replace_overhead`] kept, else
    /// [`Pending::overhead_record`]; none → nothing).
    pub(super) fn overhead_message(&mut self, receiver: UnitId, unit: UnitId, ty: u8, guid: u32) {
        let Some(r) = self.units.get(unit) else {
            return;
        };
        if r.hover.is_none() {
            self.h.x.send(receiver, &messages::unit_ref(0x76, ty, guid));
            return;
        }
        if r.ty == UnitType::Player
            && (self.h.x.player_relation(receiver, unit, 4)
                || self.h.x.player_relation(unit, receiver, 2))
        {
            return;
        }
        let record = self.h.session.overheads.get(&unit).cloned();
        if let Some((text, byte8)) = record.or_else(|| self.h.x.overhead_record(unit)) {
            self.h
                .x
                .send(receiver, &messages::overhead_chat(byte8, ty, guid, &text));
        }
    }

    /// The overhead record replaced (`intents-events.md` §9 rule 3:
    /// `0x006611A0` free, `0x00661110(game +0x1C, text, frame)` new, byte
    /// +8 := `byte8`, `0x00661230`): unit +0xA4's timeout frame
    /// (`UnitRecord::hover`, read by event 6, `units.md` §6.1) := `end`
    /// and the record kept for the overhead 0x26 (§7.9 rule 3); then
    /// [`Pending::replace_overhead`].
    pub fn replace_overhead(&mut self, unit: UnitId, text: &[u8], byte8: u8, end: i32) {
        if let Some(r) = self.units.get_mut(unit) {
            r.hover = Some(end);
        }
        self.h
            .session
            .overheads
            .insert(unit, (text.to_vec(), byte8));
        self.h.x.replace_overhead(unit, text, byte8, end);
    }

    /// S→C 0xAA of `unit` (§7.9 rule 1): its states in ascending order,
    /// rule 1.1's filter (below the `states` count, no `nosend` bit), each
    /// with its stat list (`0x006256B0`, the base array in list order).
    /// The `nosend` bits and the itemstatcost send columns come from
    /// [`super::ActionHooks::bodies`]; without them no state is `nosend`
    /// and no stat has a row.
    pub fn unit_states_message(&self, unit_type: u8, guid: u32, unit: UnitId) -> Vec<u8> {
        let count = self.stats.data().states.count();
        let bodies = self.h.bodies.as_deref();
        let nosend = |s: usize| {
            bodies
                .and_then(|b| b.state_nosend.get(s))
                .copied()
                .unwrap_or(false)
        };
        let mut states = Vec::new();
        if let Some((bits, _)) = self.stats.state_bits(unit) {
            for (w, word) in bits.iter().enumerate() {
                for b in 0..32 {
                    let s = w * 32 + b;
                    if word & (1 << b) == 0 || s >= count || nosend(s) {
                        continue;
                    }
                    let entries = self.state_list(unit, s as u16).map(|l| {
                        self.stats
                            .base_entries(l)
                            .into_iter()
                            .map(|(k, v)| (k as u32 as u16, crate::stats::key_stat(k), v))
                            .collect()
                    });
                    states.push(SentState {
                        state: s as u16,
                        entries,
                    });
                }
            }
        }
        messages::unit_states(unit_type, guid, &states, |id| {
            bodies
                .and_then(|b| b.stat(i32::from(id)))
                .map(|r| SendStat {
                    bits: r.send_bits,
                    param_bits: r.send_param_bits,
                    signed: r.signed,
                })
        })
    }

    /// Object part A (§7.2): S→C 0x51 (GUID, class, x, y, mode, the
    /// object data's interact byte), then the portal message 0x60 for a
    /// `SubClass` bit 2 row (`objects.md` §14 builder details).
    ///
    /// A game without the object control (`ActionHooks::objects` `None`,
    /// no object data) sends interact 0, as its 0x03 sends game +0x80 = 0.
    /// Class 59 then sends 0x82 (`0x0053DB90`, §7.2 table, layout §6
    /// rule 6) from [`Pending::portal_owner`].
    fn object_add(
        &mut self,
        receiver: UnitId,
        unit: UnitId,
        guid: u32,
        class: u32,
        mode: u32,
        (x, y): (i32, i32),
    ) {
        let st = self.h.objects.as_ref();
        let data = st.and_then(|s| s.control.data.get(&unit));
        let interact = data.map_or(0, |d| d.interact);
        let portal = st
            .zip(data)
            .filter(|(s, d)| s.tables.object(d.class).is_ok_and(|o| o.subclass & 4 != 0))
            .map(|(_, d)| crate::world::objects::portal_message(d));
        let m =
            messages::assign_object(guid, class as u16, x as u16, y as u16, mode as u8, interact);
        self.h.x.send(receiver, &m);
        if let Some(b) = portal {
            self.h.x.send(receiver, &b);
        }
        if class == 59 {
            if let Some((owner, name, portal2)) = self.h.x.portal_owner(unit) {
                // PROVISIONAL (intents-events.md §7.2, §6 rule 6): u32@21 is
                // this portal's GUID and u32@25 its pair's (−1: none), as
                // the client handler reads them (`client/msg-units.md`
                // rule 7); settled by a town-portal join capture.
                let m = messages::portal_ownership(owner, &name, guid, portal2);
                self.h.x.send(receiver, &m);
            }
        }
    }

    /// Room ready `0x0061A460(R)` (`tick.md` §6 rule 6), R = the client's
    /// room: a null R is fatal assert 0x3EF ([`WiringError::NoClientRoom`]).
    pub fn client_room_ready(&mut self, game: &Game, client: ClientId) -> bool {
        let Some(room) = game.lists.client(client).and_then(|e| e.room) else {
            self.h.errors.push(WiringError::NoClientRoom(client));
            return false;
        };
        self.h.drlg.room_ready(game, room).unwrap_or(false)
    }
}
