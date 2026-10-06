// Spec: specs/sim/intents-events.md (§5: client-messages.tsv, server-messages.tsv)
//! Generates [`crate::generated`] from the two machine tables: the
//! descriptor arrays `CLIENT_MESSAGES` / `SERVER_MESSAGES` (one entry per
//! id) and one typed struct per message whose size is fixed and whose
//! layout has only fixed fields (or that is the id byte alone).
//! `data-tool gen-proto` writes the output to `src/generated.rs`; unit
//! tests check the committed file is current and that its tables equal
//! the TSVs ([`crate::tsv::check`]).
//!
//! Struct field names: the layout name; `type` → `type_`; an unnamed
//! field → `f<offset>`; a name used twice in one layout → `<name>_<offset>`
//! for each use (offsets in decimal).

use std::fmt::Write;

use crate::schema::{Field, FieldType, HandlerSize, SizeRule, Width};
use crate::tsv::{parse_client, parse_server, ClientRow, ServerRow, TsvError};

/// Path of the generated file, relative to the `d2-proto` crate.
pub const GENERATED_PATH: &str = "src/generated.rs";

fn opt_addr(a: Option<u32>) -> String {
    match a {
        Some(a) => format!("Some(0x{a:08X})"),
        None => "None".into(),
    }
}

fn size_rule(r: &SizeRule) -> String {
    match *r {
        SizeRule::Fixed(n) => format!("SizeRule::Fixed({n})"),
        SizeRule::Field {
            width,
            offset,
            mul,
            add,
            cap,
            min,
        } => format!(
            "SizeRule::Field {{ width: Width::{}, offset: {offset}, mul: {mul}, add: {add}, \
             cap: {}, min: {min} }}",
            match width {
                Width::U8 => "U8",
                Width::U16 => "U16",
            },
            match cap {
                Some(c) => format!("Some(0x{c:X})"),
                None => "None".into(),
            }
        ),
        SizeRule::Chat => "SizeRule::Chat".into(),
        SizeRule::Chat26 => "SizeRule::Chat26".into(),
        SizeRule::Af => "SizeRule::Af".into(),
    }
}

fn handler_size(h: &HandlerSize) -> String {
    match *h {
        HandlerSize::Exact(n) => format!("HandlerSize::Exact({n})"),
        HandlerSize::Range(a, b) => format!("HandlerSize::Range({a}, {b})"),
        HandlerSize::Chat => "HandlerSize::Chat".into(),
        HandlerSize::Any => "HandlerSize::Any".into(),
        HandlerSize::None => "HandlerSize::None".into(),
    }
}

fn field_type(t: FieldType) -> String {
    match t {
        FieldType::U8 => "FieldType::U8".into(),
        FieldType::U16 => "FieldType::U16".into(),
        FieldType::U32 => "FieldType::U32".into(),
        FieldType::Cstr => "FieldType::Cstr".into(),
        FieldType::Cstr16 => "FieldType::Cstr16".into(),
        FieldType::Bits(n) => format!("FieldType::Bits({n})"),
        FieldType::Bit(n) => format!("FieldType::Bit({n})"),
        FieldType::Tail => "FieldType::Tail".into(),
    }
}

fn layout(fields: &[Field]) -> String {
    if fields.is_empty() {
        return "&[]".into();
    }
    let items: Vec<String> = fields
        .iter()
        .map(|f| {
            format!(
                "Field {{ name: {:?}, ty: {}, offset: {} }}",
                f.name,
                field_type(f.ty),
                match f.offset {
                    Some(o) => format!("Some({o})"),
                    None => "None".into(),
                }
            )
        })
        .collect();
    format!("&[{}]", items.join(", "))
}

/// Rust type of a fixed field in a typed struct.
fn rust_type(t: FieldType) -> &'static str {
    match t {
        FieldType::U8 => "u8",
        FieldType::U16 => "u16",
        FieldType::U32 => "u32",
        FieldType::Cstr16 => "[u8; 16]",
        FieldType::Bits(n) if n <= 8 => "u8",
        FieldType::Bits(n) if n <= 16 => "u16",
        FieldType::Bits(_) => "u32",
        FieldType::Bit(_) => "bool",
        FieldType::Cstr | FieldType::Tail => unreachable!("not a fixed field"),
    }
}

/// Struct field identifiers for `fields` (module doc).
fn idents(fields: &[Field]) -> Vec<String> {
    fields
        .iter()
        .map(|f| {
            let off = f.offset.expect("fixed field");
            if f.name.is_empty() {
                format!("f{off}")
            } else if fields.iter().filter(|g| g.name == f.name).count() > 1 {
                format!("{}_{off}", f.name)
            } else if f.name == "type" {
                "type_".into()
            } else {
                f.name.to_string()
            }
        })
        .collect()
}

/// Whether a row gets a typed struct; its fixed size if so.
fn typed(name: &str, size: &SizeRule, fields: &[Field]) -> Option<usize> {
    let n = size.fixed()?;
    let fixed = fields
        .iter()
        .all(|f| f.offset.is_some() && !matches!(f.ty, FieldType::Cstr | FieldType::Tail));
    (name != "-" && fixed && (!fields.is_empty() || n == 1)).then_some(n)
}

fn struct_code(out: &mut String, id: u8, name: &str, doc: &str, size: usize, fields: &[Field]) {
    let ids = idents(fields);
    let unit = if size == 1 { "byte" } else { "bytes" };
    let _ = writeln!(out, "\n    /// 0x{id:02X} {name}{doc} ({size} {unit}).");
    out.push_str("    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n");
    if fields.is_empty() {
        let _ = writeln!(out, "    pub struct {name};");
    } else {
        let _ = writeln!(out, "    pub struct {name} {{");
        for (f, ident) in fields.iter().zip(&ids) {
            let _ = writeln!(
                out,
                "        /// `{}` at {}.",
                field_type_word(f.ty),
                f.offset.expect("fixed field")
            );
            let _ = writeln!(out, "        pub {ident}: {},", rust_type(f.ty));
        }
        out.push_str("    }\n");
    }
    let _ = writeln!(out, "\n    impl FixedMessage for {name} {{");
    let _ = writeln!(out, "        const ID: u8 = 0x{id:02X};");
    let _ = writeln!(out, "        const SIZE: usize = {size};");
    out.push_str("        fn decode(b: &[u8]) -> Result<Self, DecodeError> {\n");
    out.push_str("            check(b, Self::ID, Self::SIZE)?;\n");
    if fields.is_empty() {
        out.push_str("            Ok(Self)\n");
    } else {
        out.push_str("            Ok(Self {\n");
        for (f, ident) in fields.iter().zip(&ids) {
            let o = f.offset.expect("fixed field");
            let expr = match f.ty {
                FieldType::U8 => format!("u8_at(b, {o})"),
                FieldType::U16 => format!("u16_at(b, {o})"),
                FieldType::U32 => format!("u32_at(b, {o})"),
                FieldType::Cstr16 => format!("bytes16_at(b, {o})"),
                FieldType::Bits(n) if n <= 16 => {
                    format!("bits_at(b, {o}, {n}) as {}", rust_type(f.ty))
                }
                FieldType::Bits(n) => format!("bits_at(b, {o}, {n})"),
                FieldType::Bit(n) => format!("bit_at(b, {o}, {n})"),
                FieldType::Cstr | FieldType::Tail => unreachable!(),
            };
            let _ = writeln!(out, "                {ident}: {expr},");
        }
        out.push_str("            })\n");
    }
    out.push_str("        }\n");
    out.push_str("        fn write(&self, out: &mut [u8]) {\n");
    out.push_str("            start(out, Self::ID, Self::SIZE);\n");
    for (f, ident) in fields.iter().zip(&ids) {
        let o = f.offset.expect("fixed field");
        let stmt = match f.ty {
            FieldType::U8 => format!("put_u8(out, {o}, self.{ident})"),
            FieldType::U16 => format!("put_u16(out, {o}, self.{ident})"),
            FieldType::U32 => format!("put_u32(out, {o}, self.{ident})"),
            FieldType::Cstr16 => format!("put_bytes16(out, {o}, &self.{ident})"),
            FieldType::Bits(n) if n <= 16 => {
                format!("put_bits(out, {o}, {n}, u32::from(self.{ident}))")
            }
            FieldType::Bits(n) => format!("put_bits(out, {o}, {n}, self.{ident})"),
            FieldType::Bit(n) => format!("put_bit(out, {o}, {n}, self.{ident})"),
            FieldType::Cstr | FieldType::Tail => unreachable!(),
        };
        let _ = writeln!(out, "            {stmt};");
    }
    out.push_str("        }\n    }\n");
    let _ = writeln!(out, "\n    impl {name} {{");
    out.push_str("        /// The message bytes; unlisted bytes are 0.\n");
    let _ = writeln!(out, "        pub fn encode(&self) -> [u8; {size}] {{");
    let _ = writeln!(out, "            let mut b = [0; {size}];");
    out.push_str("            self.write(&mut b);\n            b\n        }\n    }\n");
}

fn field_type_word(t: FieldType) -> String {
    match t {
        FieldType::U8 => "u8".into(),
        FieldType::U16 => "u16".into(),
        FieldType::U32 => "u32".into(),
        FieldType::Cstr => "cstr".into(),
        FieldType::Cstr16 => "cstr16".into(),
        FieldType::Bits(n) => format!("u{n}"),
        FieldType::Bit(n) => format!("bit{n}"),
        FieldType::Tail => "bytes".into(),
    }
}

const MODULE_USE: &str = "    use crate::wire::*;\n";

fn client_code(out: &mut String, rows: &[ClientRow]) {
    let _ = writeln!(
        out,
        "\n/// C→S messages, indexed by id (0x00–0x{:02X}): `client-messages.tsv`.\n\
         pub static CLIENT_MESSAGES: [ClientMessage; {}] = [",
        rows.len() - 1,
        rows.len()
    );
    for r in rows {
        let _ = writeln!(
            out,
            "    ClientMessage {{ id: 0x{:02X}, name: {:?}, transport_size: {}, handler_size: {}, \
             layout: {}, handler: {}, kind: Kind::{:?}, gate: Gate::{:?}, request: {:?}, \
             scope: Scope::{:?}, confirmed: Confirmed::{:?} }},",
            r.id,
            r.name,
            size_rule(&r.transport_size),
            handler_size(&r.handler_size),
            layout(&r.layout),
            opt_addr(r.handler),
            r.kind,
            r.gate,
            r.request,
            r.scope,
            r.confirmed
        );
    }
    out.push_str("];\n");
    out.push_str(
        "\n/// Typed C→S messages with a fixed layout (fields a 1.14d handler reads).\n\
         pub mod client {\n",
    );
    out.push_str(MODULE_USE);
    for r in rows {
        if let Some(n) = typed(r.name, &r.transport_size, &r.layout) {
            let doc = format!(": {}", r.request);
            struct_code(out, r.id, r.name, &doc, n, &r.layout);
        }
    }
    out.push_str("}\n");
}

fn server_code(out: &mut String, rows: &[ServerRow]) {
    let _ = writeln!(
        out,
        "\n/// S→C messages, indexed by id (0x00–0x{:02X}): `server-messages.tsv`.\n\
         pub static SERVER_MESSAGES: [ServerMessage; {}] = [",
        rows.len() - 1,
        rows.len()
    );
    for r in rows {
        let senders: Vec<String> = r.senders.iter().map(|a| format!("0x{a:08X}")).collect();
        let _ = writeln!(
            out,
            "    ServerMessage {{ id: 0x{:02X}, name: {:?}, size: {}, layout: {}, senders: &[{}], \
             client_handler: {}, client_unit_handler: {}, produced_by: ProducedBy::{:?}, \
             confirmed: Confirmed::{:?} }},",
            r.id,
            r.name,
            size_rule(&r.size),
            layout(&r.layout),
            senders.join(", "),
            opt_addr(r.client_handler),
            opt_addr(r.client_unit_handler),
            r.produced_by,
            r.confirmed
        );
    }
    out.push_str("];\n");
    out.push_str(
        "\n/// Typed S→C messages with a fixed, confirmed layout.\n\
         pub mod server {\n",
    );
    out.push_str(MODULE_USE);
    for r in rows {
        if let Some(n) = typed(r.name, &r.size, &r.layout) {
            struct_code(out, r.id, r.name, "", n, &r.layout);
        }
    }
    out.push_str("}\n");
}

/// The generated module source for the two TSV texts.
pub fn generate(client_tsv: &str, server_tsv: &str) -> Result<String, TsvError> {
    let client = parse_client(client_tsv)?;
    let server = parse_server(server_tsv)?;
    let mut out = String::from(
        "// Spec: specs/sim/intents-events.md (generated from client-messages.tsv and \
         server-messages.tsv)\n\
         // @generated by `data-tool gen-proto` (d2-proto::codegen). Do not edit.\n\
         //! Message descriptors per id and direction; typed fixed-layout messages.\n\
         \n\
         use crate::schema::{\n\
         \x20   ClientMessage, Confirmed, Field, FieldType, Gate, HandlerSize, Kind, ProducedBy,\n\
         \x20   Scope, ServerMessage, SizeRule, Width,\n\
         };\n",
    );
    client_code(&mut out, &client);
    server_code(&mut out, &server);
    Ok(out)
}

#[cfg(test)]
mod mutant_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tsv::{CLIENT_TSV, SERVER_TSV};

    /// The committed file is what the generator writes today.
    #[test]
    fn generated_file_is_current() {
        let committed = include_str!("generated.rs");
        assert!(
            generate(CLIENT_TSV, SERVER_TSV).unwrap() == committed,
            "src/generated.rs is stale: run `cargo run -p data-tool -- gen-proto`"
        );
    }

    /// M08: a changed TSV row changes the generated file.
    #[test]
    fn staleness_check_can_fail() {
        let changed = CLIENT_TSV.replacen("\tWalk\t5\t==5\t", "\tWalk\t6\t==6\t", 1);
        assert_ne!(changed, CLIENT_TSV);
        let code = generate(&changed, SERVER_TSV).unwrap();
        assert_ne!(code, include_str!("generated.rs"));
        assert!(code.contains("transport_size: SizeRule::Fixed(6)"));
    }

    #[test]
    fn field_idents() {
        let f = |name, offset| Field {
            name,
            ty: FieldType::U8,
            offset: Some(offset),
        };
        assert_eq!(
            idents(&[f("type", 1), f("", 2), f("unk", 3), f("unk", 4)]),
            ["type_", "f2", "unk_3", "unk_4"]
        );
    }
}
