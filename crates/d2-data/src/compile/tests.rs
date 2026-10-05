// Spec: specs/data/field-types.md, specs/data/txt-format.md (test vectors)

use super::*;
use crate::schema::FieldType as T;

fn field(name: &str, t: T, len: u32, offset: u32, link: &str) -> FieldDef {
    let link = match link {
        "" => Link::None,
        l if l.starts_with("cb:") => Link::Table(l[3..].to_owned()),
        l => Link::Linker(l.to_owned()),
    };
    FieldDef::new(name, t, len, offset, link)
}

/// A recorded field-callback call: text, offset, len, record, slot.
type Call = (Option<Vec<u8>>, u32, u32, u32, u32);

/// Callbacks that record their calls; the key callback returns 0x100 +
/// the text length.
#[derive(Default)]
struct Mock {
    keys: Vec<Vec<u8>>,
    calls: Vec<Call>,
}

impl Callbacks for Mock {
    fn key(&mut self, _f: &FieldDef, text: &[u8]) -> u16 {
        self.keys.push(text.to_vec());
        0x100 + text.len() as u16
    }
    fn field(&mut self, call: FieldCall<'_>) -> Result<(), CallbackError> {
        self.calls.push((
            call.text.map(<[u8]>::to_vec),
            call.field.offset,
            call.field.length,
            call.record_index,
            call.slot,
        ));
        Ok(())
    }
}

fn compile_with(
    fields: &[FieldDef],
    size: usize,
    text: &[u8],
    linkers: &mut Linkers,
    cb: &mut dyn Callbacks,
) -> Result<Compiled, TxtError> {
    let txt = TxtTable::parse("t.txt", text).expect("test text parses");
    compile_table("t.txt", &txt, fields, size, linkers, cb)
}

fn compile(fields: &[FieldDef], size: usize, text: &[u8], linkers: &mut Linkers) -> Compiled {
    compile_with(fields, size, text, linkers, &mut Mock::default()).expect("compiles")
}

fn code(s: &[u8]) -> u32 {
    u32::from_le_bytes(code4(s))
}

fn diag_kinds(c: &Compiled) -> Vec<DiagKind> {
    c.diagnostics.iter().map(|d| d.kind).collect()
}

#[test]
fn integer_vectors() {
    // (cell, u32) from field-types.md and txt-format.md.
    let cases: [(&[u8], u32); 35] = [
        (b"", 0),
        (b"5", 5),
        (b"12", 12),
        (b"-1", 0xFFFF_FFFF),
        (b"-12", 0xFFFF_FFF4),
        (b"300", 0x12C),
        (b"256", 0x100),
        (b"999", 0x3E7),
        (b" ", 0xFFFF_FFF0),
        (b" 5", 0xFFFF_FF65),
        (b" 1", 0xFFFF_FF61),
        (b"1 ", 0xFFFF_FFFA),
        (b"5 ", 0x22),
        (b"+5", 0xFFFF_FFD3),
        (b"12abc", 0x442B),
        (b"abc", 0x154B),
        (b"0x10", 0x1C2A),
        (b"1.5", 0x55),
        (b"1e3", 0x279),
        (b"-", 0),
        (b"-0", 0),
        (b"--5", 0x19),
        (b"--1", 0x1D),
        (b"5-", 0x2F),
        (b"007", 7),
        (b"*16", 0xFFFF_FDB8),
        (b"*12", 0xFFFF_FDB4),
        (b"65535", 0xFFFF),
        (b"65536", 0x10000),
        (b"-32768", 0xFFFF_8000),
        (b"2147483648", 0x8000_0000),
        (b"4294967295", 0xFFFF_FFFF),
        (b"4294967296", 0),
        (b"99999999999", 0x4876_E7FF),
        (b"\xE9", 0xFFFF_FFB9),
    ];
    for (cell, v) in cases {
        assert_eq!(parse_int(cell), v, "{cell:?}");
    }
    assert_eq!(parse_int(b"\x85"), 0xFFFF_FF55);
}

#[test]
fn bit_and_optional_byte() {
    let f = [field("b", T::Bit, 10, 16, "")];
    let mut l = Linkers::default();
    for (cell, set) in [
        ("1", true),
        ("2", true),
        ("-1", true),
        (" ", true),
        ("0", false),
        ("", false),
        ("-", false),
    ] {
        let text = format!("b\r\n{cell}\r\n");
        let c = compile(&f, 20, text.as_bytes(), &mut l);
        assert_eq!(c.records[17], if set { 0x04 } else { 0 }, "{cell:?}");
    }
    // u8?: empty writes nothing; "-" writes 0.
    let f = [
        field("b", T::Byte, 0, 0, ""),
        field("u", T::Unknown1, 1, 0, ""),
    ];
    let c = compile(&f, 1, b"b\tu\r\n7\t\r\n7\t-\r\n", &mut l);
    assert_eq!(c.records, [0x07, 0x00]);
}

#[test]
fn text_and_code_vectors() {
    let mut l = Linkers::default();
    let one = |t: T, len: u32, cell: &[u8], size: usize, l: &mut Linkers| {
        let mut text = b"a\r\n".to_vec();
        text.extend_from_slice(cell);
        text.extend_from_slice(b"\r\n");
        compile(&[field("a", t, len, 0, "")], size, &text, l).records
    };
    assert_eq!(one(T::Ascii, 4, b"abc", 5, &mut l), b"abc\0\0");
    assert_eq!(one(T::Ascii, 4, b"abcdef", 5, &mut l), b"abcd\0");
    assert_eq!(one(T::Ascii, 4, b"", 5, &mut l), [0; 5]);
    assert_eq!(one(T::Ascii, 2, b"NU0", 3, &mut l), b"NU\0");
    assert_eq!(one(T::Raw, 0, b"", 4, &mut l), b"    ");
    assert_eq!(one(T::Raw, 0, b"DT", 4, &mut l), b"DT  ");
    assert_eq!(one(T::Raw, 0, b"ab", 4, &mut l), b"ab  ");
    assert_eq!(one(T::Raw, 0, b"staff", 4, &mut l), b"staf");
    assert_eq!(one(T::Raw, 0, b"ring ", 4, &mut l), b"ring");
    assert_eq!(one(T::Raw, 0, b" a", 4, &mut l), b" a  ");
    // Missing RAW column: zeros, not spaces.
    let c = compile(
        &[field("a", T::Byte, 0, 4, ""), field("r", T::Raw, 0, 0, "")],
        5,
        b"a\r\n1\r\n",
        &mut l,
    );
    assert_eq!(c.records, [0, 0, 0, 0, 1]);
}

#[test]
fn own_key_text_types() {
    let mut l = Linkers::default();
    // UNKNOWN6 (key(str)), len 4: stores "abc" + NUL, registers "abc".
    let f = [field("k", T::Unknown6, 4, 0, "N")];
    let c = compile(&f, 4, b"k\r\nabcdef\r\n", &mut l);
    assert_eq!(c.records, b"abc\0");
    assert_eq!(l.name("N").unwrap().find(b"abc"), Some(0));
    assert_eq!(diag_kinds(&c), [DiagKind::TextCut]);
    // len 0: stores NUL, registers "".
    let f = [field("k", T::Unknown6, 0, 0, "N0")];
    let c = compile(&f, 1, b"k\r\nabc\r\n", &mut l);
    assert_eq!(c.records, [0]);
    assert_eq!(l.name("N0").unwrap().find(b""), Some(0));
    // UNKNOWN4 / UNKNOWN5 store code bytes, not the index.
    let c = compile(
        &[field("k", T::Unknown4, 0, 0, "C4")],
        1,
        b"k\r\nabc\r\n",
        &mut l,
    );
    assert_eq!(c.records, b"a");
    assert_eq!(l.code("C4").unwrap().find(code(b"abc")), Some(0));
    let c = compile(
        &[field("k", T::Unknown5, 0, 0, "C5")],
        2,
        b"k\r\nabc\r\n",
        &mut l,
    );
    assert_eq!(c.records, b"ab");
}

#[test]
fn code_linker_vectors() {
    let mut l = CodeLinker::default();
    assert_eq!(l.add(code(b"abc")), (0, false));
    assert_eq!(l.add(code(b"xyz")), (1, false));
    assert_eq!(l.add(code(b"abc")), (2, true));
    assert_eq!(l.find(0x2063_6262), Some(2)); // "bbc "
    for (s, i) in [
        (&b"abc"[..], Some(0)),
        (b"xyz", Some(1)),
        (b"bbc", Some(2)),
        (b"abd", None),
        (b"", None),
        (b"ABC", None),
    ] {
        assert_eq!(l.find(code(s)), i, "{s:?}");
    }

    let mut l = CodeLinker::default();
    l.add(code(b"abc"));
    l.add(code(b"abc"));
    assert_eq!(l.add(code(b"bbc")), (2, true));
    assert_eq!(
        (l.find(code(b"bbc")), l.find(code(b"cbc"))),
        (Some(1), Some(2))
    );

    let mut l = CodeLinker::default();
    for i in 0..5 {
        assert_eq!(l.add(code(b"")).0, i);
        assert_eq!(l.find(0x2020_2020 + i), Some(i));
    }
    assert_eq!(l.find(code(b"")), Some(0));
}

#[test]
fn name_linker_vectors() {
    let mut linkers = Linkers::default();
    let f = [field("n", T::NameToIndex, 0, 0, "N")];
    let c = compile(
        &f,
        2,
        b"n\r\nFire Bolt\r\nfire bolt\r\nIce\r\n\r\n",
        &mut linkers,
    );
    assert_eq!(c.records, [0, 0, 0, 0, 1, 0, 2, 0]);
    let n = linkers.name("N").unwrap();
    assert_eq!(n.len(), 3);
    assert_eq!(n.find(&name_key(b"FIRE BOLT").unwrap()), Some(0));
    assert_eq!(n.find(b""), Some(2));
    assert_eq!(n.find(b"fire"), None);

    let f = [field("n", T::Unknown6, 8, 0, "A")];
    let c = compile(&f, 8, b"n\r\na\r\nA\r\nb\r\n", &mut linkers);
    let a = linkers.name("A").unwrap();
    assert_eq!((a.find(b"a"), a.find(b"b"), a.len()), (Some(0), Some(2), 3));
    assert_eq!(diag_kinds(&c), [DiagKind::DupName]);

    let mut n = NameLinker::default();
    let long_a = name_key(&[b'q'; 40]).unwrap();
    let mut other = vec![b'q'; 31];
    other.extend_from_slice(b"zzzzzzzzz");
    assert_eq!(
        n.find_or_add(&long_a),
        n.find_or_add(&name_key(&other).unwrap())
    );

    // Non-ASCII in the first 31 bytes: E11; past them: accepted.
    let e = compile_with(
        &[field("n", T::NameToIndex, 0, 0, "E")],
        2,
        b"n\r\nCaf\xE9\r\n",
        &mut linkers,
        &mut Mock::default(),
    )
    .unwrap_err();
    assert_eq!(
        (e.code, e.line, e.column),
        (ErrorCode::E11, Some(2), Some(0))
    );
    let mut text = b"n\r\n".to_vec();
    text.extend_from_slice(&[b'a'; 35]);
    text.push(0xE9);
    text.extend_from_slice(b"aaaa\r\n");
    let c = compile(
        &[field("n", T::NameToIndex, 0, 0, "F")],
        2,
        &text,
        &mut linkers,
    );
    assert_eq!(diag_kinds(&c), [DiagKind::TextCut]);
}

#[test]
fn whole_record_vectors() {
    // k ASCIITOCODE@0 → L, n CODETOBYTE@4 → L, m CODETOWORD@6 → L2 (no
    // column), r RAW@8 (no column); 12.
    let mut l = Linkers::default();
    l.ensure("L2", LinkerKind::Code).unwrap();
    let f = [
        field("k", T::AsciiToCode, 0, 0, "L"),
        field("n", T::CodeToByte, 0, 4, "L"),
        field("m", T::CodeToWord, 0, 6, "L2"),
        field("r", T::Raw, 0, 8, ""),
    ];
    let c = compile(&f, 12, b"k\tn\r\na\tb\r\nb\ta\r\n", &mut l);
    assert_eq!(
        c.record(0),
        [0x61, 0x20, 0x20, 0x20, 0x01, 0, 0xFF, 0xFF, 0, 0, 0, 0]
    );
    assert_eq!(
        c.record(1),
        [0x62, 0x20, 0x20, 0x20, 0x00, 0, 0xFF, 0xFF, 0, 0, 0, 0]
    );
    assert!(l.code("L2").unwrap().is_empty());

    // Duplicate code: the record keeps the cell's code.
    let mut l = Linkers::default();
    let c = compile(
        &[field("k", T::AsciiToCode, 0, 0, "L")],
        4,
        b"k\r\nabc\r\nabc\r\n",
        &mut l,
    );
    assert_eq!(c.records, b"abc abc ");
    assert_eq!(l.code("L").unwrap().find(0x2063_6262), Some(1));
    assert_eq!(c.diagnostics[0].kind, DiagKind::DupCode);
    assert_eq!(c.diagnostics[0].line, 3);

    // Missing key column: no key.
    let mut l = Linkers::default();
    let f = [
        field("a", T::Byte, 0, 4, ""),
        field("k", T::AsciiToCode, 0, 0, "L"),
    ];
    let c = compile(&f, 5, b"a\r\n1\r\n", &mut l);
    assert_eq!(c.records, [0, 0, 0, 0, 1]);
    assert!(l.code("L").unwrap().is_empty());

    // Missing value is written last.
    let mut l = Linkers::default();
    l.ensure("N", LinkerKind::Name).unwrap();
    let f = [
        field("a", T::Word, 0, 0, ""),
        field("z", T::NameToWord, 0, 0, "N"),
    ];
    assert_eq!(compile(&f, 2, b"a\r\n5\r\n", &mut l).records, [0xFF, 0xFF]);

    // NAMETOWORD2 stores one byte.
    let c = compile(
        &[field("s", T::NameToWord2, 0, 0, "N")],
        2,
        b"s\r\nx\r\n",
        &mut l,
    );
    assert_eq!(c.records, [0xFF, 0x00]);
    assert_eq!(diag_kinds(&c), [DiagKind::LinkMiss]);

    // NAMETODWORD with N = {x → 0}.
    let mut l = Linkers::default();
    let Linker::Name(n) = l.ensure("N", LinkerKind::Name).unwrap() else {
        unreachable!()
    };
    n.find_or_add(b"x");
    let f = [
        field("d", T::NameToDword, 0, 0, "N"),
        field("e", T::NameToDword, 0, 4, "N"),
    ];
    let c = compile(&f, 8, b"d\r\ny\r\nX\r\n", &mut l);
    assert_eq!(c.record(0), [0xFF; 8]);
    assert_eq!(c.record(1), [0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);

    // A shared code linker across tables.
    let mut l = Linkers::default();
    compile(
        &[field("c", T::AsciiToCode, 0, 0, "I")],
        4,
        b"c\r\nx\r\ny\r\n",
        &mut l,
    );
    compile(
        &[field("c", T::AsciiToCode, 0, 0, "I")],
        4,
        b"c\r\nz\r\n",
        &mut l,
    );
    let c = compile(
        &[field("i", T::Unknown3, 0, 0, "I")],
        4,
        b"i\r\nz\r\nx\r\n",
        &mut l,
    );
    assert_eq!(c.records, [2, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn callback_vectors() {
    let mut l = Linkers::default();
    let f = [
        field("a", T::Dword, 0, 0, ""),
        field("f", T::CustomLink, 7, 4, "cb:M"),
        field("g", T::CustomLink, 9, 8, "cb:M"),
    ];
    let mut m = Mock::default();
    let c = compile_with(&f, 12, b"f\ta\r\nxy\t5\r\n\t6\r\n", &mut l, &mut m).unwrap();
    assert_eq!(
        m.calls,
        [
            (Some(b"xy".to_vec()), 4, 7, 0, 0),
            (None, 8, 9, 0, 2),
            (Some(Vec::new()), 4, 7, 1, 0),
            (None, 8, 9, 1, 2),
        ]
    );
    assert_eq!(c.record(0), [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(c.record(1), [6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    let f = [
        field("k", T::KeyToWord, 0, 0, ""),
        field("m", T::KeyToWord, 0, 2, ""),
    ];
    let mut m = Mock::default();
    let c = compile_with(&f, 4, b"k\r\nabc\r\n", &mut l, &mut m).unwrap();
    assert_eq!(m.keys, [b"abc".to_vec()]);
    assert_eq!(c.records, [3, 1, 0, 0]);

    let mut text = b"k\r\n".to_vec();
    text.extend_from_slice(&[b'x'; 300]);
    text.extend_from_slice(b"\r\n");
    let mut m = Mock::default();
    let c = compile_with(&f[..1], 2, &text, &mut l, &mut m).unwrap();
    assert_eq!(m.keys[0].len(), 256);
    assert_eq!(diag_kinds(&c), [DiagKind::TextCut]);
}

#[test]
fn diagnostics_vectors() {
    let mut l = Linkers::default();
    let byte = |cell: &str, l: &mut Linkers| {
        let c = compile(
            &[field("a", T::Byte, 0, 0, "")],
            1,
            format!("a\r\n{cell}\r\n").as_bytes(),
            l,
        );
        (c.records[0], diag_kinds(&c))
    };
    assert_eq!(byte("999", &mut l), (0xE7, vec![DiagKind::IntRange]));
    assert_eq!(byte(" ", &mut l), (0xF0, vec![DiagKind::IntSyntax]));
    assert_eq!(byte("-", &mut l), (0x00, vec![DiagKind::IntSyntax]));
    assert_eq!(byte("", &mut l), (0x00, vec![]));
    assert_eq!(byte("-128", &mut l), (0x80, vec![]));
    assert_eq!(byte("255", &mut l), (0xFF, vec![]));
    assert_eq!(byte("-129", &mut l), (0x7F, vec![DiagKind::IntRange]));
    let word = compile(
        &[field("a", T::Word, 0, 0, "")],
        2,
        b"a\r\n65535\r\n65536\r\n",
        &mut l,
    );
    assert_eq!(word.records, [0xFF, 0xFF, 0, 0]);
    assert_eq!(diag_kinds(&word), [DiagKind::IntRange]);
    let dword = compile(
        &[field("a", T::Dword, 0, 0, "")],
        4,
        b"a\r\n-1\r\n4294967296\r\n",
        &mut l,
    );
    assert_eq!(dword.records, [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0]);
    assert_eq!(diag_kinds(&dword), [DiagKind::IntRange]);
    let bit = compile(&[field("a", T::Bit, 0, 0, "")], 1, b"a\r\n2\r\n", &mut l);
    assert_eq!(
        (bit.records[0], diag_kinds(&bit)),
        (1, vec![DiagKind::IntRange])
    );
    // CODETOWORD `staff` looks up `staf`: TextCut.
    let c = compile(
        &[field("k", T::AsciiToCode, 0, 0, "S")],
        4,
        b"k\r\nstaf\r\n",
        &mut l,
    );
    assert!(c.diagnostics.is_empty());
    let c = compile(
        &[field("a", T::CodeToWord, 0, 0, "S")],
        2,
        b"a\r\nstaff\r\n",
        &mut l,
    );
    assert_eq!(
        (c.records.clone(), diag_kinds(&c)),
        (vec![0, 0], vec![DiagKind::TextCut])
    );
    // Empty lookup that misses: no LinkMiss.
    l.ensure("E", LinkerKind::Name).unwrap();
    let c = compile(
        &[field("a", T::NameToWord, 0, 0, "E")],
        2,
        b"a\r\nzzz\r\n\r\n",
        &mut l,
    );
    assert_eq!(c.records, [0xFF; 4]);
    assert_eq!(diag_kinds(&c), [DiagKind::LinkMiss]);
}

#[test]
fn binding_diagnostics() {
    let mut l = Linkers::default();
    let f = [
        field("name", T::Byte, 0, 0, ""),
        field("level", T::Byte, 0, 1, ""),
    ];
    let c = compile(&f, 2, b"Name\tLEVEL\tname\r\n1\t2\t3\r\n", &mut l);
    assert_eq!(c.records, [1, 2]);
    assert_eq!(c.diagnostics[0].kind, DiagKind::DupColumn);
    assert_eq!(
        (c.diagnostics[0].line, c.diagnostics[0].column),
        (1, Some(2))
    );
}

fn e13(fields: &[FieldDef], size: usize, text: &[u8]) -> ErrorCode {
    let mut l = Linkers::default();
    l.ensure("N", LinkerKind::Name).unwrap();
    compile_with(fields, size, text, &mut l, &mut Mock::default())
        .map(|_| ErrorCode::E1)
        .unwrap_or_else(|e| e.code)
}

#[test]
fn field_list_checks() {
    const OK: ErrorCode = ErrorCode::E1; // stands for "compiled"
    let t = b"q\r\n1\r\n";
    assert_eq!(
        e13(
            &[field("x", T::Byte, 0, 0, ""), field("X", T::Byte, 0, 1, "")],
            2,
            t
        ),
        ErrorCode::E13
    );
    assert_eq!(e13(&[field("", T::Byte, 0, 0, "")], 1, t), ErrorCode::E13);
    let many = |n: usize| {
        (0..n)
            .map(|i| field(&format!("f{i}"), T::Byte, 0, 0, ""))
            .collect::<Vec<_>>()
    };
    assert_eq!(e13(&many(281), 1, t), ErrorCode::E13);
    assert_eq!(e13(&many(279), 1, t), OK);
    assert_eq!(e13(&[field("a", T::Byte, 0, 0, "")], 0, t), ErrorCode::E13);
    assert_eq!(
        e13(
            &[
                field("a", T::Unknown1, 2, 0, ""),
                field("b", T::Byte, 0, 1, "")
            ],
            2,
            t
        ),
        ErrorCode::E13
    );
    assert_eq!(
        e13(
            &[
                field("a", T::Unknown1, 2, 0, ""),
                field("b", T::Unknown1, 2, 1, "")
            ],
            2,
            t
        ),
        OK
    );
    assert_eq!(
        e13(&[field("a", T::Word, 0, 11, "")], 12, t),
        ErrorCode::E13
    );
    assert_eq!(e13(&[field("a", T::Word, 0, 10, "")], 12, t), OK);
    assert_eq!(
        e13(&[field("a", T::Ascii, 4, 8, "")], 12, t),
        ErrorCode::E13
    );
    assert_eq!(e13(&[field("a", T::Ascii, 3, 8, "")], 12, t), OK);
    assert_eq!(e13(&[field("a", T::Bit, 95, 0, "")], 12, t), OK);
    assert_eq!(e13(&[field("a", T::Bit, 96, 0, "")], 12, t), ErrorCode::E13);
    assert_eq!(
        e13(&[field("a", T::CodeToWord, 0, 0, "N")], 2, t),
        ErrorCode::E13
    );
    assert_eq!(
        e13(&[field("a", T::CodeToWord, 0, 0, "absent")], 2, t),
        ErrorCode::E13
    );
}

#[test]
fn slot_limit() {
    let f = [field("zz", T::Byte, 0, 0, "")];
    let text = |cols: usize| {
        let mut t = vec![b'c'];
        for _ in 1..cols {
            t.extend_from_slice(b"\tc");
        }
        t.extend_from_slice(b"\r\n");
        t.extend(std::iter::repeat_n(b'\t', cols - 1));
        t.extend_from_slice(b"\r\n");
        t
    };
    assert_eq!(e13(&f, 1, &text(280)), ErrorCode::E14);
    assert_eq!(e13(&f, 1, &text(279)), ErrorCode::E1);
}

#[test]
fn range_linker_codes() {
    let Linker::Code(r) = range_linker() else {
        unreachable!()
    };
    for (i, c) in [&b"none"[..], b"h2h", b"rng", b"both", b"loc"]
        .iter()
        .enumerate()
    {
        assert_eq!(r.find(code(c)), Some(i as u32));
    }
}

#[test]
fn param_callback() {
    let strings = StringTables::default();
    let mut cb = StdCallbacks::new(&strings);
    let mut l = Linkers::default();
    let Linker::Name(s) = l.ensure("skills.skill", LinkerKind::Name).unwrap() else {
        unreachable!()
    };
    s.find_or_add(b"attack");
    s.find_or_add(b"battle command");
    let f = [FieldDef::new("p", T::CalcToDword, 0, 0, Link::Param)];
    let c = compile_with(
        &f,
        4,
        b"p\r\nBattle Command\r\n41\r\n12abc\r\n-5\r\nnothing\r\n\r\n",
        &mut l,
        &mut cb,
    )
    .unwrap();
    let values: Vec<u32> = c
        .records
        .chunks(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(values, [1, 41, 12, (-5i32) as u32, 0, 0]);
}

#[test]
fn calc_callback_appends() {
    let strings = StringTables::default();
    let mut cb = StdCallbacks::new(&strings);
    let mut l = Linkers::default();
    let f = [
        FieldDef::new(
            "calc1",
            T::CalcToDword,
            0,
            0,
            Link::Calc(CalcBuffer::ItemsCode),
        ),
        FieldDef::new(
            "len",
            T::CalcToDword,
            0,
            4,
            Link::Calc(CalcBuffer::ItemsCode),
        ),
    ];
    let c = compile_with(&f, 8, b"calc1\r\n5\r\n\r\n750\r\n", &mut l, &mut cb).unwrap();
    let values: Vec<u32> = c
        .records
        .chunks(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    // `len` is missing: 0xFFFFFFFF.
    assert_eq!(values, [0, u32::MAX, u32::MAX, u32::MAX, 3, u32::MAX]);
    assert_eq!(
        cb.buffers[&CalcBuffer::ItemsCode],
        [0x07, 0x05, 0x00, 0x08, 0xEE, 0x02, 0x00]
    );
}
