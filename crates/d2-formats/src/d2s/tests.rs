// Spec: specs/formats/d2s.md
//! Synthetic saves built in memory from the spec's rules and test vectors;
//! no save file is read or committed.

use super::*;

/// The 1.14d `itemstatcost` save columns measured in §7.1 (359 rows; ids
/// 0–15 saved, none with `CSvParam` / `CSvSigned`), plus test overrides.
struct Tables {
    extra: Vec<(u16, StatSave)>,
}

impl Tables {
    fn v114d() -> Self {
        Tables { extra: Vec::new() }
    }
}

impl SaveTables for Tables {
    fn stat_save(&self, id: u16) -> Option<StatSave> {
        if let Some(&(_, c)) = self.extra.iter().find(|(i, _)| *i == id) {
            return Some(c);
        }
        if id >= 359 {
            return None;
        }
        let bits = match id {
            0..=4 => 10,
            5 => 8,
            6..=11 => 21,
            12 => 7,
            13 => 32,
            14 | 15 => 25,
            _ => 0,
        };
        Some(StatSave {
            bits,
            param: 0,
            signed: false,
        })
    }

    /// Test items: `4A 4D`, a length byte (the whole entry), then filler.
    fn item_entry_len(&self, buf: &[u8]) -> Result<usize, String> {
        match buf {
            [0x4A, 0x4D, n, ..] if usize::from(*n) >= 3 => Ok(usize::from(*n)),
            _ => Err("not a test item".into()),
        }
    }
}

fn item(n: u8, fill: u8) -> ItemEntry {
    let mut bytes = vec![0x4A, 0x4D, n];
    bytes.resize(usize::from(n), fill);
    ItemEntry { bytes }
}

fn stats_bytes(e: &[StatEntry]) -> Vec<u8> {
    write_stats(e, &Tables::v114d())
}

fn st(id: u16, value: i32) -> StatEntry {
    StatEntry {
        id,
        layer: 0,
        value,
    }
}

/// A level-1 character with every section, items in all lists.
fn sample(expansion: bool) -> D2s {
    let mut h = Header::default();
    h.set_name(b"Tester").unwrap();
    h.class = 4;
    h.status = if expansion { status::EXPANSION } else { 0 };
    h.create_time = 0x6000_0000;
    h.save_time = 0x6000_1000;
    h.hotkeys[0] = Slot::encode(36, true, 0).unwrap();
    h.mouse[1] = Slot::encode(0, false, 0).unwrap();
    h.towns = [0x80, 0, 0];
    h.map_seed = 0x1234_5678;
    h.hireling = Hireling {
        flags: 0,
        seed: 77,
        name_index: 3,
        id: 1,
        experience: 1000,
        rest: [0; 16],
    };
    let mut quests = Quests::default();
    quests.records[0][0] = 1;
    let mut wp = Waypoints::default();
    wp.records[0][2] |= 0x02;
    let mut npcs = Npcs::default();
    npcs.b[0][0] = 0x06;
    let body = Body {
        quests,
        waypoints: wp,
        npcs,
        stats: Stats::Bits(vec![st(0, 30), st(6, 55 << 8), st(12, 1), st(13, 500)]),
        skills: {
            let mut s = vec![0u8; 30];
            s[0] = 1;
            s
        },
        items: vec![item(10, 0xAA), item(5, 0xBB)],
        corpses: vec![Corpse {
            unk: 0,
            x: 5000,
            y: 6000,
            items: vec![item(4, 0xCC)],
        }],
        hireling_items: expansion.then(|| Some(vec![item(6, 0xDD)])),
        golem: expansion.then(|| Golem {
            flag: 1,
            item: Some(item(7, 0xEE)),
        }),
        trailing: Vec::new(),
    };
    D2s {
        header: h,
        body: Some(body),
    }
}

fn opts(expansion: bool) -> ReadOptions {
    ReadOptions {
        expansion,
        game: None,
    }
}

fn code_of(file: &[u8], o: &ReadOptions) -> Option<u8> {
    read(file, o, &Tables::v114d()).unwrap_err().internal()
}

/// Re-stores size and checksum after a hand edit.
fn fix(mut b: Vec<u8>) -> Vec<u8> {
    finish(&mut b);
    b
}

// -------------------------------------------------------------- checksum

// Covers: specs/formats/d2s.md §3 r1
#[test]
fn checksum_vectors() {
    assert_eq!(checksum(&[1, 2, 3]), 0x0000_000B);
    assert_eq!(checksum(&[0xFF; 4]), 0x0000_0EF1);
    // s = 0x80000000 then byte 0: rotl gives 1.
    assert_eq!(0x8000_0000u32.rotate_left(1).wrapping_add(0), 1);
}

// Covers: specs/formats/d2s.md §2.6, §3 r2, §3 r3, §3 r4, §9 r1
#[test]
fn stub_vector() {
    // Test vector: zero header, magic, version, size, "Test", status 0x21,
    // +0x29 0x10, +0x2A 0x1E, +0x2B 1.
    let mut b = vec![0u8; HEADER_SIZE];
    b[0..4].copy_from_slice(&[0x55, 0xAA, 0x55, 0xAA]);
    b[4] = 0x60;
    b[8..10].copy_from_slice(&[0x4F, 0x01]);
    b[0x14..0x18].copy_from_slice(b"Test");
    b[0x24] = 0x21;
    b[0x29] = 0x10;
    b[0x2A] = 0x1E;
    b[0x2B] = 0x01;
    assert_eq!(checksum(&b), 0xC737_3BCA);
    let mut f = b.clone();
    finish(&mut f);
    assert_eq!(&f[0x0C..0x10], &[0xCA, 0x3B, 0x37, 0xC7]);
    let s = read(&f, &opts(true), &Tables::v114d()).unwrap();
    assert!(s.body.is_none(), "new character");
    assert_eq!(s.header.name_bytes(), b"Test");
    // The model writes the same bytes back.
    assert_eq!(write(&s, &Tables::v114d()).unwrap(), f);
}

// Covers: specs/formats/d2s.md §2.6
#[test]
fn new_stub_layout() {
    let s = D2s::new_stub(b"Nec", 2, status::HARDCORE, 1234).unwrap();
    let f = write(&s, &Tables::v114d()).unwrap();
    assert_eq!(f.len(), HEADER_SIZE);
    assert_eq!(u16_at(&f, 0x24), 0x0005);
    assert_eq!(&f[0x29..0x2C], &[0x10, 0x1E, 1]);
    assert_eq!(u32_at(&f, 0x2C), 1234);
    assert_eq!(u32_at(&f, 0x30), 1234);
    assert_eq!(&f[0x88..0x98], &STUB_COMPONENTS);
    assert_eq!(&f[0x98..0xA8], &[0xFF; 16]);
    assert!(f[0x34..0x88].iter().all(|&b| b == 0));
    assert!(f[0xA8..].iter().all(|&b| b == 0));
    // Class 5 / 6 get 0x20.
    let d = D2s::new_stub(b"Dru", 5, 0, 0).unwrap();
    assert_eq!(d.header.status, 0x21);
    assert!(D2s::new_stub(b"SixteenCharsLong", 0, 0, 0).is_none());
}

// -------------------------------------------------------- header checks

// Covers: specs/formats/d2s.md §2.2 r2, §2.2 r3, §10 r1
#[test]
fn header_check_errors() {
    let good = write(&D2s::new_stub(b"Test", 0, 0, 0).unwrap(), &Tables::v114d()).unwrap();
    // One byte changed after the checksum → 6 → result 14.
    let mut bad = good.clone();
    bad[0x40] ^= 1;
    let e = read(&bad, &opts(false), &Tables::v114d()).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(6), Some(14)));
    // Size 336 with the checksum recomputed → 5 → result 14.
    let mut sz = good.clone();
    sz[8..12].copy_from_slice(&336u32.to_le_bytes());
    sz[0x0C..0x10].fill(0);
    let c = checksum(&sz);
    sz[0x0C..0x10].copy_from_slice(&c.to_le_bytes());
    let e = read(&sz, &opts(false), &Tables::v114d()).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(5), Some(14)));
    // Version 0x61 → 7 → result 1.
    let mut v = good.clone();
    v[4] = 0x61;
    let e = read(&fix(v), &opts(false), &Tables::v114d()).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(7), Some(1)));
}

// Covers: specs/formats/d2s.md §1 r6, §10 r2, §2.2 r1
#[test]
fn dispatch_errors() {
    let t = Tables::v114d();
    assert_eq!(
        read(&[0x55, 0xAA], &opts(false), &t),
        Err(D2sError::NotASave)
    );
    assert_eq!(D2sError::NotASave.result(), Some(9));
    let mut b = vec![0u8; 16];
    b[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    b[4] = 0x5B;
    assert_eq!(read(&b, &opts(false), &t), Err(D2sError::Legacy(0x5B)));
    b[4] = 0x60;
    assert_eq!(code_of(&b, &opts(false)), Some(4));
}

// Covers: specs/formats/d2s.md §10 r4
#[test]
fn file_cut_at_0x2000() {
    let s = sample(true);
    let mut f = write(&s, &Tables::v114d()).unwrap();
    // Pad the model past the buffer by hand: the cut copy fails the checksum.
    f.resize(MAX_FILE + 10, 0);
    finish(&mut f);
    assert_eq!(code_of(&f, &opts(true)), Some(6));
}

// Covers: specs/formats/d2s.md §2.2 r6, §2.2 r7, §9 r1
#[test]
fn class_and_new_flag() {
    let t = Tables::v114d();
    let mut s = D2s::new_stub(b"Test", 7, 0, 0).unwrap();
    // Class 7 passes the header check.
    assert!(read(&write(&s, &t).unwrap(), &opts(false), &t).is_ok());
    s.header.class = 8;
    let f = fix(s.header.to_bytes().to_vec());
    assert_eq!(code_of(&f, &opts(false)), Some(4));
    // New flag with more data → 2 → result 9.
    let mut f = write(&D2s::new_stub(b"Test", 0, 0, 0).unwrap(), &t).unwrap();
    f.push(0);
    let f = fix(f);
    let e = read(&f, &opts(false), &t).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(2), Some(9)));
}

// Covers: specs/formats/d2s.md §2.2 r4, §2.2 r5, §2.3
#[test]
fn context_checks() {
    let mut h = Header::default();
    h.set_name(b"Hero").unwrap();
    let game = |name: &[u8], exp: bool, hc: bool, d: u8| GameContext {
        client_name: name.to_vec(),
        expansion: exp,
        hardcore: hc,
        difficulty: d,
    };
    let c = |h: &Header, g: GameContext| check_header(h, &g).err().and_then(|e| e.internal());
    assert_eq!(c(&h, game(b"hERO", false, false, 0)), None);
    assert_eq!(c(&h, game(b"", false, false, 0)), Some(3));
    assert_eq!(c(&h, game(b"Other", false, false, 0)), Some(7));
    assert_eq!(c(&h, game(b"Hero", true, false, 0)), Some(9));
    h.status = status::EXPANSION;
    assert_eq!(c(&h, game(b"Hero", false, false, 0)), Some(8));
    h.status = status::EXPANSION | status::HARDCORE | status::DEAD;
    assert_eq!(c(&h, game(b"Hero", true, true, 0)), Some(10));
    h.status = status::EXPANSION | status::HARDCORE;
    assert_eq!(c(&h, game(b"Hero", true, false, 0)), Some(11));
    h.status = status::EXPANSION;
    assert_eq!(c(&h, game(b"Hero", true, true, 0)), Some(12));
    // Progression thresholds: expansion 5 / 10, classic 4 / 8.
    h.status = status::EXPANSION | (4 << 8);
    assert_eq!(c(&h, game(b"Hero", true, false, 1)), Some(13));
    h.status = status::EXPANSION | (5 << 8);
    assert_eq!(c(&h, game(b"Hero", true, false, 1)), None);
    assert_eq!(c(&h, game(b"Hero", true, false, 2)), Some(14));
    h.status = 4 << 8;
    assert_eq!(c(&h, game(b"Hero", false, false, 1)), None);
    h.status = 7 << 8;
    assert_eq!(c(&h, game(b"Hero", false, false, 2)), Some(14));
    h.status = 8 << 8;
    assert_eq!(c(&h, game(b"Hero", false, false, 2)), None);
    // Run from `read`: the name check precedes the class check.
    let mut s = D2s::new_stub(b"Hero", 0, 0, 0).unwrap();
    s.header.class = 9;
    let f = fix(s.header.to_bytes().to_vec());
    let o = ReadOptions {
        expansion: false,
        game: Some(game(b"Other", false, false, 0)),
    };
    assert_eq!(code_of(&f, &o), Some(7));
}

// Covers: specs/formats/d2s.md §2.1, §2.5 r2
#[test]
fn header_field_offsets() {
    let s = sample(true);
    let f = write(&s, &Tables::v114d()).unwrap();
    assert_eq!(&f[0x14..0x1A], b"Tester");
    assert_eq!(f[0x28], 4);
    assert_eq!(u32_at(&f, 0x34), 0xFFFF_FFFF);
    assert_eq!(&f[0x38..0x3C], &[0x24, 0x80, 0, 0]);
    assert_eq!(f[0xA8], 0x80);
    assert_eq!(u32_at(&f, 0xAB), 0x1234_5678);
    assert_eq!(u32_at(&f, 0xB3), 77);
    assert_eq!(u16_at(&f, 0xB7), 3);
    assert_eq!(u16_at(&f, 0xB9), 1);
    assert_eq!(u32_at(&f, 0xBB), 1000);
    assert!(s.header.hireling.is_present());
    assert!(!Hireling::default().is_present());
}

// ------------------------------------------------------------ hotkeys

// Covers: specs/formats/d2s.md §2.4 r1, §2.4 r4
#[test]
fn hotkey_vectors() {
    let s = Slot::encode(36, true, 0).unwrap();
    assert_eq!((s.code.to_le_bytes(), s.item), ([0x24, 0x80], 0));
    assert_eq!(s.decode(), (36, true, -1));
    let n = Slot::encode(-1, false, 0).unwrap();
    assert_eq!(n, Slot::NONE);
    assert_eq!(n.decode(), (-1, false, -1));
    assert!(Slot::encode(0x8000, false, 0).is_none());
    assert_eq!(
        Slot {
            code: 0x0005,
            item: 3
        }
        .decode(),
        (5, false, 3)
    );
}

// -------------------------------------------------------------- stats

// Covers: specs/formats/d2s.md §7.1 r1, §7.1 r2, §7.1 r3, §7.1 r4
#[test]
fn stats_vectors() {
    assert_eq!(
        stats_bytes(&[st(0, 30), st(6, 14080), st(12, 1)]),
        [0x67, 0x66, 0x00, 0x3C, 0x30, 0x00, 0x70, 0x03, 0x18, 0x04, 0xFE, 0x03]
    );
    assert_eq!(stats_bytes(&[]), [0x67, 0x66, 0xFF, 0x01]);
    assert_eq!(
        stats_bytes(&[st(14, 40_000_000)]),
        [0x67, 0x66, 0x0E, 0xFE, 0xFF, 0xFF, 0xFF, 0x07]
    );
    // CSvBits 0 and ids past the table are not written.
    assert_eq!(
        stats_bytes(&[st(16, 5), st(400, 1)]),
        [0x67, 0x66, 0xFF, 0x01]
    );
}

// Covers: specs/formats/d2s.md §7.1 r2
#[test]
fn stat_clamps() {
    let u = StatSave {
        bits: 7,
        param: 0,
        signed: false,
    };
    assert_eq!(clamp_stat(-5, u), 0);
    assert_eq!(clamp_stat(200, u), 127);
    assert_eq!(clamp_stat(50, u), 50);
    let s = StatSave { signed: true, ..u };
    assert_eq!(clamp_stat(-100, s), 0x40); // −64 in 7 bits
    assert_eq!(clamp_stat(100, s), 63);
    assert_eq!(clamp_stat(-1, s), 0x7F);
    let w = StatSave { bits: 32, ..u };
    assert_eq!(clamp_stat(-1, w), 0xFFFF_FFFF);
}

// Covers: specs/formats/d2s.md §7.1 r5, §7.1 r6
#[test]
fn stats_reader_columns() {
    // A signed stat with a param: the layer is read sign-extended, the
    // value sign-extended.
    let t = Tables {
        extra: vec![(
            20,
            StatSave {
                bits: 6,
                param: 4,
                signed: true,
            },
        )],
    };
    let mut s = sample(false);
    s.body.as_mut().unwrap().stats = Stats::Bits(vec![StatEntry {
        id: 20,
        layer: 0xFFFF,
        value: -3,
    }]);
    let f = write(&s, &t).unwrap();
    let back = read(&f, &opts(false), &t).unwrap();
    assert_eq!(
        back.body.unwrap().stats,
        Stats::Bits(vec![StatEntry {
            id: 20,
            layer: 0xFFFF,
            value: -3
        }])
    );
    // Stat 16 (CSvBits 0) on the wire → 18 → result 4: write it with an
    // override table, read with the 1.14d one.
    let t16 = Tables {
        extra: vec![(
            16,
            StatSave {
                bits: 5,
                param: 0,
                signed: false,
            },
        )],
    };
    let mut s = sample(false);
    s.body.as_mut().unwrap().stats = Stats::Bits(vec![st(16, 1)]);
    let f = write(&s, &t16).unwrap();
    let e = read(&f, &opts(false), &Tables::v114d()).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(18), Some(4)));
}

// Covers: specs/formats/d2s.md §7.1 r6
#[test]
fn stats_without_terminator_rejected() {
    // Edge case 2: the game never ends; d2rs rejects with 18. Build a file
    // ending inside the stats bit stream.
    let s = sample(false);
    let f = write(&s, &Tables::v114d()).unwrap();
    let cut = fix(f[..STATS_OFFSET + 4].to_vec());
    assert_eq!(code_of(&cut, &opts(false)), Some(18));
}

// Covers: specs/formats/d2s.md §7.1 r7
#[test]
fn legacy_mask_stats() {
    let mut s = sample(false);
    s.header.version = 0x5E;
    s.body.as_mut().unwrap().stats = Stats::Mask {
        mask: vec![0x01, 0x10],
        values: vec![(0, 30), (12, 1)],
    };
    let f = write(&s, &Tables::v114d()).unwrap();
    assert_eq!(
        &f[STATS_OFFSET..STATS_OFFSET + 12],
        &[0x67, 0x66, 0x01, 0x10, 0x1E, 0, 0, 0, 0x01, 0, 0, 0]
    );
    let back = read(&f, &opts(false), &Tables::v114d()).unwrap();
    assert_eq!(back.body, s.body);
    assert_eq!(write(&back, &Tables::v114d()).unwrap(), f);
    assert_eq!(
        back.body.unwrap().stats.entries(),
        vec![st(0, 30), st(12, 1)]
    );
}

// ------------------------------------------------------------- skills

// Covers: specs/formats/d2s.md §7.2 r1, §7.2 r2, §7.2 r3
#[test]
fn skills_section() {
    let mut s = sample(false);
    s.header.class = 0;
    s.body.as_mut().unwrap().skills = vec![0; 30];
    let f = write(&s, &Tables::v114d()).unwrap();
    let at = STATS_OFFSET + stats_bytes(&[st(0, 30), st(6, 55 << 8), st(12, 1), st(13, 500)]).len();
    let mut want = vec![0x69, 0x66];
    want.extend([0u8; 30]);
    assert_eq!(&f[at..at + 32], &want[..]);
    // Writer error 2 for class 7.
    s.header.class = 7;
    assert_eq!(write(&s, &Tables::v114d()), Err(WriteError::Class(7)));
    // Bad marker and a short section → 19 → result 5.
    s.header.class = 0;
    let mut f = write(&s, &Tables::v114d()).unwrap();
    f[at] = 0;
    let e = read(&fix(f), &opts(false), &Tables::v114d()).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(19), Some(5)));
    let f = write(&s, &Tables::v114d()).unwrap();
    assert_eq!(code_of(&fix(f[..at + 10].to_vec()), &opts(false)), Some(19));
}

// ------------------------------------------------------- fixed sections

// Covers: specs/formats/d2s.md §1 r1, §4 r1, §4 r2, §4 r3, §5, §6 text, §6 r1, §6 r4
#[test]
fn fixed_section_layout_and_errors() {
    let s = sample(false);
    let f = write(&s, &Tables::v114d()).unwrap();
    assert_eq!(
        &f[QUEST_OFFSET..QUEST_OFFSET + 10],
        &[0x57, 0x6F, 0x6F, 0x21, 6, 0, 0, 0, 0x2A, 0x01]
    );
    assert_eq!(f[QUEST_OFFSET + 10], 1);
    assert_eq!(
        &f[WAYPOINT_OFFSET..WAYPOINT_OFFSET + 11],
        &[0x57, 0x53, 1, 0, 0, 0, 0x50, 0, 0x02, 0x01, 0x03]
    );
    assert_eq!(&f[NPC_OFFSET..NPC_OFFSET + 4], &[0x01, 0x77, 0x34, 0]);
    assert_eq!(f[NPC_OFFSET + 0x1C], 0x06);
    assert_eq!(&f[STATS_OFFSET..STATS_OFFSET + 2], &[0x67, 0x66]);
    // Each marker broken → its code.
    for (at, code, result) in [
        (QUEST_OFFSET, 15, 2),
        (QUEST_OFFSET + 4, 15, 2),
        (WAYPOINT_OFFSET, 16, 3),
        (NPC_OFFSET, 17, 11),
        (STATS_OFFSET, 18, 4),
    ] {
        let mut b = f.clone();
        b[at] ^= 0x40;
        let e = read(&fix(b), &opts(false), &Tables::v114d()).unwrap_err();
        assert_eq!(
            (e.internal(), e.result()),
            (Some(code), Some(result)),
            "at {at:#x}"
        );
    }
    // The quest size, waypoint u32 / size / pad bytes and the NPC size are
    // not checked.
    let mut b = f.clone();
    b[QUEST_OFFSET + 8] = 0;
    b[WAYPOINT_OFFSET + 2] = 9;
    b[WAYPOINT_OFFSET + 6] = 0;
    b[WAYPOINT_OFFSET + 8 + 16] = 7;
    b[NPC_OFFSET + 2] = 0;
    assert!(read(&fix(b), &opts(false), &Tables::v114d()).is_ok());
}

// Covers: specs/formats/d2s.md §5
#[test]
fn waypoint_record_magic() {
    let f = write(&sample(false), &Tables::v114d()).unwrap();
    for (magic, ok) in [
        ([0x02, 0x01], true),
        ([0x01, 0x01], true),
        ([0, 0], true),
        ([0x03, 0x01], false),
    ] {
        let mut b = f.clone();
        b[WAYPOINT_OFFSET + 8 + 24..WAYPOINT_OFFSET + 8 + 26].copy_from_slice(&magic);
        let r = read(&fix(b), &opts(false), &Tables::v114d());
        assert_eq!(r.is_ok(), ok, "{magic:?}");
        if !ok {
            assert_eq!(r.unwrap_err().internal(), Some(16));
        }
    }
    assert_eq!(Waypoints::bit(0), Some((2, 1)));
    assert_eq!(Waypoints::bit(9), Some((3, 2)));
    assert_eq!(Waypoints::bit(0x70), None);
}

// Covers: specs/formats/d2s.md §6 r2
#[test]
fn npc_bits() {
    for (class, bit) in [
        (147, 1),
        (148, 2),
        (150, 3),
        (155, 4),
        (154, 5),
        (265, 6),
        (175, 7),
        (178, 10),
        (202, 11),
        (200, 12),
        (210, 13),
        (201, 14),
        (198, 15),
        (199, 16),
        (244, 17),
        (246, 19),
        (251, 20),
        (257, 26),
        (264, 27),
        (297, 28),
        (511, 29),
        (515, 33),
        (520, 34),
        (-1, 0),
        (149, 0),
        (0, 0),
    ] {
        assert_eq!(npc_bit(class), bit, "class {class}");
    }
}

// ---------------------------------------------------------------- items

// Covers: specs/formats/d2s.md §8.1 r1, §8.1 r2, §8.3 r2, §8.4 r1, §8.5 r1
#[test]
fn item_section_bytes() {
    let mut s = sample(true);
    {
        let b = s.body.as_mut().unwrap();
        b.items.clear();
        b.corpses.clear();
        b.hireling_items = Some(None);
        b.golem = Some(Golem::default());
    }
    let f = write(&s, &Tables::v114d()).unwrap();
    // Empty player list, no corpse, no hireling list, no golem.
    assert!(f.ends_with(&[0x4A, 0x4D, 0, 0, 0x4A, 0x4D, 0, 0, 0x6A, 0x66, 0x6B, 0x66, 0x00]));
    // With items: counts and entries in order.
    let f = write(&sample(true), &Tables::v114d()).unwrap();
    let mut want = vec![0x4A, 0x4D, 2, 0];
    want.extend(item(10, 0xAA).bytes);
    want.extend(item(5, 0xBB).bytes);
    want.extend([0x4A, 0x4D, 1, 0, 0, 0, 0, 0]);
    want.extend(5000u32.to_le_bytes());
    want.extend(6000u32.to_le_bytes());
    want.extend([0x4A, 0x4D, 1, 0]);
    want.extend(item(4, 0xCC).bytes);
    want.extend([0x6A, 0x66, 0x4A, 0x4D, 1, 0]);
    want.extend(item(6, 0xDD).bytes);
    want.extend([0x6B, 0x66, 1]);
    want.extend(item(7, 0xEE).bytes);
    assert!(f.ends_with(&want));
}

// Covers: specs/formats/d2s.md §1 r2, §1 r4, §1 r5, §10 r6
#[test]
fn round_trip_both_games() {
    let t = Tables::v114d();
    for exp in [false, true] {
        let s = sample(exp);
        let f = write(&s, &t).unwrap();
        assert_eq!(u32_at(&f, 8) as usize, f.len());
        let back = read(&f, &opts(exp), &t).unwrap();
        let mut want = s.clone();
        want.header.file_size = f.len() as u32;
        want.header.checksum = u32_at(&f, 0x0C);
        assert_eq!(back, want);
        assert_eq!(write(&back, &t).unwrap(), f);
    }
    // A classic game does not read jf / kf: they stay as trailing bytes.
    let f = write(&sample(true), &t).unwrap();
    let back = read(&f, &opts(false), &t).unwrap();
    let b = back.body.as_ref().unwrap();
    assert_eq!(b.hireling_items, None);
    assert_eq!(&b.trailing[..2], &[0x6A, 0x66]);
    assert_eq!(write(&back, &t).unwrap(), f);
}

// Covers: specs/formats/d2s.md §8.2 r2, §8.2 r6
#[test]
fn player_list_errors() {
    let t = Tables::v114d();
    let f = write(&sample(false), &t).unwrap();
    let at = f.windows(4).position(|w| w == [0x4A, 0x4D, 2, 0]).unwrap();
    let mut b = f.clone();
    b[at] = 0;
    let e = read(&fix(b), &opts(false), &t).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(20), Some(7)));
    // An entry that does not decode.
    let mut b = f.clone();
    b[at + 4] = 0;
    assert_eq!(code_of(&fix(b), &opts(false)), Some(20));
    // An entry longer than the file.
    let mut b = f.clone();
    b[at + 6] = 0xFF;
    assert_eq!(code_of(&fix(b), &opts(false)), Some(20));
}

// Covers: specs/formats/d2s.md §8.3 r3, §8.3 r4
#[test]
fn corpse_rules() {
    let t = Tables::v114d();
    let mut s = sample(false);
    s.body.as_mut().unwrap().corpses[0].unk = 0xDEAD_BEEF;
    let f = write(&s, &t).unwrap();
    // The first u32 is skipped by the loader but kept by the model.
    assert_eq!(
        read(&f, &opts(false), &t).unwrap().body.unwrap().corpses[0].unk,
        0xDEAD_BEEF
    );
    // Corpse count 2 → 21 → result 8.
    let at = f.windows(4).position(|w| w == [0x4A, 0x4D, 1, 0]).unwrap();
    let mut b = f.clone();
    b[at + 2] = 2;
    let e = read(&fix(b), &opts(false), &t).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(21), Some(8)));
    // Marker and the corpse item list.
    let mut b = f.clone();
    b[at + 1] = 0;
    assert_eq!(code_of(&fix(b), &opts(false)), Some(21));
    let mut b = f.clone();
    b[at + 16] = 0;
    assert_eq!(code_of(&fix(b), &opts(false)), Some(21));
}

// Covers: specs/formats/d2s.md §8.4 r2, §8.5 r2
#[test]
fn expansion_sections() {
    let t = Tables::v114d();
    let f = write(&sample(true), &t).unwrap();
    let jf = f.windows(2).rposition(|w| w == [0x6A, 0x66]).unwrap();
    let kf = f.windows(2).rposition(|w| w == [0x6B, 0x66]).unwrap();
    // Missing jf and kf (fewer than 2 bytes left) → absent, no error.
    let cut = fix(f[..jf].to_vec());
    let s = read(&cut, &opts(true), &t).unwrap();
    assert_eq!(
        (
            s.body.as_ref().unwrap().hireling_items.clone(),
            s.body.unwrap().golem
        ),
        (None, None)
    );
    let cut = fix(f[..kf + 1].to_vec());
    let s = read(&cut, &opts(true), &t).unwrap();
    assert!(s.body.as_ref().unwrap().golem.is_none());
    assert_eq!(s.body.unwrap().trailing, vec![0x6B]);
    // Bad markers.
    let mut b = f.clone();
    b[jf] = 0;
    let e = read(&fix(b), &opts(true), &t).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(22), Some(10)));
    let mut b = f.clone();
    b[kf] = 0;
    let e = read(&fix(b), &opts(true), &t).unwrap_err();
    assert_eq!((e.internal(), e.result()), (Some(23), Some(10)));
    // No hireling in the header → no list is read after jf.
    let mut s = sample(true);
    s.header.hireling = Hireling::default();
    s.body.as_mut().unwrap().hireling_items = Some(None);
    let f2 = write(&s, &t).unwrap();
    let back = read(&f2, &opts(true), &t).unwrap();
    assert_eq!(back.body.as_ref().unwrap().hireling_items, Some(None));
    assert_eq!(back.body.unwrap().golem.unwrap().item, Some(item(7, 0xEE)));
    // g = 0: no item, the cursor moves past g.
    let mut s = sample(true);
    s.body.as_mut().unwrap().golem = Some(Golem::default());
    let f3 = write(&s, &t).unwrap();
    let back = read(&f3, &opts(true), &t).unwrap();
    assert_eq!(back.body.as_ref().unwrap().golem, Some(Golem::default()));
    assert!(back.body.unwrap().trailing.is_empty());
    // Golem item that does not decode → 23.
    let mut b = f.clone();
    b[kf + 3] = 0;
    assert_eq!(code_of(&fix(b), &opts(true)), Some(23));
}

// Covers: specs/formats/d2s.md §1 r3
#[test]
fn writer_overflow() {
    let mut s = sample(false);
    s.body.as_mut().unwrap().items = (0..40).map(|_| item(250, 0)).collect();
    assert!(matches!(
        write(&s, &Tables::v114d()),
        Err(WriteError::Overflow(_))
    ));
}

// Covers: specs/formats/d2s.md §10 r1
#[test]
fn result_table() {
    let want = [
        0, 14, 9, 14, 9, 14, 14, 1, 24, 23, 21, 20, 19, 17, 18, 2, 3, 11, 4, 5, 7, 8, 10, 10, 14,
        26, 25,
    ];
    for (i, &r) in want.iter().enumerate() {
        assert_eq!(load_result(i as u8), r);
    }
    assert_eq!(load_result(27), 1);
}

// M08: the round trip catches a changed byte in every section.
// Covers: specs/formats/d2s.md §3 r1
#[test]
fn checksum_catches_every_byte() {
    let f = write(&sample(true), &Tables::v114d()).unwrap();
    for at in (0..f.len()).filter(|&a| !(0x0C..0x10).contains(&a)) {
        let mut b = f.clone();
        b[at] ^= 0x01;
        assert!(
            read(&b, &opts(true), &Tables::v114d()).is_err(),
            "byte {at:#x}"
        );
    }
}
