// Spec: specs/data/calc-expressions.md (test vectors)

use super::*;

/// §5 code tables.
const SKILLCALC: [&str; 73] = [
    "ln12", "dm12", "ln34", "dm34", "ln56", "dm56", "ln78", "dm78", "par1", "par2", "par3", "par4",
    "par5", "par6", "par7", "par8", "lvl", "edmn", "edmx", "edln", "toht", "mana", "mps", "math",
    "madm", "macr", "m1en", "m1ex", "m1el", "m2en", "m2ex", "m2el", "m3en", "m3ex", "m3el", "m1rn",
    "m2rn", "m3rn", "edns", "edxs", "ulvl", "blvl", "usmc", "m1eo", "m1ey", "m2eo", "m2ey", "me3o",
    "me3y", "enma", "exma", "edma", "enms", "exms", "len", "clc1", "clc2", "clc3", "clc4", "rng",
    "ast1", "ast2", "ast3", "ast4", "ast5", "ast6", "pst1", "pst2", "pst3", "pst4", "pst5", "pets",
    "skpt",
];
const MISSCALC: [&str; 43] = [
    "par1", "par2", "par3", "par4", "par5", "cpa1", "cpa2", "cpa3", "cpa4", "cpa5", "hpa1", "hpa2",
    "hpa3", "chp1", "chp2", "chp3", "dpa1", "dpa2", "lvl", "edmn", "edmx", "edln", "edns", "edxs",
    "damn", "damx", "dmns", "dmxs", "rang", "sl12", "sd12", "sl34", "sd34", "cl12", "cd12", "cl34",
    "cd34", "shl1", "shd1", "chl1", "chd1", "dl12", "dd12",
];

/// The 1.14d links of the synthetic vectors: `Fire Bolt` = skill 36,
/// `firebolt` = missile 58, `strength` = stat 0, `Holy Fire` = skill 102.
struct Links;

impl CalcLinks for Links {
    fn skill(&self, key: &[u8]) -> Option<u32> {
        match key {
            b"fire bolt" => Some(36),
            b"holy fire" => Some(102),
            _ => None,
        }
    }
    fn missile(&self, key: &[u8]) -> Option<u32> {
        (key == b"firebolt").then_some(58)
    }
    fn stat(&self, key: &[u8]) -> Option<u32> {
        (key == b"strength").then_some(0)
    }
    fn skillcalc(&self, code: u32) -> Option<u32> {
        SKILLCALC
            .iter()
            .position(|c| code_of(c.as_bytes()) == code)
            .map(|i| i as u32)
    }
    fn misscalc(&self, code: u32) -> Option<u32> {
        MISSCALC
            .iter()
            .position(|c| code_of(c.as_bytes()) == code)
            .map(|i| i as u32)
    }
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

fn check(family: Family, text: &str, expected: &str) {
    let got = compile(family, &Links, text.as_bytes()).unwrap().code;
    let want = if expected == "fail" {
        Vec::new()
    } else {
        hex(expected)
    };
    assert_eq!(got, want, "{family:?} `{text}`");
}

#[test]
fn skills_synthetic() {
    let s = |t, e| check(Family::Skills, t, e);
    for t in ["", "   ", "\"\"", "()", ",", "min("] {
        s(t, "fail");
    }
    s("7", "07 07 00");
    s("-7", "07 F9 00");
    s("- 5", "07 FB 00");
    s("300", "08 2C 01 00");
    s("-300", "08 D4 FE 00");
    s("100000", "09 A0 86 01 00 00");
    s("2147483648", "09 00 00 00 80 00");
    s("4294967297", "07 01 00");
    s("2+3*4", "07 0E 00");
    s("(2+3)*4", "07 14 00");
    s("10-4-3", "07 03 00");
    s("2^3^2", "07 40 00");
    s("-2^2", "07 04 00");
    s("2^-1", "07 01 00");
    s("0^0", "07 01 00");
    s("2*-3", "07 FA 00");
    s("7/2", "07 03 00");
    s("-7/2", "07 FD 00");
    s("7/-2", "07 FD 00");
    s("5/0", "07 00 00");
    s("1<2", "07 01 00");
    s("2>=2", "07 01 00");
    s("2<=1", "07 00 00");
    s("3==3", "07 01 00");
    s("3!=3", "07 00 00");
    s("3>2>1", "07 00 00");
    s("1?2:3", "07 02 00");
    s("0?2:3", "07 03 00");
    s("1 < 2 ? 5 : 6", "07 01 00");
    s("(1<2)?5:6", "07 05 00");
    s("1?2:3+10", "07 0C 00");
    s("-1?2:3", "07 FE 00");
    s("(1<2)?(3+4):5*2", "07 0E 00");
    s("1?2 3", "07 02 00");
    s("0?2 3", "07 03 00");
    s("(lvl<4)?lvl 3", "04 10 07 04 0A 04 10 07 03 16 00");
    for t in ["1?2+1:3", "1?-2:3", "1?2", "--5", "+5", "5*", "lvl+", "1)"] {
        s(t, "fail");
    }
    s("1=1", "07 01 00");
    s("1.5", "07 01 00");
    s("5 $ 3", "07 05 00");
    s("5 3", "07 03 00");
    s("1:2", "07 02 00");
    s("1,2", "07 02 00");
    s("(", "07 00 00");
    s("ln12", "04 00 00");
    s("lvl", "04 10 00");
    s("'lvl'", "04 10 00");
    s("\"ln12\"", "04 00 00");
    s("LVL", "07 00 00");
    s("foo", "07 00 00");
    s("a.b", "07 00 00");
    s("par34", "04 0A 00");
    s("1+ln12", "07 01 04 00 10 00");
    s("ln12\"+1\"", "04 00 07 01 10 00");
    for t in ["min(3,5)", "MIN(3,5)", "min (3,5)"] {
        s(t, "07 03 07 05 01 00 00");
    }
    for t in [
        "min(-1,2)",
        "min((-1),2)",
        "skill(ln12)",
        "stat('strength')",
    ] {
        s(t, "fail");
    }
    s("min(0-1,2)", "07 00 07 01 11 07 02 01 00 00");
    s("max(2,0-1)", "07 02 07 00 07 01 11 01 01 00");
    s("min(1,2,3)", "07 01 07 02 07 03 01 00 00");
    s("min(1,2)min(3,4)", "07 01 07 02 01 00 07 03 07 04 01 00 00");
    s("max(1,2", "07 01 07 02 00");
    s("lvl(2)", "04 10 07 02 00");
    s("lvl*(2", "04 10 07 02 02 12 00");
    s("rand(1,6)", "07 01 07 06 01 02 00");
    s("skill('Fire Bolt'.lvl)", "07 24 07 10 01 03 00");
    s("skill('Fire Bolt'.blvl", "07 24 07 29 00");
    s("skill('fire bolt'.LVL)", "07 24 00");
    s("skill('No Such Skill'.lvl)", "07 00 07 10 01 03 00");
    s("stat('strength'.base)", "07 00 07 01 01 05 00");
    s("stat('strength'.mod)", "07 00 07 02 01 05 00");
    s("stat('strength'.accr)", "07 00 07 00 01 05 00");
    s("stat('strength'.base)+1", "07 00 07 01 01 05 07 01 10 00");
    s("stat('nosuchstat'.base)", "04 00 07 01 01 05 00");
    s("miss('firebolt'.edmn)", "07 3A 07 13 01 04 00");
    s("sklvl('Fire Bolt'.ln12.lvl)", "07 24 07 00 07 10 01 06 00");
    // Real 1.14d formulas (the links they need are in `Links`).
    s("\"-min(ln34,150)\"", "04 02 08 96 00 01 00 15 00");
    s("-par3 * lvl", "04 0A 15 04 10 12 00");
    s(
        "(lvl < 4) ?lvl:(2+lvl/3)",
        "04 10 07 04 0A 04 10 07 02 04 10 07 03 13 10 16 00",
    );
    s(
        "\"(lvl < 5) ? lvl : min(12,5+(lvl-5)/3)\"",
        "04 10 07 05 0A 04 10 07 0C 07 05 04 10 07 05 11 07 03 13 10 01 00 16 00",
    );
    s("sklvl('Holy Fire'.ln56.edmn)", "07 66 07 04 07 11 01 06 00");
    s(" ", "fail");
}

#[test]
fn other_families() {
    let m = |t, e| check(Family::Missiles, t, e);
    m("dl12", "04 29 00");
    m("lvl", "04 12 00");
    m("miss('firebolt'.dl12)", "07 3A 07 29 01 04 00");
    m("skill('Fire Bolt'.sl12)", "07 24 00");
    m("stat('strength'.base)", "07 00 00");
    m("sklvl('Fire Bolt'.ln12.lvl)", "07 00 00");
    let i = |t, e| check(Family::Items, t, e);
    i("5", "07 05 00");
    i("750", "08 EE 02 00");
    i("lvl", "04 00 00");
    i("lvl+1", "04 00 07 01 10 00");
    i("stat('strength'.base)", "07 00 07 01 01 03 00");
    i(
        "min(stat('strength'.base),10)",
        "07 00 07 01 01 03 07 0A 01 00 00",
    );
    i("skill(1,2)", "04 00 07 01 07 02 00");
    i("rand(1,3)", "07 01 07 03 01 02 00");
}

#[test]
fn refusals() {
    assert_eq!(
        compile(Family::Skills, &Links, b"lvl\xE9").unwrap_err(),
        CalcError::NonAscii
    );
    assert_eq!(
        compile(Family::Missiles, &Links, b"rand(1,3)").unwrap_err(),
        CalcError::MissileRand
    );
}

#[test]
fn diagnostics() {
    let d = |t: &str| {
        compile(Family::Skills, &Links, t.as_bytes())
            .unwrap()
            .diagnostics
    };
    assert_eq!(d("1?2"), [CalcDiag::Fail]);
    assert_eq!(d("5 $ 3"), [CalcDiag::Stop]);
    assert_eq!(d("foo"), [CalcDiag::UnknownName]);
    assert_eq!(d("lvl*(2"), [CalcDiag::OpenParen]);
    assert_eq!(d("max(1,2"), [CalcDiag::OpenFunction]);
    assert!(d("lvl+1").is_empty());
}

#[test]
fn constant_evaluator() {
    let e = |s: &str| eval_const(&hex(s));
    assert_eq!(e("07 05 07 00 13 00"), 0);
    assert_eq!(e("07 F9 07 02 13 00"), -3);
    assert_eq!(e("07 07 07 FE 13 00"), -3);
    assert_eq!(e("07 FE 07 03 14 00"), -8);
    assert_eq!(e("07 02 07 00 14 00"), 1);
    assert_eq!(e("07 02 07 FF 14 00"), 1);
    assert_eq!(e("09 00 00 00 80 15 00"), i32::MIN);
    assert_eq!(e("10 00"), 0);
    assert_eq!(e("07 05 07 06 16 00"), 6);
    assert_eq!(e("07 00 07 01 16 07 05 00"), 5);
    assert_eq!(e("07 FF 00"), -1);
    assert_eq!(e("07 05 03 07 06 00"), 5);
    assert_eq!(e("07 05 17 07 06 00"), 5);
    assert_eq!(e("07 05 FF 07 06 00"), 5);
    assert_eq!(e("07 05 02 07 06 10 00"), 5);
    assert_eq!(e("07"), 0);
    assert_eq!(e("07 05"), 0);
    let mut many = hex(&"07 01 ".repeat(64));
    many.extend(hex("07 02 00"));
    assert_eq!(eval_const(&many), 1);
}

#[test]
fn buffer_validation() {
    let v = |buf: &str, fields: &[u32]| {
        validate_buffer(Family::Skills, &hex(buf), fields.iter().copied())
    };
    assert!(matches!(
        v("07 05 00 07", &[]),
        Err(BufferError::Truncated { .. })
    ));
    assert!(matches!(
        v("08 05", &[]),
        Err(BufferError::Truncated { .. })
    ));
    assert!(matches!(
        v("07 05 03 00", &[]),
        Err(BufferError::BadOpcode { .. })
    ));
    let r = v("04 29 00 07 00 00", &[0, 3]).unwrap();
    assert_eq!(
        (r.starts.clone(), r.shared, r.unreferenced),
        (vec![0, 3], 0, 0)
    );
    assert!(matches!(
        v("04 29 00 07 00 00", &[0, 1]),
        Err(BufferError::BadField { value: 1 })
    ));
    assert_eq!(v("04 29 00 07 00 00", &[0, 3, 3]).unwrap().shared, 1);
    assert_eq!(v("04 29 00 07 00 00", &[u32::MAX]).unwrap().unreferenced, 2);
}
