// Spec: specs/sim/tick.md
//! One server tick (`0x0052D870`, D2MOO `GAME_UpdateProgress`): the frame
//! counter (§2), the fixed step order (§3), the room pass (§4), the timer
//! run (§5), the client pass (§6) and the periodic steps (§7).
//!
//! The host schedule (§1) and the wall-clock, host-only parts (§8: frame-
//! rate statistics, heartbeat) belong to `d2-server` and are not here; the
//! client pass's character save (§6 rule 3) is raised here as
//! [`Game::character_save_due`] and written by `d2-server`. d2-sim owns the step order, the list iteration and the
//! flags; the bodies of steps whose behaviour is owned by specs not yet
//! written (environment, population, messages, quests, items, AI, ...)
//! are hooks: [`TickHooks`] and, for timer events, [`EventDispatch`].

pub mod events;
pub mod timer;

use crate::game::Game;
use crate::units::lists::{client_state, ACTS};
use crate::units::{ClientId, RoomId, UnitId};
use timer::{TimerClass, TimerList, TimerRun};

/// Tick length in ms: `1000 / rate`, integer division (§1.1). Used by the
/// host schedule only.
pub const TICK_LENGTH_MS: u32 = 1000 / crate::TICKS_PER_SECOND;

/// Periods of the periodic steps in frames (§7).
pub mod period {
    pub const QUESTS: i32 = 20;
    pub const ROOM_DEACTIVATION: i32 = 12;
    pub const FREE_INACTIVE_ROOMS: i32 = 11;
    pub const EXPIRED_ITEMS: i32 = 1500;
    /// The client pass's character save (§6 rule 3).
    pub const CHARACTER_SAVE: i32 = 8192;
}

/// Room inactivity counter threshold for step 9: deactivate when > 10.
pub const ROOM_DEACTIVATION_THRESHOLD: u32 = 10;

/// Whether a periodic step with `period` runs at `frame` (signed
/// remainder, §2.2).
pub const fn is_due(frame: i32, period: i32) -> bool {
    timer::frame_mod(frame, period) == 0
}

/// Runs one timer event (§5.5 "Running"): the timer's callback, or its
/// class's default handler (§5.6, data in [`events`]). What each event
/// does per unit kind is owned by the unit specs (open question 3).
///
/// The handler may schedule and cancel timers and remove units through
/// `game`; the queue's cursor rules (§5.4, §5.5) keep the run consistent.
pub trait EventDispatch {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun);
}

/// Bodies of the tick steps owned by other specs. Each default does
/// nothing: a placeholder until its spec is written, never a claimed
/// behaviour. d2-sim calls them in the spec's order and owns the lists
/// and flags around them.
#[allow(unused_variables)]
pub trait TickHooks: EventDispatch {
    /// Step 1 (`0x0061C040(act, a)`, `render/lighting.md` §9.3 rule 5):
    /// advance act `act`'s day/night cycle with `A` = `act`; true when the
    /// index or type changed or the hour moved more than 16 degrees since
    /// the last report.
    fn advance_environment(&mut self, game: &mut Game, act: u8) -> bool {
        false
    }

    /// Step 1, per client when act `act`'s cycle changed: refresh the
    /// player's items (`0x0055FDE0`), then message 0x53 if the client is
    /// in game (state 4) in that act.
    fn environment_changed(&mut self, game: &mut Game, act: u8, client: ClientId) {}

    /// Step 3 (§4.1, `0x0054F060`): ambient spawns of an active room.
    fn ambient_spawns(&mut self, game: &mut Game, room: RoomId) {}

    /// Step 3 (§4.2, `0x005559A0`): preset units.
    fn spawn_presets(&mut self, game: &mut Game, room: RoomId) {}

    /// Step 3 (§4.2–§4.3, `0x00542B40`): restore inactive units.
    fn restore_inactive_units(&mut self, game: &mut Game, room: RoomId) {}

    /// Step 3 (§4.2, `0x00552610`): object population.
    fn populate_objects(&mut self, game: &mut Game, room: RoomId) {}

    /// Step 3 (§4.2, `0x0054EC90`): monster population.
    fn populate_monsters(&mut self, game: &mut Game, room: RoomId) {}

    /// Step 5 (`0x0061A460`): the client's room is ready.
    fn client_room_ready(&mut self, game: &mut Game, client: ClientId) -> bool {
        false
    }

    /// Step 5: message 4 to the client (before its state becomes 4).
    fn send_load_complete(&mut self, game: &mut Game, client: ClientId) {}

    /// Step 5 (`0x0055DF00`): inventory refresh after the state change.
    fn refresh_inventory(&mut self, game: &mut Game, client: ClientId) {}

    /// Step 5, joining clients: the join sequence (`0x0052C410`,
    /// `0x0055B620`, host callback, message 0x5A to all).
    fn join_sequence(&mut self, game: &mut Game, client: ClientId) {}

    /// Per-client update (§6.5, `0x0053A770`): removal messages for units
    /// deleted in the adjacent rooms.
    fn send_removed_units(&mut self, game: &mut Game, client: ClientId) {}

    /// Per-client update (§6.5, `0x0053A5D0`): one unit's update message,
    /// for each unit of each adjacent room's update queue.
    fn send_unit_update(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {}

    /// Per-client update (§6.5): player stat-change messages
    /// (`0x006258D0`); then, only when the player's flag-ex (+0xC8) bit 21
    /// is set, the inventory refresh (`0x0055DF00`) and `0x0055F4F0`
    /// (both skipped otherwise; `0x0055F4F0` is an empty function in
    /// 1.14d, OQ6, so an implementation calls nothing for it).
    fn client_update_messages(&mut self, game: &mut Game, client: ClientId) {}

    /// Per-client update (§6.5): the player's room differs from the
    /// client's: level change (`0x00543B90`, `0x00537340`) and room switch
    /// (`0x00537B50`, which updates the client's room).
    fn client_level_change(&mut self, game: &mut Game, client: ClientId) {}

    /// Per-client update (§6.5, `0x0053FC20`): arena sync.
    fn arena_sync(&mut self, game: &mut Game, client: ClientId) {}

    /// Step 6 (`0x00553220`): per unit of an update queue.
    fn unit_update(&mut self, game: &mut Game, unit: UnitId) {}

    /// Step 6, last (`0x0053FAE0`): clears an arena flag bit.
    fn clear_arena_flag(&mut self, game: &mut Game) {}

    /// Step 7 (`0x0061A2C0`): free an active room's unit-removal records.
    fn free_removal_records(&mut self, game: &mut Game, room: RoomId) {}

    /// Step 8 (`0x00543E10`): quest updater.
    fn update_quests(&mut self, game: &mut Game) {}

    /// Step 9 (`0x0061A790`): the room's inactivity counter (what
    /// increments it: tick.md open question 4).
    fn room_inactivity(&mut self, game: &mut Game, room: RoomId) -> u32 {
        0
    }

    /// Step 9 (`0x0061A3F0`): the act allows removing the room.
    fn act_allows_room_removal(&mut self, game: &mut Game, act: u8, room: RoomId) -> bool {
        false
    }

    /// Step 9 (`0x005433F0`): compress a unit to inactive storage.
    fn compress_unit(&mut self, game: &mut Game, unit: UnitId) {}

    /// Step 9, after `0x0061A910` unlinked the room from the act list:
    /// the rest of that function, owned by the DRLG spec (`rooms.md`
    /// §8.2: neighbours' adjacency arrays, record free, tiles).
    fn room_deactivated(&mut self, game: &mut Game, act: u8, room: RoomId) {}

    /// Step 10 (`0x0061AA20`): free inactive rooms of an act.
    fn free_inactive_rooms(&mut self, game: &mut Game, act: u8) {}

    /// Step 11 (`0x00558B90`): delete inactive items of an act.
    fn delete_inactive_items(&mut self, game: &mut Game, act: u8) {}

    /// Step 11 (`0x00542AC0`): expired inactive-unit item nodes of an act.
    fn expire_inactive_unit_items(&mut self, game: &mut Game, act: u8) {}
}

/// One tick (`0x0052D870`, §3): frame += 1, then steps 1–11 in order.
pub fn tick<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    tick_through_timers(game, hooks);
    tick_from_client_pass(game, hooks);
}

/// The first half of [`tick`]: frame += 1 and steps 1–4, ending with the
/// timer queue. A host whose unit work runs inside step 4 but outside
/// [`EventDispatch`] runs it between this and [`tick_from_client_pass`],
/// so its messages reach the same tick's client pass
/// (`flows/server-tick.md` §2 rule 2).
pub fn tick_through_timers<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    // Step 0. The debug trap switch on game +0x1DC8 is never set in
    // normal play and is not modelled.
    game.frame = game.frame.wrapping_add(1);
    environment(game, hooks);
    // Step 2 (frame-rate statistics) is wall-clock only (§8).
    room_pass(game, hooks);
    run_timer_events(game, hooks);
}

/// The second half of [`tick`]: steps 5–11, from the client pass to the
/// periodic steps.
pub fn tick_from_client_pass<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    client_pass(game, hooks);
    room_update_queues(game, hooks);
    removal_records(game, hooks);
    let frame = game.frame;
    if is_due(frame, period::QUESTS) {
        hooks.update_quests(game);
    }
    if is_due(frame, period::ROOM_DEACTIVATION) {
        room_deactivation(game, hooks);
    }
    if is_due(frame, period::FREE_INACTIVE_ROOMS) {
        for act in 0..ACTS as u8 {
            if game.lists.act(act).is_some() {
                hooks.free_inactive_rooms(game, act);
            }
        }
    }
    if is_due(frame, period::EXPIRED_ITEMS) {
        for act in 0..ACTS as u8 {
            if game.lists.act(act).is_some() {
                hooks.delete_inactive_items(game, act);
                hooks.expire_inactive_unit_items(game, act);
            }
        }
    }
}

/// Step 1 (`0x0052D7B0`).
fn environment<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    for act in 0..ACTS as u8 {
        if game.lists.act(act).is_none() || !hooks.advance_environment(game, act) {
            continue;
        }
        let mut cur = game.lists.client_first();
        while let Some(c) = cur {
            cur = game.lists.client_next(c);
            hooks.environment_changed(game, act, c);
        }
    }
}

/// Step 3, the room pass (§4).
fn room_pass<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    for act in 0..ACTS as u8 {
        if !game.lists.act(act).is_some_and(|a| a.pending_rooms) {
            continue;
        }
        let mut cur = game.lists.room_first(act);
        while let Some(room) = cur {
            room_body(game, hooks, room);
            // Next read after the body (unit-order.md §10).
            cur = game.lists.room_next(room);
        }
        if let Some(a) = game.lists.act_mut(act) {
            a.pending_rooms = false;
        }
    }
}

/// Off-tick population `0x0052D0F0(game, room)` (§4 r5): the room
/// pass body (rules 1–3) for one room, without the act flag test and
/// without clearing act +0x54. Called by the portal and arrival paths of
/// `monsters/population.md` §1 r2 inside whichever step runs them; the
/// room stays in its act list, so the next step 3 visits it again (one
/// more ambient call, then nothing).
pub fn populate_room<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H, room: RoomId) {
    room_body(game, hooks, room);
}

/// §4 rules 1–3 for one room.
fn room_body<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H, room: RoomId) {
    hooks.ambient_spawns(game, room);
    let (populated, active) = match game.lists.room(room) {
        Some(r) => (r.populated, r.units_active),
        None => (true, true),
    };
    if !populated {
        hooks.spawn_presets(game, room);
        hooks.restore_inactive_units(game, room);
        hooks.populate_objects(game, room);
        hooks.populate_monsters(game, room);
        if let Some(r) = game.lists.room_mut(room) {
            r.populated = true;
            r.units_active = true;
        }
    } else if !active {
        hooks.restore_inactive_units(game, room);
        if let Some(r) = game.lists.room_mut(room) {
            r.units_active = true;
        }
    }
}

/// Step 4 (`0x005414D0`, §5.5): run the timer queue.
pub fn run_timer_events<D: EventDispatch + ?Sized>(game: &mut Game, dispatch: &mut D) {
    let frame = game.frame;
    game.timers.begin_run(frame);
    for class in TimerClass::RUN_ORDER {
        for list in [TimerList::EveryTick, TimerList::Due] {
            game.timers.begin_list(class, list);
            while let Some(run) = game.timers.next_run(list, frame) {
                dispatch.run_event(game, &run);
                game.timers.finish_run(&run);
            }
        }
    }
}

/// Step 5, the client pass (`0x0052D440`, §6). The arena assert and the
/// heartbeat are host-only (§8). The 8192-frame save (rule 3) runs in
/// single player too (no heartbeat drop is modelled, so only the frame
/// decides): it is raised as [`Game::character_save_due`], before the
/// per-client loop, for the host's character storage to write.
fn client_pass<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    if is_due(game.frame, period::CHARACTER_SAVE) {
        game.character_save_due = true;
    }
    let mut cur = game.lists.client_first();
    while let Some(c) = cur {
        // Next saved before the body (unit-order.md §7.3).
        cur = game.lists.client_next(c);
        let Some(state) = game.lists.client(c).map(|e| e.state) else {
            continue;
        };
        match state {
            client_state::JOINING | client_state::CHANGING_ACT => {
                client_update(game, hooks, c);
                if hooks.client_room_ready(game, c) {
                    hooks.send_load_complete(game, c);
                    if let Some(e) = game.lists.client_mut(c) {
                        e.state = client_state::IN_GAME;
                    }
                    hooks.refresh_inventory(game, c);
                    if state == client_state::JOINING {
                        hooks.join_sequence(game, c);
                    }
                }
            }
            client_state::IN_GAME => client_update(game, hooks, c),
            _ => {}
        }
    }
}

/// The per-client update (`0x005380D0`, §6.5).
fn client_update<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H, client: ClientId) {
    hooks.send_removed_units(game, client);
    let adjacent = game
        .lists
        .client(client)
        .and_then(|e| e.room)
        .and_then(|r| game.lists.room(r))
        .map(|r| r.adjacent.clone())
        .unwrap_or_default();
    for room in adjacent {
        let mut cur = game.lists.update_first(room);
        while let Some(u) = cur {
            // Next saved before the body (unit-order.md §10).
            cur = game.lists.update_next(u);
            hooks.send_unit_update(game, client, u);
        }
    }
    hooks.client_update_messages(game, client);
    let Some(e) = game.lists.client_mut(client) else {
        return;
    };
    e.update_count = e.update_count.wrapping_add(1);
    let (player, client_room) = (e.player, e.room);
    let player_room = player
        .and_then(|p| game.lists.unit(p))
        .and_then(|u| u.room());
    if player_room != client_room {
        hooks.client_level_change(game, client);
    }
    hooks.arena_sync(game, client);
    if let Some(p) = game.lists.client(client).and_then(|e| e.player) {
        if game.lists.unit(p).is_some() {
            // Cannot fail: the unit exists.
            let _ = game.lists.queue_update(p);
        }
    }
}

/// Step 6 (`0x0053B000`): walk and clear the update queues of acts with
/// pending updates.
fn room_update_queues<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    for act in 0..ACTS as u8 {
        if !game.lists.act(act).is_some_and(|a| a.pending_updates) {
            continue;
        }
        let mut room_cur = game.lists.room_first(act);
        while let Some(room) = room_cur {
            let mut cur = game.lists.update_first(room);
            while let Some(u) = cur {
                // TODO(handoff open question T2): the spec does not say
                // whether 0x00553220 can change the queue; the next link
                // is saved before the body, as for the client-update walk.
                cur = game.lists.update_next(u);
                hooks.unit_update(game, u);
            }
            // Cannot fail: the room is in the act list.
            let _ = game.lists.clear_update_queue(room);
            room_cur = game.lists.room_next(room);
        }
        if let Some(a) = game.lists.act_mut(act) {
            a.pending_updates = false;
        }
    }
    hooks.clear_arena_flag(game);
}

/// Step 7 (`0x0053A820`).
fn removal_records<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    for act in 0..ACTS as u8 {
        if !game.lists.act(act).is_some_and(|a| a.pending_removals) {
            continue;
        }
        let mut cur = game.lists.room_first(act);
        while let Some(room) = cur {
            hooks.free_removal_records(game, room);
            cur = game.lists.room_next(room);
        }
        if let Some(a) = game.lists.act_mut(act) {
            a.pending_removals = false;
        }
    }
}

/// Step 9 (`0x0052D240`): deactivate rooms inactive for more than 10
/// counts where the act allows it.
fn room_deactivation<H: TickHooks + ?Sized>(game: &mut Game, hooks: &mut H) {
    for act in 0..ACTS as u8 {
        if game.lists.act(act).is_none() {
            continue;
        }
        let mut cur = game.lists.room_first(act);
        while let Some(room) = cur {
            // Next saved before the body (unit-order.md §10).
            cur = game.lists.room_next(room);
            if hooks.room_inactivity(game, room) > ROOM_DEACTIVATION_THRESHOLD
                && hooks.act_allows_room_removal(game, act, room)
            {
                let mut unit_cur = game.lists.room_unit_first(room);
                while let Some(u) = unit_cur {
                    unit_cur = game.lists.room_unit_next(u);
                    hooks.compress_unit(game, u);
                }
                // `0x0061A910` removes the room from the act list; what
                // else it frees belongs to the DRLG spec.
                let _ = game.lists.deactivate_room(room);
                hooks.room_deactivated(game, act, room);
            }
        }
    }
}

#[cfg(test)]
mod gaps_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_fsim;
#[cfg(test)]
mod tests_lsim;
