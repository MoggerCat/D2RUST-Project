// Spec: specs/client/bridge.md (§10)
//! Client outputs (§10): the 1.14d UI and sound calls an S→C handler
//! makes, carried out of the bridge as [`Output`]s. A handler appends
//! them to its message's sink ([`Outputs`]); the receive path moves them
//! to the bridge's list in message order, and the bridge hands the list
//! over whole at the end of the frame (§10 rule 4). [`dispatch`] applies
//! a list in order, each output to its one consumer (rule 5). Consumers
//! never write the model (rule 6).

use std::cell::RefCell;

use super::objects::{ObjFx, ObjSound};
use super::world::{RosterRecord, UnitKey};

/// One output: a 1.14d UI, sound or client-effect entry point with the
/// values it read when the handler ran (§10 rules 1, 3). Each variant is
/// owned by the spec of its producer ([`ROWS`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    /// S→C 0x2C (`audio/triggers.md` §2 r4): the event unit's key,
    /// class and position x, y at receive (§10 r3.1 (b); `None`: the
    /// unit has no position), and the event.
    ServerSound {
        unit: UnitKey,
        class: u32,
        at: Option<(u16, u16)>,
        event: u16,
    },
    /// S→C 0x5D (`client/msg-ui.md` §1): chain, flags, status, extra.
    QuestUi {
        chain: u8,
        flags: u8,
        status: u8,
        extra: i16,
    },
    /// S→C 0x63 (`client/msg-ui.md` §2): the waypoint object's GUID and
    /// the 16-byte record as received.
    WaypointMenu { guid: u32, record: [u8; 16] },
    /// S→C 0x77 (`client/msg-ui.md` §3): the code, and `0x00463DF0`
    /// captured at receive (r4): no local player, or its mode is 0x11
    /// (dead).
    TradeAction { code: u8, dead_or_absent: bool },
    /// S→C 0x26 (`client/msg-ui.md` §4 r1–r2): the record fields, whether
    /// the unit was in S, and a present player's name.
    ChatLine {
        kind: u8,
        lang: u8,
        unit: UnitKey,
        b8: u8,
        b9: u8,
        /// The name as copied (at most 16 bytes, up to its NUL).
        name: Vec<u8>,
        /// The text as copied (at most 256 bytes, up to its NUL).
        text: Vec<u8>,
        present: bool,
        /// The `name` of a present player unit (type 0).
        player_name: Option<[u8; 16]>,
    },
    /// S→C 0x27 (`client/msg-ui.md` §5 r1): the 40 bytes, whether the unit
    /// of type 1 or 2 was in S, and a type-2 object's class (0 when
    /// absent).
    NpcText {
        bytes: [u8; 40],
        present: bool,
        object_class: u32,
    },
    /// S→C 0x4E (`client/msg-ui.md` §6).
    HireOffer { name: u16, seed: u32 },
    /// S→C 0x4F (`client/msg-ui.md` §6).
    HireListReset,
    /// S→C 0x50 codes 1–4, 23, 36 (`client/msg-ui.md` §7 r3).
    QuestSpecial { code: u16, words: [u16; 6] },
    /// S→C 0x58 (`client/msg-ui.md` §8 r3).
    OpenUi { guid: u32, code: u8, arg: u8 },
    /// S→C 0x78 (`client/msg-ui.md` §11).
    TradePartner { name: [u8; 16], guid: u32 },
    /// S→C 0x8A (`client/msg-ui.md` §9 r2).
    NpcInteract {
        unit: UnitKey,
        present: bool,
        class: u32,
        /// Monster data +0x3C (`0x004AE130`); `None`: no monster U.
        /// PROVISIONAL (client/msg-ui.md §9 r2): no model rule writes
        /// monster data +0x3C, so it keeps −1 (the act5pow sound 4607
        /// path); settled by a Ghidra xref of +0x3C writes in monster
        /// data.
        mdata_3c: Option<i32>,
        /// S holds an object of class 318 in mode 2.
        blocker_open: bool,
    },
    /// S→C 0x91 (`client/msg-ui.md` §10).
    NpcIntro { slots: [u16; 12] },
    /// S→C 0x29 (`client/msg-ui.md` §12).
    GameQuestFlags { record: [u8; 96] },
    /// S→C 0x52 (`client/msg-ui.md` §13).
    QuestLog { status: [u8; 41] },
    /// S→C 0x5E (`client/msg-ui.md` §14).
    QuestAvailability { bytes: [u8; 37] },
    /// S→C 0x9B (`client/msg-ui.md` §15).
    MercRevive { state: u16, value: u16 },
    /// S→C 0x99, 0x9A (`client/msg-skills.md` §7 r3).
    SkillEvent {
        unit: UnitKey,
        skill: u16,
        level: u8,
        target: SkillTarget,
        w: u16,
    },
    /// S→C 0xA3 (`client/msg-skills.md` §8 r3).
    SkillDo {
        unit: UnitKey,
        target: Option<UnitKey>,
        skill: u16,
        level: i16,
        x: u32,
        y: u32,
        v: u8,
    },
    /// The shrine functions of S→C 0x0E code 3 and 0x4D code 0x15
    /// (`client/model.md` §15 rules 3–4).
    ShrineFx {
        kind: ShrineFxKind,
        code: u8,
        object: UnitKey,
        player: Option<UnitKey>,
        /// Entry +0x08, +0x0C (−1 = none).
        overlays: [i32; 2],
    },
    /// The shrine sound of S→C 0x4D code 0x15 (`client/model.md` §15
    /// rule 4 step 4).
    ShrineSound { sound: u32, player: UnitKey },
    /// S→C 0x11 (`client/msg-units.md` §7 r2).
    UnitOverlay {
        unit: UnitKey,
        overlay: u16,
        mode: u8,
        /// 0 (none), 396 or 397.
        sound: u32,
    },
    /// S→C 0x57 (`client/msg-units.md` §7 r3): the nine umod bytes and
    /// flags bit 3.
    UmodFx {
        unit: UnitKey,
        umods: [u8; 9],
        flag8: bool,
    },
    /// S→C 0x73 (`client/msg-units.md` §7 r6): the local player's key and
    /// the message fields.
    ClientMissile {
        owner: Option<UnitKey>,
        class: u16,
        f07: u32,
        f0b: u32,
        f0f: u32,
        f13: u32,
        f17: u16,
        source: UnitKey,
        f1e: u8,
        f1f: u8,
    },
    /// S→C 0x7E (`client/msg-units.md` §7 r8): the act index.
    CommonCof { act: u32 },
    /// S→C 0xA4 (`client/msg-units.md` §7 r10).
    MonsterPreload { class: u16 },
    /// S→C 0x5B, 0x5C, 0x65 (`client/msg-units.md` §8): the active roster
    /// records after the message.
    RosterChanged { roster: Vec<RosterRecord> },
    /// S→C 0xA5 (`client/msg-skills.md` §10 r3).
    SkillEndFx {
        unit: UnitKey,
        skill: u16,
        srvdofunc: i16,
    },
    /// S→C 0x28 type 6 (`client/msg-ui.md` §16 r2).
    QuestFlags { record: [u8; 96] },
    /// S→C 0x28, unit absent (`client/msg-ui.md` §16 r3).
    NpcGone { guid: u32 },
    /// S→C 0x28, unit present (`client/msg-ui.md` §16 r4).
    NpcDialog(Box<NpcDialog>),
    /// S→C 0x62 (`client/msg-ui.md` §17).
    NpcDialogEnd { kind: u8 },
    /// S→C 0x2A (`client/msg-ui.md` §18).
    NpcTransaction { bytes: [u8; 15], gold: i32 },
    /// S→C 0x5A (`client/msg-ui.md` §19 r3): the 40 bytes (name cut) and
    /// the local player's name.
    EventText {
        bytes: [u8; 40],
        local_name: Option<[u8; 16]>,
    },
    /// S→C 0x61 (`client/msg-ui.md` §20).
    ActVideo { video: u8 },
    /// S→C 0x76 (`client/msg-ui.md` §21).
    OverheadClear { unit: UnitKey },
    /// S→C 0x7B (`client/msg-ui.md` §22).
    HotkeyAssign {
        slot: u8,
        skill: i32,
        left: bool,
        item: u32,
    },
    /// S→C 0xB4 (`client/model.md` §7 r8.3): the mapped error number n
    /// of `0x0044E380(n)`.
    JoinRefused { error: u8 },
    /// The update pass's town exit (`client/model.md` §17 r6;
    /// delivery `client/bridge.md` §10 r11): the local player's key and
    /// the GUIDs of every monster (type 1) in S, in key order.
    TownExit { player: UnitKey, monsters: Vec<u32> },
    /// One effect call of state on / hooks / off (`client/stat-lists.md`
    /// §3 r6, r6.7): the unit, the state, the phase, whether the bit was
    /// set before, the unit's dead test, the hook number (setfunc /
    /// remfunc; 0 = none) and the two hook values the model captured.
    StateFx {
        unit: UnitKey,
        state: u16,
        phase: StatePhase,
        was_set: bool,
        dead: bool,
        hook: u8,
        values: [i32; 2],
    },
    /// A model unit free (S→C 0x0A and every free path of
    /// `client/model.md` §2 r5; §10 r3.1 (a)), in free order: the audio
    /// layer detaches the unit's requests without force
    /// (`audio/triggers-2.md` §19 r5). `client_only`: a unit of set C
    /// (`model.md` §5 rule 5).
    UnitFreed { unit: UnitKey, client_only: bool },
    /// The client object update's audio calls (`world/objects-client.md`
    /// §28 r3; mode sound calls, requests, the player event sound), in
    /// update order.
    ObjectSound(ObjSound),
    /// The client object update's effect calls (`world/objects-client.md`
    /// §28 r3; graphics refresh and loads, overlays, lights, the client
    /// skill start), in update order.
    ObjectFx(ObjFx),
}

/// The phase of a `StateFx` (`client/stat-lists.md` §3 r6.1–r6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatePhase {
    /// State on `0x004D9B20` (r6.1).
    On,
    /// State on hooks `0x004D9E60` (r6.2).
    Hooks,
    /// State off `0x004D9F40` / `0x004D9C30` (r6.3).
    Off,
}

/// The target of a skill event (`client/msg-skills.md` §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillTarget {
    Unit(UnitKey),
    Point(u16, u16),
}

/// Which shrine function a `ShrineFx` runs (`client/model.md` §15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShrineFxKind {
    /// Entry +0x00, run by a code-3 mode change.
    OnMode,
    /// Entry +0x04, run by a code-0x15 request.
    OnUse,
}

/// The NPC classes `0x004B1A10(class)` holds (`ui/panels-2.md` §14 r8).
pub const CLASSES_4B1A10: [u32; 13] = [
    146, 251, 266, 331, 377, 378, 406, 408, 521, 527, 537, 538, 539,
];

/// `0x004B1A10(class)` (`ui/panels-2.md` §14 r8): 1 when `class` is one
/// of [`CLASSES_4B1A10`], else 0.
pub fn f4b1a10(class: u32) -> u32 {
    u32::from(CLASSES_4B1A10.contains(&class))
}

/// The payload of `NpcDialog` (`client/msg-ui.md` §16 r4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcDialog {
    pub kind: u8,
    pub guid: u32,
    pub quest_flags: [u8; 96],
    pub unit: UnitKey,
    pub class: u32,
    /// The monstats `interact` flag of the class (bit 9).
    pub interact: bool,
    /// `0x004B1A10(class)`: 1 for a class of [`CLASSES_4B1A10`]
    /// (`ui/panels-2.md` §14 r8), else 0; `None`: not captured.
    pub f4b1a10: Option<u32>,
    /// The local player has a cursor item.
    pub cursor_item: bool,
    /// The local player's level (base stat 12) at receive (`client/bridge.md`
    /// §10 r3, r9): the menu uses it even when a later message of the same
    /// chunk changes the stat.
    pub level: i32,
    /// The number of unidentified items of the local player at receive.
    pub unidentified: u32,
    /// The game is an expansion game (`ClientWorld::expansion` ≠ 0) at receive.
    pub expansion: bool,
    /// The S monsters whose monstats `npc` flag (bit 8) is set, in key
    /// order.
    pub npc_monsters: Vec<UnitKey>,
}

/// Who applies an output (§10 rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    Ui,
    Audio,
    /// The client effect layer (client missiles, overlays, client skill
    /// code; Phase 6).
    Effects,
}

/// Who emits a variant (§10 table `Producer` column): an S→C message
/// handler, or the client update pass (§10 r11, written `update`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    /// The first S→C id of the cell.
    Message(u8),
    /// The update pass.
    Update,
}

/// One row of the §10 table: variant name, producer, consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub variant: &'static str,
    pub producer: Producer,
    pub consumer: Consumer,
}

const fn row(variant: &'static str, producer: u8, consumer: Consumer) -> Row {
    Row {
        variant,
        producer: Producer::Message(producer),
        consumer,
    }
}

const fn update_row(variant: &'static str, consumer: Consumer) -> Row {
    Row {
        variant,
        producer: Producer::Update,
        consumer,
    }
}

use Consumer::{Audio, Effects, Ui};

/// The variants in code, in the §10 table's order (checked against the
/// table, §10 rule 8).
pub const ROWS: [Row; 43] = [
    row("ServerSound", 0x2C, Audio),
    row("QuestUi", 0x5D, Ui),
    row("WaypointMenu", 0x63, Ui),
    row("TradeAction", 0x77, Ui),
    row("ChatLine", 0x26, Ui),
    row("NpcText", 0x27, Ui),
    row("HireOffer", 0x4E, Ui),
    row("HireListReset", 0x4F, Ui),
    row("QuestSpecial", 0x50, Ui),
    row("OpenUi", 0x58, Ui),
    row("TradePartner", 0x78, Ui),
    row("NpcInteract", 0x8A, Ui),
    row("NpcIntro", 0x91, Ui),
    row("GameQuestFlags", 0x29, Ui),
    row("QuestLog", 0x52, Ui),
    row("QuestAvailability", 0x5E, Ui),
    row("MercRevive", 0x9B, Ui),
    row("SkillEvent", 0x99, Effects),
    row("SkillDo", 0xA3, Effects),
    row("ShrineFx", 0x0E, Effects),
    row("ShrineSound", 0x4D, Audio),
    row("UnitOverlay", 0x11, Effects),
    row("UmodFx", 0x57, Effects),
    row("ClientMissile", 0x73, Effects),
    row("CommonCof", 0x7E, Effects),
    row("MonsterPreload", 0xA4, Effects),
    row("RosterChanged", 0x5B, Ui),
    row("SkillEndFx", 0xA5, Effects),
    row("QuestFlags", 0x28, Ui),
    row("NpcGone", 0x28, Ui),
    row("NpcDialog", 0x28, Ui),
    row("NpcDialogEnd", 0x62, Ui),
    row("NpcTransaction", 0x2A, Ui),
    row("EventText", 0x5A, Ui),
    row("ActVideo", 0x61, Ui),
    row("OverheadClear", 0x76, Ui),
    row("HotkeyAssign", 0x7B, Ui),
    row("JoinRefused", 0xB4, Ui),
    update_row("TownExit", Ui),
    row("StateFx", 0xA8, Effects),
    row("UnitFreed", 0x0A, Audio),
    update_row("ObjectSound", Audio),
    update_row("ObjectFx", Effects),
];

impl Output {
    /// The variant's row in [`ROWS`].
    pub fn row(&self) -> &'static Row {
        let i = match self {
            Output::ServerSound { .. } => 0,
            Output::QuestUi { .. } => 1,
            Output::WaypointMenu { .. } => 2,
            Output::TradeAction { .. } => 3,
            Output::ChatLine { .. } => 4,
            Output::NpcText { .. } => 5,
            Output::HireOffer { .. } => 6,
            Output::HireListReset => 7,
            Output::QuestSpecial { .. } => 8,
            Output::OpenUi { .. } => 9,
            Output::TradePartner { .. } => 10,
            Output::NpcInteract { .. } => 11,
            Output::NpcIntro { .. } => 12,
            Output::GameQuestFlags { .. } => 13,
            Output::QuestLog { .. } => 14,
            Output::QuestAvailability { .. } => 15,
            Output::MercRevive { .. } => 16,
            Output::SkillEvent { .. } => 17,
            Output::SkillDo { .. } => 18,
            Output::ShrineFx { .. } => 19,
            Output::ShrineSound { .. } => 20,
            Output::UnitOverlay { .. } => 21,
            Output::UmodFx { .. } => 22,
            Output::ClientMissile { .. } => 23,
            Output::CommonCof { .. } => 24,
            Output::MonsterPreload { .. } => 25,
            Output::RosterChanged { .. } => 26,
            Output::SkillEndFx { .. } => 27,
            Output::QuestFlags { .. } => 28,
            Output::NpcGone { .. } => 29,
            Output::NpcDialog(_) => 30,
            Output::NpcDialogEnd { .. } => 31,
            Output::NpcTransaction { .. } => 32,
            Output::EventText { .. } => 33,
            Output::ActVideo { .. } => 34,
            Output::OverheadClear { .. } => 35,
            Output::HotkeyAssign { .. } => 36,
            Output::JoinRefused { .. } => 37,
            Output::TownExit { .. } => 38,
            Output::StateFx { .. } => 39,
            Output::UnitFreed { .. } => 40,
            Output::ObjectSound(_) => 41,
            Output::ObjectFx(_) => 42,
        };
        &ROWS[i]
    }

    pub fn consumer(&self) -> Consumer {
        self.row().consumer
    }
}

/// Moves the model's unit frees made since the last call to `outputs`
/// as `UnitFreed` outputs, in free order (§10 r3.1 (a)). The receive
/// path and the update pass call it after each handler (every free path
/// frees after the handler's own outputs), the bridge once more before
/// it hands the list over.
pub fn move_freed(world: &mut super::world::ClientWorld, outputs: &mut Vec<Output>) {
    outputs.extend(
        world
            .freed
            .drain(..)
            .map(|(unit, client_only)| Output::UnitFreed { unit, client_only }),
    );
}

/// The sink of one handler call: outputs in the handler's call order
/// (§10 rule 2). Shared through the message, so handlers keep their
/// signature.
#[derive(Debug, Default)]
pub struct Outputs(RefCell<Vec<Output>>);

impl Outputs {
    pub fn push(&self, o: Output) {
        self.0.borrow_mut().push(o);
    }

    /// The outputs pushed so far, in order; the sink is left empty.
    pub fn take(&self) -> Vec<Output> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

/// Applies one frame's outputs in list order, each to its consumer (§10
/// rules 4–5). Never reorders, merges or drops.
pub fn dispatch<E>(
    outputs: &[Output],
    ui: &mut dyn FnMut(&Output) -> Result<(), E>,
    audio: &mut dyn FnMut(&Output) -> Result<(), E>,
    effects: &mut dyn FnMut(&Output) -> Result<(), E>,
) -> Result<(), E> {
    for o in outputs {
        match o.consumer() {
            Consumer::Ui => ui(o)?,
            Consumer::Audio => audio(o)?,
            Consumer::Effects => effects(o)?,
        }
    }
    Ok(())
}

/// A malformed §10 table.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error("bridge.md has no §10 rows table")]
    NoTable,
    #[error(
        "§10 row {0:?}: expected | `Variant` | payload | 0xNN … or update | ui, audio or effects | owner |"
    )]
    Row(String),
}

/// One row of the §10 table as written in the spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub variant: String,
    pub producer: Producer,
    pub consumer: Consumer,
}

/// The §10 table of `bridge.md` (the rows after `<!-- rows -->` in
/// §10); the producer is the first `0xNN` of its cell, or the cell
/// `update` (§10 r11).
pub fn parse_table(spec: &str) -> Result<Vec<TableRow>, TableError> {
    let section = spec.split("### 10.").nth(1).ok_or(TableError::NoTable)?;
    let table = section
        .split("<!-- rows -->")
        .nth(1)
        .ok_or(TableError::NoTable)?;
    let mut rows = Vec::new();
    for line in table.lines().skip_while(|l| l.trim().is_empty()) {
        if !line.starts_with('|') {
            break;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 7 || cells[1] == "Variant" || cells[1].starts_with("---") {
            continue;
        }
        let bad = || TableError::Row(line.to_owned());
        let variant = cells[1]
            .strip_prefix('`')
            .and_then(|v| v.strip_suffix('`'))
            .ok_or_else(bad)?;
        let producer = if cells[3] == "update" {
            Producer::Update
        } else {
            let hex = cells[3].get(2..4).filter(|_| cells[3].starts_with("0x"));
            hex.and_then(|h| u8::from_str_radix(h, 16).ok())
                .map(Producer::Message)
                .ok_or_else(bad)?
        };
        let consumer = match cells[4] {
            "UI" | "ui" => Consumer::Ui,
            "audio" => Consumer::Audio,
            "effects" => Consumer::Effects,
            _ => return Err(bad()),
        };
        rows.push(TableRow {
            variant: variant.to_owned(),
            producer,
            consumer,
        });
    }
    if rows.is_empty() {
        return Err(TableError::NoTable);
    }
    Ok(rows)
}

/// The §10 rule 8 check: every variant in code has exactly one row with
/// the same producer and consumer, and every row has a variant. Returns
/// the variant names that disagree.
pub fn check(table: &[TableRow], code: &[Row]) -> Vec<String> {
    let mut out = Vec::new();
    for c in code {
        let found: Vec<&TableRow> = table.iter().filter(|r| r.variant == c.variant).collect();
        if found.len() != 1 || (found[0].producer, found[0].consumer) != (c.producer, c.consumer) {
            out.push(c.variant.to_owned());
        }
    }
    for r in table {
        if !code.iter().any(|c| c.variant == r.variant) {
            out.push(r.variant.to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::objects::{ObjFx, ObjSound};

    const SPEC: &str = include_str!("../../../../specs/client/bridge.md");

    // Covers: specs/client/bridge.md §10 r8
    #[test]
    fn variants_match_the_spec_table() {
        let table = parse_table(SPEC).unwrap();
        assert_eq!(check(&table, &ROWS), Vec::<String>::new());
        let names: Vec<&str> = table.iter().map(|r| r.variant.as_str()).collect();
        let code: Vec<&str> = ROWS.iter().map(|r| r.variant).collect();
        assert_eq!(names, code, "same order as the table");
    }

    // Covers: specs/client/bridge.md §10 r8
    #[test]
    fn a_changed_row_is_reported() {
        let perturbed = SPEC.replace(
            "captured) | 0x77 | UI | `client/msg-ui.md` §3 |",
            "captured) | 0x78 | UI | `client/msg-ui.md` §3 |",
        );
        assert_ne!(perturbed, SPEC);
        let table = parse_table(&perturbed).unwrap();
        assert_eq!(check(&table, &ROWS), ["TradeAction"]);
        let renamed = SPEC.replace("| `QuestUi` |", "| `QuestScreen` |");
        let table = parse_table(&renamed).unwrap();
        assert_eq!(check(&table, &ROWS), ["QuestUi", "QuestScreen"]);
    }

    // Covers: specs/client/bridge.md §10 r11
    #[test]
    fn an_update_producer_is_its_own_kind() {
        let table = parse_table(SPEC).unwrap();
        let town = table.iter().find(|r| r.variant == "TownExit").unwrap();
        assert_eq!(town.producer, Producer::Update);
        let refused = table.iter().find(|r| r.variant == "JoinRefused").unwrap();
        assert_eq!(refused.producer, Producer::Message(0xB4));
        let bad = SPEC.replace(
            "| `TownExit` | local player key, GUIDs of the S monsters | update |",
            "| `TownExit` | local player key, GUIDs of the S monsters | updates |",
        );
        assert_ne!(bad, SPEC);
        assert!(matches!(parse_table(&bad), Err(TableError::Row(_))));
        let moved = SPEC.replace(
            "| `TownExit` | local player key, GUIDs of the S monsters | update |",
            "| `TownExit` | local player key, GUIDs of the S monsters | 0x15 |",
        );
        assert_eq!(check(&parse_table(&moved).unwrap(), &ROWS), ["TownExit"]);
    }

    // Covers: specs/client/bridge.md §10 r4, §10 r5
    #[test]
    fn dispatch_keeps_list_order_and_routes_by_consumer() {
        let list = [
            Output::ServerSound {
                unit: UnitKey::new(1, 0x26),
                class: 3,
                at: None,
                event: 18,
            },
            Output::TradeAction {
                code: 0x10,
                dead_or_absent: false,
            },
            Output::ServerSound {
                unit: UnitKey::new(0, 1),
                class: 0,
                at: None,
                event: 2,
            },
            Output::MonsterPreload { class: 7 },
        ];
        let seen = RefCell::new(Vec::new());
        dispatch::<()>(
            &list,
            &mut |o| {
                seen.borrow_mut().push(("ui", o.clone()));
                Ok(())
            },
            &mut |o| {
                seen.borrow_mut().push(("audio", o.clone()));
                Ok(())
            },
            &mut |o| {
                seen.borrow_mut().push(("effects", o.clone()));
                Ok(())
            },
        )
        .unwrap();
        let seen = seen.into_inner();
        assert_eq!(
            seen.iter().map(|s| s.0).collect::<Vec<_>>(),
            ["audio", "ui", "audio", "effects"]
        );
        assert_eq!(seen.into_iter().map(|s| s.1).collect::<Vec<_>>(), list);
    }

    fn every_variant() -> Vec<Output> {
        let k = UnitKey::new(1, 9);
        let obj = crate::bridge::objects::ObjUnit {
            key: k,
            client_only: false,
        };
        vec![
            Output::ServerSound {
                unit: k,
                class: 1,
                at: None,
                event: 2,
            },
            Output::QuestUi {
                chain: 0,
                flags: 0,
                status: 0,
                extra: 0,
            },
            Output::WaypointMenu {
                guid: 1,
                record: [0; 16],
            },
            Output::TradeAction {
                code: 0,
                dead_or_absent: false,
            },
            Output::ChatLine {
                kind: 0,
                lang: 0,
                unit: k,
                b8: 0,
                b9: 0,
                name: vec![],
                text: vec![],
                present: false,
                player_name: None,
            },
            Output::NpcText {
                bytes: [0; 40],
                present: false,
                object_class: 0,
            },
            Output::HireOffer { name: 0, seed: 0 },
            Output::HireListReset,
            Output::QuestSpecial {
                code: 0,
                words: [0; 6],
            },
            Output::OpenUi {
                guid: 0,
                code: 0,
                arg: 0,
            },
            Output::TradePartner {
                name: [0; 16],
                guid: 0,
            },
            Output::NpcInteract {
                unit: k,
                present: false,
                class: 0,
                mdata_3c: None,
                blocker_open: false,
            },
            Output::NpcIntro { slots: [0; 12] },
            Output::GameQuestFlags { record: [0; 96] },
            Output::QuestLog { status: [0; 41] },
            Output::QuestAvailability { bytes: [0; 37] },
            Output::MercRevive { state: 0, value: 0 },
            Output::SkillEvent {
                unit: k,
                skill: 0,
                level: 0,
                target: SkillTarget::Point(0, 0),
                w: 0,
            },
            Output::SkillDo {
                unit: k,
                target: None,
                skill: 0,
                level: 0,
                x: 0,
                y: 0,
                v: 0,
            },
            Output::ShrineFx {
                kind: ShrineFxKind::OnMode,
                code: 0,
                object: k,
                player: None,
                overlays: [-1, -1],
            },
            Output::ShrineSound {
                sound: 0,
                player: k,
            },
            Output::UnitOverlay {
                unit: k,
                overlay: 0,
                mode: 0,
                sound: 0,
            },
            Output::UmodFx {
                unit: k,
                umods: [0; 9],
                flag8: false,
            },
            Output::ClientMissile {
                owner: None,
                class: 0,
                f07: 0,
                f0b: 0,
                f0f: 0,
                f13: 0,
                f17: 0,
                source: k,
                f1e: 0,
                f1f: 0,
            },
            Output::CommonCof { act: 0 },
            Output::MonsterPreload { class: 0 },
            Output::RosterChanged { roster: vec![] },
            Output::SkillEndFx {
                unit: k,
                skill: 0,
                srvdofunc: 0,
            },
            Output::QuestFlags { record: [0; 96] },
            Output::NpcGone { guid: 0 },
            Output::NpcDialog(Box::new(NpcDialog {
                kind: 0,
                guid: 0,
                quest_flags: [0; 96],
                unit: k,
                class: 0,
                interact: false,
                f4b1a10: None,
                cursor_item: false,
                level: 1,
                unidentified: 0,
                expansion: false,
                npc_monsters: vec![],
            })),
            Output::NpcDialogEnd { kind: 0 },
            Output::NpcTransaction {
                bytes: [0; 15],
                gold: 0,
            },
            Output::EventText {
                bytes: [0; 40],
                local_name: None,
            },
            Output::ActVideo { video: 0 },
            Output::OverheadClear { unit: k },
            Output::HotkeyAssign {
                slot: 0,
                skill: 0,
                left: false,
                item: 0,
            },
            Output::JoinRefused { error: 0 },
            Output::TownExit {
                player: k,
                monsters: vec![],
            },
            Output::StateFx {
                unit: k,
                state: 0,
                phase: StatePhase::On,
                was_set: false,
                dead: false,
                hook: 0,
                values: [0; 2],
            },
            Output::UnitFreed {
                unit: k,
                client_only: false,
            },
            Output::ObjectSound(ObjSound::Request { id: 0, unit: obj }),
            Output::ObjectFx(ObjFx::GfxLoad { class: 0, flag: 0 }),
        ]
    }

    // Covers: specs/client/bridge.md §10 r1, §10 row1, §10 row2, §10 row3, §10 row4, §10 row5, §10 row6, §10 row7, §10 row8, §10 row9, §10 row10, §10 row11, §10 row12, §10 row13, §10 row14, §10 row15, §10 row16, §10 row17, §10 row18, §10 row19, §10 row20, §10 row21, §10 row22, §10 row23, §10 row24, §10 row25, §10 row26, §10 row27, §10 row28, §10 row29, §10 row30, §10 row31, §10 row32, §10 row33, §10 row34, §10 row35, §10 row36, §10 row37, §10 row38, §10 row39, §10 row40, §10 row41, §10 row42, §10 row43
    #[test]
    fn each_table_row_is_one_variant_with_its_producer_and_consumer() {
        let table = parse_table(SPEC).unwrap();
        let all = every_variant();
        assert_eq!(all.len(), 43);
        assert_eq!(table.len(), 43);
        for (i, (o, t)) in all.iter().zip(&table).enumerate() {
            // The variant's Debug name is the table's variant name.
            let dbg = format!("{o:?}");
            let name: String = dbg.chars().take_while(|c| c.is_alphanumeric()).collect();
            assert_eq!(name, t.variant, "row {}", i + 1);
            let r = o.row();
            assert_eq!(r.variant, t.variant, "row {}", i + 1);
            assert_eq!(
                (r.producer, o.consumer()),
                (t.producer, t.consumer),
                "row {}",
                i + 1
            );
        }
        // §10 r1: handlers append to the sink; the list keeps their order.
        let sink = Outputs::default();
        for o in &all {
            sink.push(o.clone());
        }
        assert_eq!(sink.take(), all);
        assert!(sink.take().is_empty());
    }
}
