// Spec: specs/client/bridge.md (§10)
//! Client outputs (§10): the 1.14d UI and sound calls an S→C handler
//! makes, carried out of the bridge as [`Output`]s. A handler appends
//! them to its message's sink ([`Outputs`]); the receive path moves them
//! to the bridge's list in message order, and the bridge hands the list
//! over whole at the end of the frame (§10 rule 4). [`dispatch`] applies
//! a list in order, each output to its one consumer (rule 5). Consumers
//! never write the model (rule 6).

use std::cell::RefCell;

use super::world::UnitKey;

/// One output: a 1.14d UI or sound entry point with the values it read
/// when the handler ran (§10 rules 1, 3). Each variant is owned by the
/// spec of its producer ([`ROWS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// S→C 0x2C (`audio/triggers.md` §2 r4): the event unit's key and
    /// class at receive, and the event.
    ServerSound {
        unit: UnitKey,
        class: u32,
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
    /// S→C 0x77 (`client/msg-ui.md` §3): the code.
    TradeAction { code: u8 },
}

/// Who applies an output (§10 rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    Ui,
    Audio,
}

/// One row of the §10 table: variant name, producer id, consumer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub variant: &'static str,
    pub producer: u8,
    pub consumer: Consumer,
}

/// The variants in code, in the §10 table's order (checked against the
/// table, §10 rule 8).
pub const ROWS: [Row; 4] = [
    Row {
        variant: "ServerSound",
        producer: 0x2C,
        consumer: Consumer::Audio,
    },
    Row {
        variant: "QuestUi",
        producer: 0x5D,
        consumer: Consumer::Ui,
    },
    Row {
        variant: "WaypointMenu",
        producer: 0x63,
        consumer: Consumer::Ui,
    },
    Row {
        variant: "TradeAction",
        producer: 0x77,
        consumer: Consumer::Ui,
    },
];

impl Output {
    /// The variant's row in [`ROWS`].
    pub fn row(&self) -> &'static Row {
        let i = match self {
            Output::ServerSound { .. } => 0,
            Output::QuestUi { .. } => 1,
            Output::WaypointMenu { .. } => 2,
            Output::TradeAction { .. } => 3,
        };
        &ROWS[i]
    }

    pub fn consumer(&self) -> Consumer {
        self.row().consumer
    }
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
) -> Result<(), E> {
    for o in outputs {
        match o.consumer() {
            Consumer::Ui => ui(o)?,
            Consumer::Audio => audio(o)?,
        }
    }
    Ok(())
}

/// A malformed §10 table.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableError {
    #[error("bridge.md has no §10 rows table")]
    NoTable,
    #[error("§10 row {0:?}: expected | `Variant` | payload | 0xNN … | ui or audio | owner |")]
    Row(String),
}

/// One row of the §10 table as written in the spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub variant: String,
    pub producer: u8,
    pub consumer: Consumer,
}

/// The §10 table of `bridge.md` (the rows after `<!-- rows -->` in
/// §10); the producer is the first `0xNN` of its cell.
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
        let hex = cells[3].get(2..4).filter(|_| cells[3].starts_with("0x"));
        let producer = hex
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .ok_or_else(bad)?;
        let consumer = match cells[4] {
            "UI" | "ui" => Consumer::Ui,
            "audio" => Consumer::Audio,
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
            "| `TradeAction` | code u8 | 0x77 |",
            "| `TradeAction` | code u8 | 0x78 |",
        );
        assert_ne!(perturbed, SPEC);
        let table = parse_table(&perturbed).unwrap();
        assert_eq!(check(&table, &ROWS), ["TradeAction"]);
        let renamed = SPEC.replace("| `QuestUi` |", "| `QuestScreen` |");
        let table = parse_table(&renamed).unwrap();
        assert_eq!(check(&table, &ROWS), ["QuestUi", "QuestScreen"]);
    }

    // Covers: specs/client/bridge.md §10 r4, §10 r5
    #[test]
    fn dispatch_keeps_list_order_and_routes_by_consumer() {
        let list = [
            Output::ServerSound {
                unit: UnitKey::new(1, 0x26),
                class: 3,
                event: 18,
            },
            Output::TradeAction { code: 0x10 },
            Output::ServerSound {
                unit: UnitKey::new(0, 1),
                class: 0,
                event: 2,
            },
        ];
        let seen = RefCell::new(Vec::new());
        dispatch::<()>(
            &list,
            &mut |o| {
                seen.borrow_mut().push(("ui", *o));
                Ok(())
            },
            &mut |o| {
                seen.borrow_mut().push(("audio", *o));
                Ok(())
            },
        )
        .unwrap();
        let seen = seen.into_inner();
        assert_eq!(
            seen.iter().map(|s| s.0).collect::<Vec<_>>(),
            ["audio", "ui", "audio"]
        );
        assert_eq!(seen.iter().map(|s| s.1).collect::<Vec<_>>(), list);
    }
}
