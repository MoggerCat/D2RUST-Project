use d2_formats::cof::Cof;

use super::*;

const TSV: &str = include_str!("../../../../specs/render/unit-directions.tsv");

/// Compares `unit-directions.tsv` with the code row by row (docs/PLAN.md
/// "Machine tables in code"): header exact, 448 rows, every (directions,
/// dir64) pair exactly once, `cof_dir` and `dcc_dir` equal to
/// [`cof_direction`] and [`file_direction`].
fn check_directions(text: &str) -> Result<usize, String> {
    let mut lines = text.lines();
    if lines.next() != Some("directions\tdir64\tcof_dir\tdcc_dir") {
        return Err("header".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut rows = 0;
    for (i, line) in lines.enumerate() {
        let f: Vec<&str> = line.split('\t').collect();
        let n = |j: usize| -> Result<u8, String> {
            f.get(j)
                .and_then(|v| v.parse::<u8>().ok())
                .ok_or_else(|| format!("row {i}: field {j} in {line:?}"))
        };
        if f.len() != 4 {
            return Err(format!("row {i}: {} fields", f.len()));
        }
        let (d, dir64, cof, dcc) = (n(0)?, n(1)?, n(2)?, n(3)?);
        if dir64 >= 64 || !seen.insert((d, dir64)) {
            return Err(format!("row {i}: ({d}, {dir64}) out of range or repeated"));
        }
        let want = (cof_direction(d, dir64), file_direction(d, dir64));
        if want != (Ok(cof), Ok(dcc)) {
            return Err(format!(
                "row {i}: ({d}, {dir64}) = ({cof}, {dcc}), code {want:?}"
            ));
        }
        rows += 1;
    }
    let all = [1u8, 2, 4, 8, 16, 32, 64]
        .iter()
        .all(|&d| (0..64).all(|x| seen.contains(&(d, x))));
    if rows != 448 || !all {
        return Err(format!("{rows} rows, all pairs present: {all}"));
    }
    Ok(rows)
}

// Covers: specs/render/unit-composite.md §3 r4
#[test]
fn directions_tsv_matches_code_row_by_row() {
    assert_eq!(check_directions(TSV), Ok(448));
}

/// METHODS M08: the row-by-row check fails on any changed value, a
/// dropped row, a repeated row and a changed header.
#[test]
fn directions_check_catches_perturbations() {
    let lines: Vec<&str> = TSV.lines().collect();
    let join = |v: &[String]| v.join("\n");
    let owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    // Every row, every value column: +1 mod 64 must be caught.
    for row in 1..owned.len() {
        for col in 2..4 {
            let mut v = owned.clone();
            let mut f: Vec<String> = v[row].split('\t').map(str::to_string).collect();
            let x: u8 = f[col].parse().unwrap();
            f[col] = ((x + 1) % 64).to_string();
            v[row] = f.join("\t");
            assert!(check_directions(&join(&v)).is_err(), "row {row} col {col}");
        }
    }
    let mut v = owned.clone();
    v.remove(100);
    assert!(check_directions(&join(&v)).is_err());
    let mut v = owned.clone();
    v[100] = v[101].clone();
    assert!(check_directions(&join(&v)).is_err());
    let mut v = owned.clone();
    v[0] = "directions\tdir64\tdcc_dir\tcof_dir".into();
    assert!(check_directions(&join(&v)).is_err());
}

// Covers: specs/render/unit-composite.md §3 r4
#[test]
fn cof_direction_vectors() {
    let d16: Vec<u8> = [0, 2, 6, 62]
        .iter()
        .map(|&x| cof_direction(16, x).unwrap())
        .collect();
    assert_eq!(d16, [0, 1, 2, 0]);
    let d8: Vec<u8> = [3, 4, 59, 60]
        .iter()
        .map(|&x| cof_direction(8, x).unwrap())
        .collect();
    assert_eq!(d8, [0, 1, 7, 0]);
    // D must be a power of two below 128 (fatal 0x1ED / 0x1EE).
    for d in [0u8, 3, 12, 128] {
        assert_eq!(
            cof_direction(d, 0),
            Err(UnitCompositeError::DirectionCount(d))
        );
    }
}

// Covers: specs/render/unit-composite.md §6 r3
#[test]
fn file_direction_and_cel_vectors() {
    let d: Vec<u8> = [0, 4, 12, 60]
        .iter()
        .map(|&x| file_direction(8, x).unwrap())
        .collect();
    assert_eq!(d, [4, 0, 5, 4]);
    // Df = 16, dir64 2, Ff = 8, frame 3: cel 8 × 8 + 3 = 67 = direction
    // set 8, frame 3.
    let path = CanonicalPath::new("data/x.dcc").unwrap();
    let cel = component_cel(&path, 16, 8, 2, 3).unwrap();
    assert_eq!(cel.set.part(), FramePart::Dir(8));
    assert_eq!(cel.index, 3);
    // A 2-direction file shows only direction 0.
    assert!((0..64).all(|x| file_direction(2, x) == Ok(0)));
    // frame = Ff passes and is the next direction's first cel.
    let next = component_cel(&path, 8, 8, 4, 8).unwrap();
    assert_eq!((next.set.part(), next.index), (FramePart::Dir(1), 0));
    // frame > Ff is fatal; the last direction's frame Ff is past the file.
    assert!(component_cel(&path, 8, 8, 4, 9).is_err());
    assert_eq!(file_direction(8, 48), Ok(7));
    assert!(component_cel(&path, 8, 8, 48, 8).is_err());
}

// Covers: specs/render/unit-composite.md §3 r3, §3 r4
#[test]
fn remote_and_local_player_direction_vectors() {
    // Remote player (n = 8), 16-direction COF, dir64 6: snapped 0.
    let n = expected_directions(DirectionSource::Player { sixteen: false }, 1);
    assert_eq!(n, 8);
    let r = unit_direction(16, n, 6, false).unwrap();
    assert_eq!((r.dir64, r.cof_dir), (0, 0));
    assert_eq!(file_direction(16, r.dir64), Ok(4));
    // Local player (n = 16): no snap.
    let n = expected_directions(DirectionSource::Player { sixteen: true }, 1);
    let l = unit_direction(16, n, 6, false).unwrap();
    assert_eq!((l.dir64, l.cof_dir), (6, 2));
    assert_eq!(file_direction(16, l.dir64), Ok(0));
}

// Covers: specs/render/unit-composite.md §3 r3
#[test]
fn expected_direction_counts() {
    let m = |d_mode, mode_table_zero| DirectionSource::Monster {
        d_mode,
        mode_table_zero,
    };
    assert_eq!(expected_directions(m(8, true), 1), 4);
    assert_eq!(expected_directions(m(8, false), 1), 8);
    assert_eq!(expected_directions(m(16, true), 1), 16);
    assert_eq!(
        expected_directions(DirectionSource::Missile { num_directions: 32 }, 0),
        32
    );
    assert_eq!(expected_directions(DirectionSource::Other, 0), 1);
    assert_eq!(
        expected_directions(DirectionSource::Player { sixteen: true }, -1),
        1
    );
}

// Covers: specs/render/unit-composite.md §3 r4, §3 r5
#[test]
fn snap_tables_and_write_back() {
    // (8, 4): table 1, 1, 3, 3, 5, 5, 7, 7 on dir64 >> 3.
    let s: Vec<u8> = (0..8).map(|k| snap(8, 4, k * 8)).collect();
    assert_eq!(s, [8, 8, 24, 24, 40, 40, 56, 56]);
    // (16, 8): 0, 0, 2, 2, …, 14, 14 on dir64 >> 2.
    assert_eq!(snap(16, 8, 7), 0);
    assert_eq!(snap(16, 8, 8), 8);
    assert_eq!(snap(16, 8, 63), 56);
    // Other pairs do not snap.
    assert_eq!(snap(16, 16, 7), 7);
    assert_eq!(snap(8, 8, 7), 7);
    // Write-back: dead and n ≠ D, whether or not a snap happened.
    assert_eq!(unit_direction(16, 8, 6, true).unwrap().write_back, Some(0));
    assert_eq!(unit_direction(16, 4, 6, true).unwrap().write_back, Some(6));
    assert_eq!(unit_direction(16, 16, 6, true).unwrap().write_back, None);
    assert_eq!(unit_direction(16, 8, 6, false).unwrap().write_back, None);
    // dir64 ≥ 64 is direction 0 with n = 1.
    let w = unit_direction(16, 16, 64, true).unwrap();
    assert_eq!((w.dir64, w.cof_dir, w.write_back), (0, 0, Some(0)));
}

// Covers: specs/render/unit-composite.md §2 text, §2 r1
#[test]
fn cof_name_vector() {
    let n = CofName::player(code(b"AM"), 1, code(b"NU"), code(b"1hs"));
    assert_eq!(n.full(), "DATA\\GLOBAL\\CHARS\\AM\\COF\\AMNU1hs.COF");
    assert_eq!(n.short(), "AMNU1hs");
    assert_eq!(
        n.path().unwrap().as_str(),
        "data/global/chars/am/cof/amnu1hs.cof"
    );
    let m = CofName {
        kind: CompositeKind::Monster,
        token: code(b"ZM"),
        mode_token: code(b"A1"),
        weapon_class: HTH,
    };
    assert_eq!(m.full(), "DATA\\GLOBAL\\MONSTERS\\ZM\\COF\\ZMA1hth.COF");
    let o = CofName {
        kind: CompositeKind::Object,
        token: code(b"TO"),
        mode_token: code(b"ON"),
        weapon_class: HTH,
    };
    assert_eq!(o.full(), "DATA\\GLOBAL\\OBJECTS\\TO\\COF\\TOONhth.COF");
    // A part ends at its first space or 0.
    assert_eq!(part(b"ab c"), b"ab");
    assert_eq!(part(b"a\0bc"), b"a");
    assert_eq!(part(b"abcd"), b"abc");
}

// Covers: specs/render/unit-composite.md §2 r3
#[test]
fn throw_mode_weapon_class() {
    let th =
        |w: &[u8]| CofName::player(code(b"AM"), PLAYER_MODE_TH, code(b"TH"), code(w)).weapon_class;
    assert_eq!(th(b"2hs"), HTH);
    assert_eq!(th(b"bow"), HTH);
    for w in [b"1hs", b"1ht", b"1js", b"1jt", b"1ss", b"1st"] {
        assert_eq!(th(w), code(w));
    }
    // Other modes keep W.
    assert_eq!(
        CofName::player(code(b"AM"), 1, code(b"NU"), code(b"2hs")).weapon_class,
        code(b"2hs")
    );
}

// Covers: specs/render/unit-composite.md §2 r2
#[test]
fn mode_override_last_match_wins() {
    let pairs = [(code(b"GH"), 5), (code(b"XX"), 6), (code(b"G2"), 5)];
    assert_eq!(mode_token(code(b"S1"), 5, &pairs), code(b"G2"));
    assert_eq!(mode_token(code(b"S1"), 7, &pairs), code(b"S1"));
}

fn hand(component: u8, item_type: u16, w: &[u8], w2: &[u8], grip: bool) -> HandItem {
    HandItem {
        component,
        item_type,
        wclass: code(w),
        two_handed_wclass: code(w2),
        two_handed_grip: grip,
    }
}

// Covers: specs/render/unit-composite.md §2.1
#[test]
fn weapon_class_rules() {
    // Monsters.
    assert_eq!(monster_weapon_class(0, false, Some(code(b"1hs"))), HTH);
    assert_eq!(
        monster_weapon_class(12, true, Some(code(b"1hs"))),
        code(b"1hs")
    );
    assert_eq!(monster_weapon_class(1, false, None), HTH);
    // Players.
    let base = PlayerHands {
        class: 0,
        base_wclass: HTH,
        loc4: None,
        loc5: None,
    };
    assert_eq!(player_weapon_class(&base), Ok(HTH));
    // Location 4 with component other than 5/6 → location 5.
    let p = PlayerHands {
        loc4: Some(hand(7, 2, b"xxx", b"xxx", false)),
        loc5: Some(hand(5, 3, b"1hs", b"2hs", false)),
        ..base
    };
    assert_eq!(player_weapon_class(&p), Ok(code(b"1hs")));
    // Grip test 2 → 2handedwclass.
    let p = PlayerHands {
        loc4: Some(hand(5, 3, b"1hs", b"2hs", true)),
        ..base
    };
    assert_eq!(player_weapon_class(&p), Ok(code(b"2hs")));
    // Two type-45 items: Assassin ht2; Barbarian is open question 3.
    let two = PlayerHands {
        loc4: Some(hand(5, 45, b"ht1", b"ht1", false)),
        loc5: Some(hand(6, 45, b"ht1", b"ht1", false)),
        ..base
    };
    assert_eq!(
        player_weapon_class(&PlayerHands { class: 6, ..two }),
        Ok(HT2)
    );
    assert!(player_weapon_class(&PlayerHands { class: 4, ..two }).is_err());
    assert_eq!(
        player_weapon_class(&PlayerHands { class: 1, ..two }),
        Ok(code(b"ht1"))
    );
    // COF name in DT/DD: hth.
    let p = PlayerHands {
        loc4: Some(hand(5, 3, b"1hs", b"2hs", false)),
        ..base
    };
    assert_eq!(player_cof_weapon_class(17, &p), Ok(HTH));
    assert_eq!(player_cof_weapon_class(1, &p), Ok(code(b"1hs")));
}

fn player() -> PlayerLook {
    PlayerLook {
        body_armor: None,
        item_gfx: [None; 16],
        holy_shield: false,
        shield_hand_item: false,
        linked_inventory: false,
    }
}

// Covers: specs/render/unit-composite.md §5.1 r3, §5.1 r4, §6 r1
#[test]
fn player_component_file_vector() {
    // Amazon HD with a `cap`, mode NU, `1hs`.
    let mut p = player();
    p.item_gfx[usize::from(component::HD)] = Some(code(b"cap"));
    let armor = armor_class(
        ArmorSource::Player(Some(&p)),
        component::HD,
        1,
        code(b"1hs"),
    )
    .unwrap();
    assert_eq!(armor, Some(code(b"cap")));
    let codes = ComponentCodes::new(
        CompositeKind::Player,
        code(b"AM"),
        code(b"HD"),
        armor,
        code(b"NU"),
        code(b"1hs"),
    )
    .unwrap();
    assert_eq!(codes.name(), "AMHDcapNU1hs");
    assert_eq!(
        codes.file(FileFormat::Dcc),
        "DATA\\GLOBAL\\CHARS\\AM\\HD\\AMHDcapNU1hs.dcc"
    );
    assert_eq!(
        codes.path(FileFormat::Dcc).unwrap().as_str(),
        "data/global/chars/am/hd/amhdcapnu1hs.dcc"
    );
}

// Covers: specs/render/unit-composite.md §5.1 r3
#[test]
fn player_armor_classes() {
    let mut p = player();
    p.body_armor = Some([2, 1, 0, 1, 2, 0]);
    let ac = |p: &PlayerLook, c, mode, w: &[u8]| {
        armor_class(ArmorSource::Player(Some(p)), c, mode, code(w)).unwrap()
    };
    // torso = 2 → hvy; legs 1 → med; rArm 0 → lit.
    assert_eq!(ac(&p, component::TR, 1, b"hth"), Some(HVY));
    assert_eq!(ac(&p, component::LG, 1, b"hth"), Some(MED));
    assert_eq!(ac(&p, component::RA, 1, b"hth"), Some(LIT));
    assert_eq!(ac(&p, component::S1, 1, b"hth"), Some(HVY));
    // DT/DD: lit.
    assert_eq!(ac(&p, component::TR, 0, b"hth"), Some(LIT));
    // No body armor: index 0.
    assert_eq!(ac(&player(), component::TR, 1, b"hth"), Some(LIT));
    // Index above 2: the request fails.
    p.body_armor = Some([3, 0, 0, 0, 0, 0]);
    assert_eq!(ac(&p, component::TR, 1, b"hth"), None);
    // No inventory: fails.
    assert_eq!(
        armor_class(ArmorSource::Player(None), component::HD, 1, HTH),
        Ok(None)
    );
    // Weapons: xbw shows the primary weapon on RH and LH; bow RH lit;
    // holy shield; no item → lit.
    let mut p = player();
    p.item_gfx[usize::from(component::RH)] = Some(code(b"lxb"));
    p.item_gfx[usize::from(component::LH)] = Some(code(b"bol"));
    assert_eq!(ac(&p, component::LH, 1, b"xbw"), Some(code(b"lxb")));
    assert_eq!(ac(&p, component::RH, 1, b"bow"), Some(LIT));
    assert_eq!(ac(&p, component::LH, 1, b"bow"), Some(code(b"bol")));
    assert_eq!(ac(&p, component::SH, 1, b"1hs"), Some(LIT));
    p.holy_shield = true;
    p.shield_hand_item = true;
    assert_eq!(ac(&p, component::SH, 1, b"1hs"), Some(HSH));
}

// Covers: specs/render/unit-composite.md §5.1 r1, §5.1 r2
#[test]
fn monster_and_object_armor_classes() {
    assert_eq!(armor_class(ArmorSource::Object, 1, 0, HTH), Ok(Some(LIT)));
    let mut m = MonsterLook {
        choices: [0; 16],
        counts: [0; 16],
        codes: [[EMPTY; 12]; 16],
        composite_death: false,
        override_level: false,
    };
    m.counts[1] = 2;
    m.choices[1] = 1;
    m.codes[1][1] = code(b"med");
    assert_eq!(
        armor_class(ArmorSource::Monster(&m), 1, 1, HTH),
        Ok(Some(MED))
    );
    // Choice ≥ count: lit.
    m.choices[1] = 2;
    assert_eq!(
        armor_class(ArmorSource::Monster(&m), 1, 1, HTH),
        Ok(Some(LIT))
    );
    // DT without compositeDeath: not looked up.
    m.choices[1] = 1;
    assert_eq!(
        armor_class(ArmorSource::Monster(&m), 1, 0, HTH),
        Ok(Some(LIT))
    );
    m.composite_death = true;
    assert_eq!(
        armor_class(ArmorSource::Monster(&m), 1, 0, HTH),
        Ok(Some(MED))
    );
    // Override tables: open question 5.
    m.override_level = true;
    assert!(armor_class(ArmorSource::Monster(&m), 1, 1, HTH).is_err());
}

// Covers: specs/render/unit-composite.md §5.1 text
#[test]
fn request_fails_on_empty_codes() {
    let ok = |u: &[u8], c: &[u8], a: Option<Code>, m: &[u8], w: &[u8]| {
        ComponentCodes::new(
            CompositeKind::Monster,
            code(u),
            code(c),
            a,
            code(m),
            code(w),
        )
    };
    assert!(ok(b"ZM", b"TR", Some(LIT), b"NU", b"hth").is_some());
    assert!(ok(b"", b"TR", Some(LIT), b"NU", b"hth").is_none());
    assert!(ok(b"ZM", b"", Some(LIT), b"NU", b"hth").is_none());
    assert!(ok(b"ZM", b"TR", Some(LIT), b"NU", b"").is_none());
    assert!(ok(b"ZM", b"TR", None, b"NU", b"hth").is_none());
    assert!(ok(b"ZM", b"TR", Some(EMPTY), b"NU", b"hth").is_none());
    // Empty mode token → xxx.
    let c = ok(b"ZM", b"TR", Some(LIT), b"", b"hth").unwrap();
    assert_eq!(c.name(), "ZMTRlitxxxhth");
}

// Covers: specs/render/unit-composite.md §6 r2
#[test]
fn dc6_component_files() {
    let codes = |kind| {
        ComponentCodes::new(kind, code(b"MP"), code(b"TR"), Some(LIT), code(b"NU"), HTH).unwrap()
    };
    let mon = codes(CompositeKind::Monster);
    for mode in [0, 1, 12] {
        assert_eq!(file_format(&mon, 242, mode), FileFormat::Dc6);
    }
    assert_eq!(file_format(&mon, 243, 0), FileFormat::Dc6);
    assert_eq!(file_format(&mon, 243, 1), FileFormat::Dcc);
    assert_eq!(file_format(&mon, 1, 0), FileFormat::Dcc);
    let obj = codes(CompositeKind::Object);
    assert_eq!(file_format(&obj, 342, 0), FileFormat::Dc6);
    assert_eq!(file_format(&obj, 242, 0), FileFormat::Dcc);
    let oy = ComponentCodes::new(
        CompositeKind::Object,
        code(b"OY"),
        code(b"TR"),
        Some(LIT),
        code(b"TN"),
        HTH,
    )
    .unwrap();
    assert_eq!(file_format(&oy, 1, 0), FileFormat::Dc6);
    assert!(mon.file(FileFormat::Dc6).ends_with("MPTRlitNUhth.dc6"));
}

// Covers: specs/render/unit-composite.md §6 r4
#[test]
fn slot_frame_none_for_failed_request_or_missing_file() {
    let codes = ComponentCodes::new(
        CompositeKind::Monster,
        code(b"ZM"),
        code(b"TR"),
        Some(LIT),
        code(b"NU"),
        HTH,
    )
    .unwrap();
    assert_eq!(
        slot_frame(None, FileFormat::Dcc, Some((8, 8)), 0, 0),
        Ok(None)
    );
    assert_eq!(
        slot_frame(Some(&codes), FileFormat::Dcc, None, 0, 0),
        Ok(None)
    );
    let f = slot_frame(Some(&codes), FileFormat::Dcc, Some((8, 8)), 4, 2)
        .unwrap()
        .unwrap();
    assert_eq!(f.set.path(), "data/global/monsters/zm/tr/zmtrlitnuhth.dcc");
    assert_eq!((f.set.part(), f.index), (FramePart::Dir(0), 2));
}

fn cof_box(x_min: i32, x_max: i32, y_min: i32, y_max: i32) -> Cof {
    Cof {
        layers_count: 0,
        frames: 1,
        directions: 16,
        version: 20,
        unknown: [0; 4],
        x_min,
        x_max,
        y_min,
        y_max,
        animation_rate: 256,
        layers: vec![],
        events: vec![0],
        event_padding: vec![],
        draw_order: vec![],
    }
}

// Covers: specs/render/unit-composite.md §4
#[test]
fn cof_box_culling_vector() {
    let cof = cof_box(-30, 30, -90, 0);
    assert!(!cof_box_visible(&cof, -31, 300, 800, 600));
    assert!(cof_box_visible(&cof, -30, 300, 800, 600));
    assert!(!cof_box_visible(&cof, 400, 690, 800, 600));
    assert!(cof_box_visible(&cof, 400, 688, 800, 600));
    // Right and top edges.
    assert!(!cof_box_visible(&cof, 829, 300, 800, 600));
    assert!(cof_box_visible(&cof, 828, 300, 800, 600));
    assert!(!cof_box_visible(&cof, 400, -1, 800, 600));
    assert!(cof_box_visible(&cof, 400, 0, 800, 600));
}

#[test]
fn unit_pose_uses_cof_direction_and_culling() {
    let name = CofName::player(code(b"AM"), 1, code(b"NU"), code(b"1hs"));
    let cof = cof_box(-30, 30, -90, 0);
    let pose = |kind, cof: Option<&Cof>, at| {
        unit_pose(kind, &name, cof, at, (800, 600), 8, 6, false, 3).unwrap()
    };
    let (p, d) = pose(Some(CompositeKind::Player), Some(&cof), (400, 300)).unwrap();
    assert_eq!(p.cof.as_str(), "data/global/chars/am/cof/amnu1hs.cof");
    assert_eq!((p.dir, p.frame), (0, 3));
    assert_eq!(d.dir64, 0);
    assert_eq!(
        pose(Some(CompositeKind::Player), Some(&cof), (-31, 300)),
        None
    );
    assert_eq!(pose(Some(CompositeKind::Player), None, (400, 300)), None);
    assert_eq!(pose(None, Some(&cof), (400, 300)), None);
}

// Covers: specs/render/unit-composite.md §7 r1, §7 r2, §7 text
#[test]
fn colormap_sources() {
    let mut items = [None; 16];
    fn inputs(kind: CompositeKind, items: &[Option<ItemLook>; 16]) -> ColormapInputs<'_> {
        ColormapInputs {
            kind,
            unit: UnitMap::None,
            palette_index: 0,
            item_maps: true,
            items,
            local_blood: true,
        }
    }
    assert_eq!(unit_map(5, CompositeKind::Player), UnitMap::ShiftRow(5));
    assert_eq!(
        unit_map(0, CompositeKind::Monster),
        UnitMap::MonsterPalShift
    );
    assert_eq!(unit_map(0, CompositeKind::Object), UnitMap::None);
    // S8: blood for monsters with local blood; S7: none.
    let m = ColormapInputs {
        unit: UnitMap::MonsterPalShift,
        ..inputs(CompositeKind::Monster, &items)
    };
    assert_eq!(colormap_source(component::S8, &m), ColormapSource::Blood);
    assert_eq!(colormap_source(component::S7, &m), ColormapSource::None);
    assert_eq!(
        colormap_source(component::TR, &m),
        ColormapSource::Unit(UnitMap::MonsterPalShift)
    );
    // Players: the item map; LG without an item uses TR's item.
    items[usize::from(component::TR)] = Some(ItemLook {
        flags: 0,
        gfx: code(b"hvy"),
    });
    let p = inputs(CompositeKind::Player, &items);
    assert_eq!(colormap_source(component::S8, &p), ColormapSource::None);
    assert_eq!(
        colormap_source(component::LG, &p),
        ColormapSource::Item {
            component: component::TR,
            fallback: UnitMap::None
        }
    );
    // HD without an item: U.
    assert_eq!(
        colormap_source(component::HD, &p),
        ColormapSource::Unit(UnitMap::None)
    );
    // Palette index 0x6C or item maps off: U.
    let q = ColormapInputs {
        palette_index: 0x6C,
        unit: UnitMap::ShiftRow(0x6C),
        ..p
    };
    assert_eq!(
        colormap_source(component::TR, &q),
        ColormapSource::Unit(UnitMap::ShiftRow(0x6C))
    );
    let q = ColormapInputs {
        item_maps: false,
        ..p
    };
    assert_eq!(
        colormap_source(component::TR, &q),
        ColormapSource::Unit(UnitMap::None)
    );
    // Flags 0x100 / 0x4000: U; gfx `lit`: no map at all.
    for flags in [0x100, 0x4000] {
        items[usize::from(component::TR)] = Some(ItemLook {
            flags,
            gfx: code(b"hvy"),
        });
        let p = inputs(CompositeKind::Player, &items);
        assert_eq!(
            colormap_source(component::TR, &p),
            ColormapSource::Unit(UnitMap::None)
        );
    }
    items[usize::from(component::TR)] = Some(ItemLook { flags: 0, gfx: LIT });
    let p = inputs(CompositeKind::Player, &items);
    assert_eq!(colormap_source(component::TR, &p), ColormapSource::None);
}

// Covers: specs/render/unit-composite.md §8 r5, §8 r6
#[test]
fn motion_record_reaches_limits_vector() {
    let mut r = MotionRecord {
        limit: [10, 10, 10],
        ..MotionRecord::default()
    };
    r.update().unwrap();
    assert_eq!(r.flags & motion::DONE, motion::DONE);
    assert_eq!(r.pos, [10 << 11, 10 << 11, 10 << 11]);
    assert_eq!(r.offset, [0, 5, -10]);
    assert_eq!(r.draw_offset(), (0, -5));
    assert_eq!(unit_offset(Some(&r), TableOffset::None), Some((0, -5)));
}

#[test]
fn done_motion_record_is_not_updated() {
    let mut r = MotionRecord {
        flags: motion::DONE,
        pos: [32 << 11, 0, 0],
        offset: [7, 8, 9],
        ..MotionRecord::default()
    };
    let before = r;
    r.update().unwrap();
    assert_eq!(r, before);
    assert_eq!(r.draw_offset(), (7, 17));
}

// Covers: specs/render/unit-composite.md §8 r1, §8 r2
#[test]
fn motion_record_integration_order_and_timer() {
    let mut r = MotionRecord {
        flags: motion::TIMED | motion::FROM_BELOW,
        vel: [1 << 11, 2 << 11, 4 << 11],
        acc: [1 << 11, 0, -(1 << 11)],
        limit: [100, 100, 100],
        ticks_left: 1,
        ..MotionRecord::default()
    };
    r.update().unwrap();
    // Position moved by the old velocity, velocity by the acceleration.
    assert_eq!(r.pos, [1 << 11, 2 << 11, 4 << 11]);
    assert_eq!(r.vel, [2 << 11, 2 << 11, 3 << 11]);
    assert_eq!(r.ticks_left, 0);
    assert_eq!(r.flags & motion::DONE, 0);
    // a = 1, b = 2: ox = (1 − 2) >> 1 = −1 (arithmetic), oy = 3 >> 2 = 0.
    assert_eq!(r.offset, [-1, 0, -4]);
    r.update().unwrap();
    // Ticks 0: done, x and y zeroed.
    assert_eq!(r.flags & motion::DONE, motion::DONE);
    assert_eq!((r.pos[0], r.pos[1]), (0, 0));
    assert_eq!(r.pos[2], 7 << 11);
}

// Covers: specs/render/unit-composite.md §8 r3
#[test]
fn motion_record_bounce() {
    let mut r = MotionRecord {
        flags: motion::BOUNCE,
        pos: [0, 0, 1 << 11],
        vel: [0, 0, -(3 << 11)],
        limit: [0, 0, 0],
        bounces_left: 1,
        bounce_factor: 50,
        ..MotionRecord::default()
    };
    r.update().unwrap();
    // z → −2 << 11 ≤ 0: vz = −trunc(50 × vz / 100), z = limit (unshifted).
    assert_eq!(r.vel[2], 3 << 10);
    assert_eq!(r.pos[2], 0);
    assert_eq!(r.bounces_left, 0);
    assert_eq!(r.flags & motion::DONE, 0);
    r.vel[2] = -1;
    r.update().unwrap();
    assert_eq!(r.flags & motion::DONE, motion::DONE);
    // −trunc(50 × −1 / 100) = 0 (truncation toward zero).
    assert_eq!(r.vel[2], 0);
    // Follow branch: open question 7.
    let mut f = MotionRecord {
        flags: motion::FOLLOW,
        ..MotionRecord::default()
    };
    assert!(f.update().is_err());
}

#[test]
fn table_offsets_add_to_the_record() {
    let r = MotionRecord {
        offset: [1, 2, 3],
        ..MotionRecord::default()
    };
    assert_eq!(unit_offset(None, TableOffset::None), Some((0, 0)));
    assert_eq!(
        unit_offset(
            Some(&r),
            TableOffset::Object {
                x: 10,
                y: 20,
                draw: true
            }
        ),
        Some((11, 25))
    );
    assert_eq!(
        unit_offset(
            Some(&r),
            TableOffset::Object {
                x: 10,
                y: 20,
                draw: false
            }
        ),
        None
    );
    assert_eq!(
        unit_offset(Some(&r), TableOffset::Missile(Some((-4, 5, 6)))),
        Some((-3, 16))
    );
    assert_eq!(unit_offset(None, TableOffset::Missile(None)), None);
}

#[test]
fn single_cel_units() {
    assert_eq!(gold_direction(99), 0);
    assert_eq!(gold_direction(100), 1);
    assert_eq!(gold_direction(499), 1);
    assert_eq!(gold_direction(500), 2);
    assert_eq!(gold_direction(4999), 2);
    assert_eq!(gold_direction(5000), 3);
    assert_eq!(item_graphic(3), Some(ItemGraphic::Ground));
    assert_eq!(item_graphic(5), Some(ItemGraphic::Ground));
    assert_eq!(item_graphic(6), Some(ItemGraphic::Inventory));
    assert_eq!(item_graphic(7), None);
    assert_eq!(
        flippy_file("flpcap", 7, Some("flpuni"), None),
        "DATA\\GLOBAL\\items\\flpuni.dc6"
    );
    assert_eq!(
        flippy_file("flpcap", 7, Some(""), None),
        "DATA\\GLOBAL\\items\\flpcap.dc6"
    );
    assert_eq!(
        flippy_file("flpcap", 5, Some("u"), Some("flpset")),
        "DATA\\GLOBAL\\items\\flpset.dc6"
    );
    assert_eq!(
        flippy_file("flpcap", 4, Some("u"), Some("s")),
        "DATA\\GLOBAL\\items\\flpcap.dc6"
    );
    assert_eq!(missile_file("Arrow"), "DATA\\GLOBAL\\MISSILES\\Arrow.dcc");
}
