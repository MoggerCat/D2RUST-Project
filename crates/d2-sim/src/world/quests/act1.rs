// Spec: specs/world/quests.md §10 (Act I), §10.1 (common pattern)
//! Act I quest callbacks, as far as §10 specifies them. Every callback,
//! status, active or sequence function that `quests.tsv` registers but §10
//! does not describe is reported through `QuestWorld::unhandled` with its
//! 1.14d address (open questions 7, 8).

use super::{
    bit, event, flags_of, grant_pending, npc, send_player_flags, EventArgs, GuidList, QuestControl,
    QuestFlags, QuestRecord, QuestWorld, TextList, TimerFn,
};
use crate::rng::Seed;
use crate::units::UnitId;

/// Object class of Wirt's body (0x10C).
pub const WIRT_BODY: u16 = 0x10C;
/// Paladin class id.
pub const PALADIN: u8 = 3;
/// Chipped gems (`0x007361DC`) and normal gems (`0x00736444`).
pub const CHIPPED_GEMS: [[u8; 4]; 7] = [
    *b"gcv ", *b"gcr ", *b"gcb ", *b"gcy ", *b"gcg ", *b"gcw ", *b"skc ",
];
pub const NORMAL_GEMS: [[u8; 4]; 7] = [
    *b"gsv ", *b"gsr ", *b"gsb ", *b"gsy ", *b"gsg ", *b"gsw ", *b"sku ",
];

/// Per-quest extra data (record +0x18), the fields §10 names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// A1Q1 +0x84: Den of Evil cleared.
    pub done: bool,
    /// A1Q1 +0x86: the start message was given (consumed by event 2).
    pub talked: bool,
    /// A1Q1 +0x87: the status timer exists.
    pub timer: bool,
    /// A1Q1: monsters left in level 8 (0x50 / 0x5D extra).
    pub monsters_left: u16,
    /// Player GUIDs (A1Q1 killers, A1Q6 lists).
    pub guids: GuidList,
    /// A1Q4: Cairn stone order, once computed.
    pub stone_order: Option<[u8; 5]>,
    /// A1Q4: gold piles left on Wirt's body.
    pub wirt_piles: Option<i32>,
    /// A1Q3: Malus drops counted.
    pub malus_count: u8,
}

/// Per-record init beyond the `quests.tsv` columns (§10.4).
pub fn init(r: &mut QuestRecord) {
    // TODO(quests §2.3): the active / state bytes the other init functions
    // store are not in `quests.tsv` or §10; they stay 0.
    if r.chain == 1 {
        r.active = true;
        r.state = 1;
    }
}

/// `0x005901E0`: Den of Evil monsters left.
pub fn den_monsters_left(r: &QuestRecord) -> u16 {
    r.extra.monsters_left
}

/// Sequence function of `chain` (§3 step 3, §10.1 "1"): the quest named
/// by its `seq_id` reaches state 1.
pub fn sequence<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, chain: u8) {
    let Some(r) = ctl.record(chain) else { return };
    let (seq_fn, seq_id) = (r.seq_fn, r.seq_id);
    if !(1..=6).contains(&chain) {
        w.unhandled(chain, seq_fn.unwrap_or(0));
        return;
    }
    // TODO(quests §10.1): the guard is not specified; d2rs only raises a
    // state 0 to 1, never lowers one.
    if let Some(next) = seq_id.and_then(|s| ctl.record_mut(s)) {
        if next.state == 0 {
            next.state = 1;
        }
    }
}

fn player_flags<W: QuestWorld>(w: &mut W, p: UnitId) -> QuestFlags {
    flags_of(w, p).copied().unwrap_or_default()
}

/// §10.1 state 2: the start message.
fn start<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    player: UnitId,
    npc: Option<UnitId>,
) {
    let slot = ctl.records[i].filter;
    ctl.records[i].state = 2;
    ctl.records[i].extra.talked = true;
    for p in w.players() {
        if let Some(f) = flags_of(w, p) {
            if !f.get(slot, bit::REWARD_GRANTED) && !f.get(slot, bit::REWARD_PENDING) {
                f.set(slot, bit::STARTED);
            }
        }
    }
    if let Some(n) = npc {
        ctl.refresh_text(w, player, n);
    }
}

/// §10.1: callback 13 restores state from the starting player's bits.
fn restore<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, player: UnitId) {
    let slot = ctl.records[i].filter;
    let f = player_flags(w, player);
    if f.get(slot, bit::REWARD_GRANTED) || f.get(slot, bit::COMPLETED_BEFORE) {
        return;
    }
    let r = &mut ctl.records[i];
    if f.get(slot, bit::ENTER_AREA) {
        (r.state, r.status) = (3, 2);
    } else if f.get(slot, bit::LEAVE_TOWN) {
        (r.state, r.status) = (3, 1);
    } else if f.get(slot, bit::STARTED) {
        (r.state, r.status) = (2, 1);
    }
}

/// §10.1: chat end after the start message sends status 1 to everyone.
fn chat_end<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize) {
    // TODO(quests §10.1): the gate is read as "start message given since
    // the last chat end" (A1Q1 +0x86); not stated for the other quests.
    if ctl.records[i].extra.talked {
        ctl.records[i].extra.talked = false;
        let chain = ctl.records[i].chain;
        let _ = ctl.set_status_all(w, chain, 1);
    }
}

/// Event 3 for the area quests (§10.4; A1Q2 uses the same shape with
/// D2MOO's area level 17).
fn area_event<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    args: EventArgs,
    area: u32,
) {
    let Some(p) = args.player else { return };
    let slot = ctl.records[i].filter;
    let state = ctl.records[i].state;
    // TODO(quests §10.1): which players get bits 3/4 is not specified;
    // d2rs sets them on the moving player when it has started the quest.
    let mark = |w: &mut W, b: u8| {
        if let Some(f) = flags_of(w, p) {
            if f.get(slot, bit::STARTED)
                && !f.get(slot, bit::REWARD_GRANTED)
                && !f.get(slot, bit::REWARD_PENDING)
            {
                f.set(slot, b);
            }
        }
    };
    if args.b == area && (state == 1 || state == 2) {
        ctl.records[i].state = 3;
        mark(w, bit::ENTER_AREA);
    } else if args.a == 1 && state == 2 {
        ctl.records[i].state = 3;
        mark(w, bit::LEAVE_TOWN);
    }
}

/// Dispatches one callback of record `i`.
pub fn callback<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    args: EventArgs,
    list: Option<&mut TextList>,
    force: bool,
) {
    let chain = ctl.records[i].chain;
    let ev = args.event;
    let handled = match (chain, ev) {
        (0, event::NPC_ACTIVATE) => warriv_gossip_text(ctl, w, i, args, list),
        (0, event::SCROLL_MESSAGE) => {
            if args.a == u32::from(npc::WARRIV1) && args.b <= 1 {
                if let Some(f) = args.player.and_then(|p| flags_of(w, p)) {
                    f.set(0, bit::REWARD_GRANTED);
                }
            }
            true
        }
        (25 | 30, event::NPC_ACTIVATE) => flavie_text(ctl, w, i, args, list),
        // A bare `ret` (`0x00596BA0`).
        (25 | 30, event::MONSTER_KILLED) => true,
        (1 | 2 | 5 | 6, event::PLAYER_STARTED_GAME) => {
            if let Some(p) = args.player {
                restore(ctl, w, i, p);
            }
            true
        }
        (1 | 2 | 3 | 4 | 6, event::NPC_DEACTIVATE) => {
            chat_end(ctl, w, i);
            true
        }
        (1, event::CHANGED_LEVEL) => {
            area_event(ctl, w, i, args, 8);
            true
        }
        (2, event::CHANGED_LEVEL) => {
            // TODO(quests §10.5): area level 17 is D2MOO's.
            area_event(ctl, w, i, args, 17);
            true
        }
        (1, event::MONSTER_KILLED) => {
            den_kill(ctl, w, i, args);
            true
        }
        (2, event::MONSTER_KILLED) => {
            blood_raven_kill(ctl, w, i, args);
            true
        }
        (4, event::MONSTER_KILLED) => {
            cow_king_kill(ctl, w, args);
            true
        }
        (6, event::MONSTER_KILLED) => {
            andariel_kill(ctl, w, i, args);
            true
        }
        (_, event::SCROLL_MESSAGE) if (1..=6).contains(&chain) => scroll(ctl, w, i, args),
        _ => false,
    };
    let _ = force;
    if !handled {
        let f = ctl
            .rows
            .iter()
            .find(|r| r.chain == chain)
            .and_then(|r| r.callbacks.iter().find(|c| c.0 == ev))
            .map_or(0, |c| c.1);
        w.unhandled(chain, f);
    }
}

/// A1Q0 event 0 (`0x0058FAD0`, §10.3).
fn warriv_gossip_text<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    args: EventArgs,
    list: Option<&mut TextList>,
) -> bool {
    let (Some(p), Some(n), Some(list)) = (args.player, args.target, list) else {
        return true;
    };
    if w.monster_class(n) != Some(npc::WARRIV1) || player_flags(w, p).get(0, bit::REWARD_GRANTED) {
        return true;
    }
    let state = u8::from(w.player_class(p) == PALADIN);
    if let Some(t) = ctl.records[i].msgs {
        ctl.add_messages(t, list, npc::WARRIV1, state);
    }
    true
}

/// Flavie's event 0 (`0x00596A90`, §10.3; edge case 2: both records).
fn flavie_text<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    args: EventArgs,
    list: Option<&mut TextList>,
) -> bool {
    let (Some(p), Some(n)) = (args.player, args.target) else {
        return true;
    };
    if w.monster_class(n) != Some(npc::NAVI) {
        return true;
    }
    let state = if flavie_open(ctl, w, p) {
        w.unit_seed(p).roll(2) as u8
    } else {
        (w.unit_seed(p).step() % 3) as u8 + 2
    };
    ctl.records[i].state = state;
    if let (Some(t), Some(list)) = (ctl.records[i].msgs, list) {
        ctl.add_messages(t, list, npc::NAVI, state);
    }
    true
}

/// Game slot 1 bit 13 clear, chain 1 not-intro, the player lacks slot 1
/// bits 0, 13, 1.
fn flavie_open<W: QuestWorld>(ctl: &QuestControl, w: &mut W, p: UnitId) -> bool {
    let f = player_flags(w, p);
    !ctl.game.get(1, bit::PRIMARY_GOAL_DONE)
        && ctl.record(1).is_some_and(|r| r.not_intro)
        && !f.get(1, bit::REWARD_GRANTED)
        && !f.get(1, bit::PRIMARY_GOAL_DONE)
        && !f.get(1, bit::REWARD_PENDING)
}

/// Active functions (§6.4) §10 specifies; others are unhandled (false).
pub fn active_fn<W: QuestWorld>(
    ctl: &QuestControl,
    w: &mut W,
    i: usize,
    player: UnitId,
    npc_class: u16,
    f: u32,
) -> bool {
    match ctl.records[i].chain {
        0 => npc_class == npc::WARRIV1 && !player_flags(w, player).get(0, bit::REWARD_GRANTED),
        25 | 30 => npc_class == npc::NAVI && flavie_open(ctl, w, player),
        c => {
            w.unhandled(c, f);
            false
        }
    }
}

/// Status functions (§6.1). A1Q0 and the intros return false; others
/// are unhandled (nothing reported).
pub fn status_fn<W: QuestWorld>(
    ctl: &QuestControl,
    w: &mut W,
    i: usize,
    _player: UnitId,
    _pf: &QuestFlags,
    f: u32,
) -> Option<u8> {
    match ctl.records[i].chain {
        0 | 37..=40 => None,
        c => {
            w.unhandled(c, f);
            None
        }
    }
}

/// Runs a timer callback; true = remove the timer.
pub fn run_timer<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    func: TimerFn,
    chain: u8,
) -> bool {
    match func {
        TimerFn::DenOfEvilStatus => {
            if ctl.record(chain).is_some_and(|r| r.state == 4) {
                let _ = ctl.set_status_all(w, chain, 5);
            }
            true
        }
        #[cfg(test)]
        TimerFn::Probe => {
            w.unhandled(chain, ctl.tick);
            false
        }
    }
}

// ----------------------------------------------------------- event 11

/// Scroll messages of chains 1–6. Returns false when unhandled.
fn scroll<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, args: EventArgs) -> bool {
    let Some(p) = args.player else { return true };
    let chain = ctl.records[i].chain;
    let (npc_class, msg) = (args.a as u16, args.b);
    let n = args.target;
    let f = player_flags(w, p);
    match (chain, msg) {
        (1, 64) if npc_class == npc::AKARA => start(ctl, w, i, p, n),
        (1, 76) if f.get(1, bit::REWARD_PENDING) => den_reward(ctl, w, i, p, n),
        (2, 81) if npc_class == npc::KASHYA => start(ctl, w, i, p, n),
        (2, 92) if npc_class == npc::KASHYA && f.get(2, bit::REWARD_PENDING) => {
            if let Some(fl) = flags_of(w, p) {
                fl.set(2, bit::REWARD_GRANTED);
                fl.clear(2, bit::REWARD_PENDING);
            }
            ctl.records[i].state = 5;
            w.mercenary_reward(p, npc::KASHYA);
        }
        (3, 146) if npc_class == npc::CHARSI => start(ctl, w, i, p, n),
        (3, 163) if w.has_item(p, *b"hdm ") => {
            if let Some(fl) = flags_of(w, p) {
                fl.set(3, bit::PRIMARY_GOAL_DONE);
                fl.set(3, bit::REWARD_PENDING);
            }
            w.delete_item(p, *b"hdm ");
            // Party members' bits (OQ7).
            w.unhandled(3, 0x0059_1490);
            ctl.records[i].state = 5;
            ctl.game.set(3, bit::PRIMARY_GOAL_DONE);
            sequence(ctl, w, 3);
        }
        (4, 97) if npc_class == npc::AKARA => start(ctl, w, i, p, n),
        (4, 112) if w.has_item(p, *b"bks ") => {
            w.delete_item(p, *b"bks ");
            w.reward_item(p, *b"bkd ", 0, 2, true);
            ctl.records[i].state = 5;
        }
        (4, 118) if f.get(4, bit::REWARD_PENDING) => {
            if let Some(fl) = flags_of(w, p) {
                fl.set(4, bit::REWARD_GRANTED);
                fl.clear(4, bit::REWARD_PENDING);
            }
            send_player_flags(w, p, 6, 0);
            let (level, quality) = match w.difficulty() {
                0 => (7, 4),
                1 => (30, 6),
                _ => (60, 6),
            };
            w.reward_item(p, *b"rin ", level, quality, true);
            w.send(p, &[0x5D, 4, 2, 0, 0, 0]);
        }
        (5, 127) => start(ctl, w, i, p, None),
        (5, 140..=145) => {
            // TODO(quests §10.7): D2MOO-derived; d2rs moves state 4 to 5
            // and sets no bits.
            if ctl.records[i].state == 4 {
                ctl.records[i].state = 5;
            }
        }
        (6, 166) if npc_class == npc::CAIN5 => start(ctl, w, i, p, n),
        (6, 183) if npc_class == npc::WARRIV1 && f.get(6, bit::REWARD_PENDING) => {
            if f.get(6, bit::PRIMARY_GOAL_DONE) {
                ctl.records[i].status = 13;
                ctl.records[i].state = 5;
                ctl.game.set(6, bit::PRIMARY_GOAL_DONE);
            }
            if let Some(fl) = flags_of(w, p) {
                fl.clear(6, bit::REWARD_PENDING);
                fl.set(6, bit::REWARD_GRANTED);
            }
            send_player_flags(w, p, 6, 0);
        }
        (6, 179) if npc_class == npc::AKARA => ctl.records[i].extra.guids.remove(w.guid(p)),
        (6, 181) if npc_class == npc::KASHYA => ctl.records[i].extra.guids.remove(w.guid(p)),
        (6, 184) if npc_class == npc::CAIN5 => ctl.records[i].extra.guids.remove(w.guid(p)),
        _ => {}
    }
    true
}

/// A1Q1 message 76 (§10.4).
fn den_reward<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    p: UnitId,
    n: Option<UnitId>,
) {
    let f = player_flags(w, p);
    if f.get(1, bit::PRIMARY_GOAL_DONE) && ctl.records[i].state != 5 {
        ctl.records[i].state = 5;
        sequence(ctl, w, 1);
        let r = &mut ctl.records[i];
        r.flags = 0;
        r.status = 13;
        r.clear_callback(event::NPC_DEACTIVATE);
    }
    if let Some(fl) = flags_of(w, p) {
        fl.set(1, bit::REWARD_GRANTED);
        fl.clear(1, bit::REWARD_PENDING);
        fl.set(41, bit::PRIMARY_GOAL_DONE);
        fl.set(41, bit::REWARD_PENDING);
        fl.reset_progress(1);
    }
    w.add_stat(p, 5, 1);
    let g = w.guid(p);
    ctl.records[i].guids.add(g);
    if let Some(n) = n {
        ctl.refresh_text(w, p, n);
    }
}

// ------------------------------------------------------------ event 8

/// A1Q1 event 8 (`0x00590260`, §10.4).
fn den_kill<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, args: EventArgs) {
    if !ctl.records[i].not_intro {
        return;
    }
    let (spawn, kills, rooms, populated) = w.den_region();
    let left = spawn.saturating_sub(kills) as u16;
    ctl.records[i].extra.monsters_left = left;
    if let Some(p) = args.player {
        let g = w.guid(p);
        ctl.records[i].extra.guids.add(g);
    }
    if populated <= rooms && kills == spawn {
        let r = &mut ctl.records[i];
        r.extra.done = true;
        r.clear_callback(event::NPC_DEACTIVATE);
        r.clear_callback(event::MONSTER_KILLED);
        r.state = 4;
        ctl.game.set(1, bit::PRIMARY_GOAL_DONE);
        let list = ctl.records[i].extra.guids.clone();
        grant_pending(w, &list, 1, 0);
        // TODO(quests OQ7): party goal `0x00590190`, COMPLETEDNOW + log
        // update `0x00590080` and the sound `0x005900E0` per player.
        for _ in w.players() {
            w.unhandled(1, 0x0059_0190);
            w.unhandled(1, 0x0059_0080);
            w.unhandled(1, 0x0059_00E0);
        }
        ctl.unique_event(w, 0);
        if !ctl.records[i].extra.timer {
            ctl.records[i].extra.timer = true;
            let _ = ctl.add_timer(1, TimerFn::DenOfEvilStatus, 8);
        }
    } else if (populated <= rooms && left < 6) || (ctl.records[i].status == 4 && left > 5) {
        ctl.records[i].flags = 0x20;
        let _ = ctl.set_status_all(w, 1, 4);
    }
}

/// A1Q2 event 8 (§10.5): Blood Raven's kill.
fn blood_raven_kill<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, args: EventArgs) {
    let Some(victim) = args.target else { return };
    for p in w.players_near(victim) {
        if let Some(f) = flags_of(w, p) {
            if !f.get(2, bit::REWARD_GRANTED) && !f.get(2, bit::REWARD_PENDING) {
                f.set(2, bit::PRIMARY_GOAL_DONE);
                f.set(2, bit::REWARD_PENDING);
                w.attach_sound(p, 34);
            }
        }
    }
    ctl.records[i].state = 4;
    // TODO(quests §10.5): timer 15's callback is not specified; no timer.
    w.unhandled(2, 0x0059_0EC0);
}

/// A1Q4 event 8 (`0x00593E70`, §10.6): the Cow King.
fn cow_king_kill<W: QuestWorld>(_ctl: &mut QuestControl, w: &mut W, args: EventArgs) {
    for p in w.players() {
        if w.unit_level(p) == Some(39) {
            if let Some(f) = flags_of(w, p) {
                f.set(4, 10);
            }
        }
    }
    if let Some(v) = args.target {
        for _ in 0..8 {
            w.drop_item_at(v, *b"vps ", 2);
        }
    }
}

/// The gem a quest-seed draw picks (§10.8).
pub fn gem_code(list: &[[u8; 4]; 7], lo: u32) -> [u8; 4] {
    list[(lo % 7) as usize]
}

/// A1Q6 event 8 (`0x005965A0`, §10.8).
fn andariel_kill<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, args: EventArgs) {
    ctl.records[i].clear_callback(event::NPC_DEACTIVATE);
    if let (Some(p), Some(v)) = (args.player, args.target) {
        let f = player_flags(w, p);
        if !f.get(6, bit::REWARD_GRANTED) && !f.get(6, bit::REWARD_PENDING) {
            w.unhandled(6, 0x0059_6210);
            for list in [&CHIPPED_GEMS, &CHIPPED_GEMS, &NORMAL_GEMS] {
                let lo = ctl.seed.step();
                w.drop_item_at(v, gem_code(list, lo), 2);
            }
        }
    }
    // TODO(quests §10.8): the per-player iterates and the period-1 timer's
    // callback are not specified.
    w.unhandled(6, 0x0059_65A0);
    ctl.records[i].state = 4;
}

// ------------------------------------------------------- objects, items

/// The Cairn stone order from successive draws (§10.6).
pub fn stone_order_from(mut lo: impl FnMut() -> u32) -> [u8; 5] {
    let mut order = [0u8; 5];
    let mut i = 0;
    while i < 5 {
        let k = (lo() % 5) as usize;
        if order[k] == 0 {
            order[k] = 17 + i;
            i += 1;
        }
    }
    order
}

/// `0x00592E90`: the stone order on the quest seed, computed once.
pub fn stone_order(ctl: &mut QuestControl) -> [u8; 5] {
    let Some(i) = ctl.find(4) else {
        return [0; 5];
    };
    if let Some(o) = ctl.records[i].extra.stone_order {
        return o;
    }
    let seed: &mut Seed = &mut ctl.seed;
    let o = stone_order_from(|| seed.step());
    ctl.records[i].extra.stone_order = Some(o);
    o
}

/// `0x00593CB0` (§9.4): read the deciphered scroll.
pub fn send_stone_order<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, _player: UnitId) {
    stone_order(ctl);
    // TODO(quests OQ5): the 0x50 layout before the five values is open.
    w.unhandled(4, 0x0059_3CB0);
}

/// Wirt's body, object event 7 (§10.6).
pub fn wirt_body<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let Some(i) = ctl.find(4) else { return };
    let piles = match ctl.records[i].extra.wirt_piles {
        Some(n) => n,
        None => ctl.seed.roll_range(10, 10),
    };
    let mut piles = piles;
    if piles > 0 && w.drop_item_at(object, *b"gld ", 2) {
        piles -= 1;
        if piles > 0 {
            let at = w.frame() + 10;
            w.schedule_quest_event(object, at);
        }
    }
    ctl.records[i].extra.wirt_piles = Some(piles);
}

/// The Malus object (operate `0x00591AC0`, §10.5).
pub fn malus_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
) {
    let Some(i) = ctl.find(3) else { return };
    let f = player_flags(w, player);
    if !ctl.records[i].not_intro || f.get(3, bit::REWARD_GRANTED) || f.get(3, bit::REWARD_PENDING) {
        return;
    }
    if w.stat(player, 12) >= 8 {
        w.drop_item_at(object, *b"hdm ", 2);
        w.set_object_opened(object);
        let r = &mut ctl.records[i];
        r.extra.malus_count = r.extra.malus_count.wrapping_add(1);
        r.state = 4;
    } else {
        // TODO(quests §10.5): the sound id is not specified.
        w.unhandled(3, 0x0059_1AC0);
    }
}

/// `0x00591790`: the imbue was granted (set 3.0, clear 3.1).
pub fn imbue_granted<W: QuestWorld>(w: &mut W, player: UnitId) {
    if let Some(f) = flags_of(w, player) {
        f.set(3, bit::REWARD_GRANTED);
        f.clear(3, bit::REWARD_PENDING);
    }
}

/// `0x0058FD20`: offer the respec (set 41.13, 41.1).
pub fn respec_offer<W: QuestWorld>(w: &mut W, player: UnitId) {
    if let Some(f) = flags_of(w, player) {
        f.set(41, bit::PRIMARY_GOAL_DONE);
        f.set(41, bit::REWARD_PENDING);
    }
}

/// `0x0058FD50`: after the respec (set 41.0, clear 41.1; if 41.15 is
/// clear, a record's active byte is cleared).
pub fn respec_done<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, player: UnitId) {
    let Some(f) = flags_of(w, player) else { return };
    f.set(41, bit::REWARD_GRANTED);
    f.clear(41, bit::REWARD_PENDING);
    if !f.get(41, bit::COMPLETED_BEFORE) {
        // TODO(quests §10.3): "a record" is read as the respec record
        // (chain 30, flag slot 41).
        if let Some(r) = ctl.record_mut(30) {
            r.active = false;
        }
    }
}
