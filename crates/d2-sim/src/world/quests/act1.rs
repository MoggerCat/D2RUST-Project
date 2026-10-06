// Spec: specs/world/quests.md §10 (Act I), §10.1 (common pattern, sequence functions)
//! Act I quest callbacks, as far as §10 specifies them. A1Q1–A1Q3 are
//! callback by callback ([`q1`], [`q2`], [`q3`]); the shared shorthands
//! of §10.1 (broadcast, every player, add state, the sequence walk) are
//! here. Every callback, status, active or sequence function that
//! `quests.tsv` registers but §10 does not describe is reported through
//! `QuestWorld::unhandled` with its 1.14d address (open questions 7, 8).

pub mod q1;
pub mod q2;
pub mod q3;

pub use q3::{imbue_granted, malus_init, malus_operate};

use super::{
    bit, event, flags_of, npc, send_player_flags, EventArgs, GuidList, QuestControl, QuestError,
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

/// Per-quest extra data (record +0x18), the fields §10 names. One
/// struct for every Act I record; each quest uses its own fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// A1Q1 +0x84: Den of Evil cleared (written, never read).
    pub done: bool,
    /// A1Q1 +0x85: level 8 entered (written, never read).
    pub entered: bool,
    /// The start message was given, chat end not yet handled (A1Q1
    /// +0x86, A1Q2 +3, A1Q3 +0x02).
    pub talked: bool,
    /// A1Q1 +0x87: the status timer is pending.
    pub timer: bool,
    /// A1Q1 +0x88: monsters left in level 8 (0x50 / 0x5D extra).
    pub monsters_left: i32,
    /// Player GUIDs: A1Q1 killers (+0x00), A1Q3 players who brought the
    /// Malus (+0x14), A1Q6 lists.
    pub guids: GuidList,
    /// A1Q2 +0: Blood Raven killed.
    pub killed: bool,
    /// A1Q2 +1 and +2 (both set on the kill; +2 gates J3).
    pub kill_b1: bool,
    pub kill_b2: bool,
    /// A1Q2 +4 (set to 1 on the kill, never read by chain 2).
    pub kill_d4: u32,
    /// A1Q2 +8: the victim's GUID.
    pub victim: u32,
    /// A1Q3 +0x01: the Malus object is known.
    pub malus_known: bool,
    /// A1Q3 +0x03: message 163 finished the quest, chat end not yet
    /// handled.
    pub rewarded: bool,
    /// A1Q3 +0x04: the Malus object's GUID.
    pub malus_guid: u32,
    /// A1Q3 +0x08 (cleared by the reset).
    pub b08: u8,
    /// A1Q3 +0x98: the Malus object mode to restore (2: taken).
    pub malus_mode: i32,
    /// A1Q3 +0x9C: Malus items in the game.
    pub malus_items: i32,
    /// A1Q3 +0xA1: a player started the game carrying `hdm `.
    pub started_with_malus: bool,
    /// A1Q4: Cairn stone order, once computed.
    pub stone_order: Option<[u8; 5]>,
    /// A1Q4: gold piles left on Wirt's body.
    pub wirt_piles: Option<i32>,
}

/// Per-record init beyond the `quests.tsv` columns (§10.4, §10.5).
pub fn init(r: &mut QuestRecord) {
    // TODO(quests §2.3): the active / state bytes the init functions of
    // chains 4–6 store are not in `quests.tsv` or §10; they stay 0.
    match r.chain {
        1 => {
            r.active = true;
            r.state = 1;
        }
        2 | 3 => {
            r.active = true;
            r.state = 0;
        }
        _ => {}
    }
}

/// `0x005901E0`: Den of Evil monsters left (the u16 field of 0x50 and
/// 0x5D carries its low 16 bits).
pub fn den_monsters_left(r: &QuestRecord) -> u16 {
    r.extra.monsters_left as u16
}

/// The sequence function of `chain` (record +0xF0, §10.1), called
/// with its own record: the own step, then the pass test, then the walk
/// to the `seq_id` record. Returns the function's result (1 = true).
pub fn sequence<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, chain: u8) -> bool {
    let Some(i) = ctl.find(chain) else {
        return false;
    };
    let r = &ctl.records[i];
    let (state, not_intro, seq_fn) = (r.state, r.not_intro, r.seq_fn);
    let pass = match chain {
        1 => 5,
        2..=4 => {
            if state == 0 && not_intro {
                ctl.records[i].state = 1;
                return true;
            }
            if chain == 4 {
                6
            } else {
                5
            }
        }
        5 => {
            if state < 2 && not_intro {
                return true;
            }
            5
        }
        6 => {
            if state == 0 && not_intro {
                // TODO(quests §10.8): the timer callback `0x00596580`
                // (period 20) is not specified; no timer is made.
                w.unhandled(6, 0x0059_6580);
            }
            return true;
        }
        _ => {
            w.unhandled(chain, seq_fn.unwrap_or(0));
            return false;
        }
    };
    if state != pass && not_intro {
        return true;
    }
    let Some(next) = ctl.records[i].seq_id.and_then(|s| ctl.find(s)) else {
        return false;
    };
    let next_chain = ctl.records[next].chain;
    if ctl.records[next].seq_fn.is_none() {
        ctl.faults.push(QuestError::NoSequenceFn(next_chain));
        return false;
    }
    sequence(ctl, w, next_chain)
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

/// R: the event player's current-difficulty record (zero when there is
/// no player record, §10.1).
fn rec<W: QuestWorld>(w: &mut W, p: Option<UnitId>) -> QuestFlags {
    p.map_or_else(QuestFlags::default, |p| player_flags(w, p))
}

/// "add state k" (§10.1, §7.1) for the NPC `target`, if it has a class.
fn add_state<W: QuestWorld>(
    ctl: &QuestControl,
    w: &W,
    i: usize,
    list: Option<&mut TextList>,
    target: Option<UnitId>,
    k: u8,
) {
    let class = target.and_then(|n| w.monster_class(n));
    if let (Some(t), Some(list), Some(c)) = (ctl.records[i].msgs, list, class) {
        ctl.add_messages(t, list, c, k);
    }
}

/// The status iterate of chains 1–3 (I1, J1, K1, §10.4): 0x5D to `p` if
/// it has neither bit 0 nor 15 of the record's slot, or has 13 or 14.
fn status_iterate<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, p: UnitId) {
    let (chain, slot) = (ctl.records[i].chain, ctl.records[i].filter);
    let f = player_flags(w, p);
    if (!f.get(slot, bit::REWARD_GRANTED) && !f.get(slot, bit::COMPLETED_BEFORE))
        || f.get(slot, bit::PRIMARY_GOAL_DONE)
        || f.get(slot, bit::COMPLETED_NOW)
    {
        if let Err(e) = ctl.send_status(w, p, chain) {
            ctl.faults.push(e);
        }
    }
}

/// status(S) with the status iterate for every player (`0x00544300`
/// with iterate 1), flags as they are.
fn status_all<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, status: u8) {
    ctl.records[i].status = status;
    for p in w.players() {
        status_iterate(ctl, w, i, p);
    }
}

/// broadcast(S, f) (§10.1) for chains 1–3.
fn broadcast<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, status: u8, flags: u8) {
    ctl.records[i].flags = flags;
    status_all(ctl, w, i, status);
}

/// `0x00545920(player, chain, act)` (§6.3): `5D chain 00 0C 0000` when
/// the player has no room, or its room's level is ≠ 0 and in an act ≥
/// `act`.
fn send_completed_now<W: QuestWorld>(w: &mut W, p: UnitId, chain: u8, act: u8) {
    let send = match w.unit_level(p) {
        None => true,
        Some(0) => false,
        Some(_) => w.unit_act(p).is_none_or(|a| a >= act),
    };
    if send {
        w.send(p, &[0x5D, chain, 0, 12, 0, 0]);
    }
}

/// `0x00590120` (I3's member step) and A1Q2's J4 (§10.4, §10.5): a
/// member with neither bit 0 nor 1 of `slot` and a room whose level is ≠
/// 0 and in Act I gets 13, then 1 (no 0x28).
fn member_goal<W: QuestWorld>(w: &mut W, m: UnitId, slot: u8) {
    let in_act1 = w.unit_level(m).is_some_and(|l| l != 0) && w.unit_act(m) == Some(0);
    let Some(f) = flags_of(w, m) else { return };
    if !f.get(slot, bit::REWARD_GRANTED) && !f.get(slot, bit::REWARD_PENDING) && in_act1 {
        f.set(slot, bit::PRIMARY_GOAL_DONE);
        f.set(slot, bit::REWARD_PENDING);
    }
}

/// I3 / J7 (§10.4, §10.5): if `p` has bit 13 of `slot` and a party, the
/// member step for each member.
fn party_goal<W: QuestWorld>(w: &mut W, p: UnitId, slot: u8) {
    if !player_flags(w, p).get(slot, bit::PRIMARY_GOAL_DONE) {
        return;
    }
    if let Some(members) = w.party_members(p) {
        for m in members {
            member_goal(w, m, slot);
        }
    }
}

/// The value `m` of a state's message-state table (§10.4, §10.5):
/// `None` for −1 or a state past the table.
fn table_state(table: &[i8], state: u8) -> Option<u8> {
    match table.get(usize::from(state)) {
        Some(&m) if m >= 0 => Some(m as u8),
        _ => None,
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
        (1, _) => q1::callback(ctl, w, i, args, list),
        (2, _) => q2::callback(ctl, w, i, args, list),
        (3, _) => q3::callback(ctl, w, i, args, list),
        (5 | 6, event::PLAYER_STARTED_GAME) => {
            if let Some(p) = args.player {
                restore(ctl, w, i, p);
            }
            true
        }
        (4 | 6, event::NPC_DEACTIVATE) => {
            chat_end(ctl, w, i);
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
        (_, event::SCROLL_MESSAGE) if (4..=6).contains(&chain) => scroll(ctl, w, i, args),
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
        1 => q1::active(ctl, w, i, player, npc_class),
        2 => q2::active(ctl, w, i, player, npc_class),
        3 => q3::active(ctl, w, i, player, npc_class),
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
    player: UnitId,
    pf: &QuestFlags,
    f: u32,
) -> Option<u8> {
    match ctl.records[i].chain {
        0 | 37..=40 => None,
        3 => Some(q3::status(ctl, w, i, player, pf)),
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
            if let Some(i) = ctl.find(chain) {
                q1::timer(ctl, w, i);
            }
            true
        }
        TimerFn::BurialStatus => {
            if let Some(i) = ctl.find(chain) {
                broadcast(ctl, w, i, 3, 0);
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

/// Scroll messages of chains 4–6. Returns false when unhandled.
fn scroll<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize, args: EventArgs) -> bool {
    let Some(p) = args.player else { return true };
    let chain = ctl.records[i].chain;
    let (npc_class, msg) = (args.a as u16, args.b);
    let n = args.target;
    let f = player_flags(w, p);
    match (chain, msg) {
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

// ------------------------------------------------------------ event 8

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
