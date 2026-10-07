// Spec: specs/world/vendors-2.md §10.1, §10.2, §10.4
//! C→S 0x4F ClickButton outside the cube and player trade: the dispatch
//! `0x00568060` (§10.1) and the stash buttons `0x00564D50` (§10.2:
//! close, gold withdraw, gold deposit, the clamped add `0x0053FF00`).
//!
//! The cube's buttons (0x17, 0x18) are [`crate::world::cube`]'s; the
//! player-trade switch (§10.3) has no owner spec, so the dispatch only
//! routes to it ([`ButtonRoute::Trade`]).

use crate::units::UnitId;

/// Button 0x12: stash close.
pub const BUTTON_STASH_CLOSE: u16 = 0x12;
/// Button 0x13: withdraw gold from the stash.
pub const BUTTON_WITHDRAW: u16 = 0x13;
/// Button 0x14: deposit gold into the stash.
pub const BUTTON_DEPOSIT: u16 = 0x14;
/// Buttons 0x17 (close) and 0x18 (transmute): the cube (`cube.md` §1).
pub const BUTTON_CUBE_CLOSE: u16 = 0x17;
pub const BUTTON_CUBE_TRANSMUTE: u16 = 0x18;

/// Interaction type 2: an object (the stash).
pub const INTERACT_OBJECT: u8 = 2;
/// Interaction type 0: a player (trade).
pub const INTERACT_PLAYER: u8 = 0;
/// The stash object class 0x10B (267, `world/objects-2.md` §16.10).
pub const STASH_CLASS: u32 = 0x10B;
/// The stash cap `0x00623460`: a constant in 1.14d (§10.2 rule 3).
pub const STASH_CAP: i32 = 2_500_000;
/// Carried gold per character level (`0x00622E70`: level × 10000).
pub const GOLD_PER_LEVEL: i32 = 10_000;
/// Sound event 19, `impossible` (`audio/triggers-2.md` §14 rule 3).
pub const SOUND_IMPOSSIBLE: u16 = 19;

/// Stats read and written (`itemstatcost`).
pub mod stat {
    pub const LEVEL: u16 = 12;
    pub const GOLD: u16 = 14;
    pub const GOLDBANK: u16 = 15;
}

/// S→C 0x77 TradeAction codes of the dispatch (`client/msg-ui.md` §3).
pub mod trade_code {
    /// No active interaction (close trade).
    pub const CLOSE: u8 = 0x0C;
    /// Another button with a non-trade interaction (close trade(1)).
    pub const CLOSE_1: u8 = 0x0D;
}

/// The calls the stash buttons make, with their expected providers.
pub trait StashWorld {
    /// `0x00554100`: the active interaction (type, GUID); `None` when the
    /// active byte is 0 (unit record).
    fn interaction(&self, player: UnitId) -> Option<(u8, u32)>;
    /// `0x00554190`: GUID −1, type 6, inactive.
    fn reset_interaction(&mut self, player: UnitId);
    /// `0x0055FA40`: the scroll / tome recount (`items/inventory.md` §5.5).
    fn inventory_pass(&mut self, player: UnitId);
    /// A message to the player's client.
    fn send(&mut self, player: UnitId, msg: &[u8]);
    /// `0x00552F60(game, 2, guid)`: the object unit with this GUID and
    /// its class.
    fn object_by_guid(&self, guid: u32) -> Option<(UnitId, u32)>;
    /// `0x00620BB0` then `0x0061AB00`: the unit has a room and the room is
    /// in a town level.
    fn in_town(&self, unit: UnitId) -> bool;
    /// `0x00625480(unit, stat, 0)`: the full value.
    fn stat(&self, unit: UnitId, stat: u16) -> i32;
    /// `0x00627260(unit, stat, value, 0)`: base set.
    fn set_base(&mut self, unit: UnitId, stat: u16, value: i32);
    /// `0x006272B0(unit, stat, d, 0)`: base add.
    fn add_base(&mut self, unit: UnitId, stat: u16, d: i32);
    /// The unit is a player (type 0).
    fn is_player(&self, unit: UnitId) -> bool;
    /// `0x0055A090`: drop a gold pile at the player (Receive's overflow).
    fn drop_gold(&mut self, player: UnitId, amount: i32);
    /// `0x00553380(unit, event, target)` (`audio/triggers-2.md` §14).
    fn sound(&mut self, unit: UnitId, event: u16, target: Option<UnitId>);
}

/// Where the dispatch `0x00568060` sends a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRoute {
    /// Handled here: the handler result (`intents-events.md`: 0 and 1
    /// enqueue, 3 drops).
    Done(u32),
    /// Buttons 0x17 / 0x18 with an active interaction: the cube
    /// (`cube.md` §1, rule 4).
    Cube,
    /// Rule 5 with interaction type 0: the player-trade switch (§10.3,
    /// no owner spec).
    Trade,
}

/// `v` = (p1 << 16) | p2 of a C→S 0x4F [button u16@1][p1 u16@3][p2
/// u16@5] (§10 intro).
pub fn button_value(p1: u16, p2: u16) -> u32 {
    (u32::from(p1) << 16) | u32::from(p2)
}

/// The carried-gold cap `0x00622E70`: stat 12 × 10000.
pub fn gold_cap<W: StashWorld + ?Sized>(w: &W, player: UnitId) -> i32 {
    w.stat(player, stat::LEVEL).wrapping_mul(GOLD_PER_LEVEL)
}

/// `0x00568060(game, P, button, v)` (§10.1 rules 2–5) for the player P
/// (rule 1, no P, is the caller's: result 1).
pub fn click_button<W: StashWorld + ?Sized>(
    w: &mut W,
    player: UnitId,
    button: u16,
    v: u32,
) -> ButtonRoute {
    // Rule 2: before the button test.
    let Some((ty, guid)) = w.interaction(player) else {
        w.send(player, &[0x77, trade_code::CLOSE]);
        return ButtonRoute::Done(0);
    };
    match button {
        // Rule 3 (u16 compare `button − 0x12 ≤ 2`).
        BUTTON_STASH_CLOSE..=BUTTON_DEPOSIT => {
            if ty != INTERACT_OBJECT {
                return ButtonRoute::Done(1);
            }
            stash_button(w, player, guid, button, v as i32);
            ButtonRoute::Done(0)
        }
        // Rule 4.
        BUTTON_CUBE_CLOSE | BUTTON_CUBE_TRANSMUTE => ButtonRoute::Cube,
        // Rule 5.
        _ if ty != INTERACT_PLAYER => {
            w.send(player, &[0x77, trade_code::CLOSE_1]);
            ButtonRoute::Done(3)
        }
        _ => ButtonRoute::Trade,
    }
}

/// `0x00564D50` (§10.2): the common checks, then the button. `guid`: the
/// interaction's (type 2).
pub fn stash_button<W: StashWorld + ?Sized>(
    w: &mut W,
    player: UnitId,
    guid: u32,
    button: u16,
    v: i32,
) {
    let Some((stash, class)) = w.object_by_guid(guid) else {
        return;
    };
    if class != STASH_CLASS || !w.in_town(player) || !w.in_town(stash) {
        return;
    }
    match button {
        BUTTON_STASH_CLOSE => close(w, player),
        BUTTON_WITHDRAW => withdraw(w, player, v),
        BUTTON_DEPOSIT => deposit(w, player, v),
        _ => {}
    }
}

/// §10.2 rule 1: reset the interaction (active, type 2), then the
/// recount; nothing is sent.
fn close<W: StashWorld + ?Sized>(w: &mut W, player: UnitId) {
    if w.interaction(player)
        .is_some_and(|(ty, _)| ty == INTERACT_OBJECT)
    {
        w.reset_interaction(player);
    }
    w.inventory_pass(player);
}

/// §10.2 rule 2 (signed compares).
fn withdraw<W: StashWorld + ?Sized>(w: &mut W, player: UnitId, v: i32) {
    if v <= 0 || v > w.stat(player, stat::GOLDBANK) {
        return;
    }
    if w.stat(player, stat::GOLD).wrapping_add(v) > gold_cap(w, player) {
        w.sound(player, SOUND_IMPOSSIBLE, Some(player));
        return;
    }
    receive(w, player, v);
    clamped_add(w, player, stat::GOLDBANK, v.wrapping_neg());
}

/// §10.2 rule 3.
fn deposit<W: StashWorld + ?Sized>(w: &mut W, player: UnitId, v: i32) {
    if v <= 0 || v > w.stat(player, stat::GOLD) {
        return;
    }
    let cap = STASH_CAP;
    let s = w.stat(player, stat::GOLDBANK);
    if (s as u32).wrapping_add(v as u32) <= cap as u32 {
        clamped_add(w, player, stat::GOLD, v.wrapping_neg());
        clamped_add(w, player, stat::GOLDBANK, v);
    } else if (s as u32) < cap as u32 {
        let part = cap.wrapping_sub(s);
        clamped_add(w, player, stat::GOLDBANK, part);
        clamped_add(w, player, stat::GOLD, part.wrapping_neg());
    }
}

/// Receive `0x0055B060(player, a)` (`world/vendors.md` §9.1): at the cap
/// the whole amount drops; over it the gold is set to the cap and the
/// rest drops; else gold += a.
pub fn receive<W: StashWorld + ?Sized>(w: &mut W, player: UnitId, a: i32) {
    let cap = gold_cap(w, player);
    let g = w.stat(player, stat::GOLD);
    if g == cap {
        w.drop_gold(player, a);
    } else if (g as u32).wrapping_add(a as u32) > cap as u32 {
        w.set_base(player, stat::GOLD, cap);
        w.drop_gold(player, g.wrapping_add(a).wrapping_sub(cap));
    } else {
        w.set_base(player, stat::GOLD, g.wrapping_add(a));
    }
}

/// The clamped add `0x0053FF00(unit, stat, d)` (§10.2 rule 4).
pub fn clamped_add<W: StashWorld + ?Sized>(w: &mut W, unit: UnitId, id: u16, d: i32) {
    let n = w.stat(unit, id).wrapping_add(d);
    if n < 0 {
        w.set_base(unit, id, 0);
        return;
    }
    if w.is_player(unit) {
        let over = match id {
            stat::GOLD => n > gold_cap(w, unit),
            stat::GOLDBANK => n > STASH_CAP,
            _ => false,
        };
        if over {
            w.set_base(unit, id, 0);
            return;
        }
    }
    w.add_base(unit, id, d);
}

#[cfg(test)]
mod tests;
