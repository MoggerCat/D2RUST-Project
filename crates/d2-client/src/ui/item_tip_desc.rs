// Spec: specs/ui/inventory.md (§5 hover state), specs/data/runtime-maps.md (§3 description list)
//! The text shapes of the `itemstatcost` `descfunc` values 1–28 for the
//! item tool tip ([`super::item_tip`]).
//!
//! PROVISIONAL (REC-242, `docs/HANDOFF.md` §7): no spec gives the
//! description builder behind `0x004E60A0`, so the shapes are the
//! community-documented ones of `itemstatcost.txt` (English text, string
//! ids of the fixed phrases unknown); settled by the item hover capture
//! (`ui/text.md` capture `text-0002`). d2rs-own, unverified.

/// Names the shapes need beyond the stat's own strings.
pub trait DescNames {
    /// The display name of skill `id` (`skills` → `skilldesc` name).
    fn skill(&self, id: u32) -> Option<String>;
    /// The character class (0–6) a skill belongs to.
    fn skill_class(&self, id: u32) -> Option<u32>;
}

/// The input of one stat line.
pub struct Shape<'a> {
    /// `descfunc` (1–28).
    pub func: u8,
    /// `descval`: 0 no value, 2 after the text, else before it.
    pub val: u8,
    /// The stat value (without `Save Add`).
    pub value: i64,
    /// The stat's param (layer).
    pub param: u32,
    /// `descstrpos` or `descstrneg`, resolved.
    pub name: &'a str,
    /// `descstr2`, resolved.
    pub name2: Option<&'a str>,
}

const CLASSES: [&str; 7] = [
    "Amazon",
    "Sorceress",
    "Necromancer",
    "Paladin",
    "Barbarian",
    "Druid",
    "Assassin",
];

const TABS: [&str; 21] = [
    "Bow and Crossbow",
    "Passive and Magic",
    "Javelin and Spear",
    "Fire",
    "Lightning",
    "Cold",
    "Curses",
    "Poison and Bone",
    "Necromancer Summoning",
    "Paladin Combat",
    "Offensive Auras",
    "Defensive Auras",
    "Barbarian Combat",
    "Masteries",
    "Warcries",
    "Druid Summoning",
    "Shape Shifting",
    "Elemental",
    "Traps",
    "Shadow Disciplines",
    "Martial Arts",
];

fn signed(v: i64) -> String {
    if v < 0 {
        format!("-{}", -v)
    } else {
        format!("+{v}")
    }
}

fn plain(v: i64) -> String {
    v.to_string()
}

/// `v * 100 / 128` as a percent (descfunc 5 and 10).
fn scaled(v: i64) -> i64 {
    v * 100 / 128
}

/// The value and the text placed by `descval`.
fn place(val: u8, v: &str, name: &str) -> String {
    match val {
        0 => name.to_owned(),
        2 => format!("{name} {v}"),
        _ => format!("{v} {name}"),
    }
}

/// Replaces the first `%d`-style conversion of a printf format.
fn printf(fmt: &str, v: i64) -> String {
    for pat in ["%+d", "%d", "%i", "%u"] {
        if let Some(i) = fmt.find(pat) {
            let n = if pat == "%+d" { signed(v) } else { plain(v) };
            return format!("{}{}{}", &fmt[..i], n, &fmt[i + pat.len()..]);
        }
    }
    fmt.to_owned()
}

fn class_name(c: u32) -> &'static str {
    CLASSES.get(c as usize).copied().unwrap_or("")
}

/// The line of one stat.
pub fn render(s: &Shape, names: &dyn DescNames) -> String {
    let (v, n) = (s.value, s.name);
    let skill = |id: u32| names.skill(id).unwrap_or_else(|| format!("Skill {id}"));
    match s.func {
        2 => place(s.val, &format!("{}%", plain(v)), n),
        3 => place(s.val, &plain(v), n),
        9 => per_level(s, &plain(v)),
        4 => place(s.val, &format!("{}%", signed(v)), n),
        5 => place(s.val, &format!("{}%", plain(scaled(v))), n),
        6 => per_level(s, &signed(v)),
        7 => per_level(s, &format!("{}%", plain(v))),
        8 => per_level(s, &format!("{}%", signed(v))),
        10 => per_level(s, &format!("{}%", plain(scaled(v)))),
        11 => format!("Repairs 1 Durability In {} Seconds", 100 / v.max(1)),
        13 => format!("{} to {} Skill Levels", signed(v), class_name(s.param)),
        14 => {
            let tab = TABS.get((s.param & 7) as usize + 3 * (s.param >> 3) as usize);
            let class = class_name(s.param >> 3);
            format!(
                "{} to {} Skills ({class} Only)",
                signed(v),
                tab.copied().unwrap_or("")
            )
        }
        15 => format!(
            "{v}% Chance to cast level {} {} {n}",
            s.param >> 6,
            skill(s.param & 0x3F)
        ),
        16 => format!("Level {v} {} Aura When Equipped", skill(s.param)),
        17 => place(s.val, &plain(v), n),
        18 => place(s.val, &format!("{}%", plain(v)), n),
        19 => printf(n, v),
        20 => place(s.val, &format!("-{}%", plain(v.abs())), n),
        21 => place(s.val, &format!("-{}", plain(v.abs())), n),
        22 | 23 => place(s.val, &format!("{}%", plain(v)), n),
        24 => format!(
            "Level {} {} ({}/{} Charges)",
            s.param >> 6,
            skill(s.param & 0x3F),
            v & 0xFF,
            (v >> 8) & 0xFF
        ),
        27 => {
            let class = names.skill_class(s.param).map(class_name).unwrap_or("");
            format!("{} to {} ({class} Only)", signed(v), skill(s.param))
        }
        28 => format!("{} to {}", signed(v), skill(s.param)),
        // 1, 12 and the shapes not described.
        _ => place(s.val, &signed(v), n),
    }
}

/// Shapes 6–10: the stat's text and the per-level text of `descstr2`.
fn per_level(s: &Shape, v: &str) -> String {
    let base = place(s.val, v, s.name);
    match s.name2 {
        Some(t) if !t.is_empty() => format!("{base} {t}"),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct N;
    impl DescNames for N {
        fn skill(&self, id: u32) -> Option<String> {
            (id == 5).then(|| "Fire Bolt".to_owned())
        }
        fn skill_class(&self, id: u32) -> Option<u32> {
            (id == 5).then_some(1)
        }
    }

    fn line(func: u8, val: u8, value: i64, param: u32) -> String {
        render(
            &Shape {
                func,
                val,
                value,
                param,
                name: "Thing",
                name2: Some("(per level)"),
            },
            &N,
        )
    }

    #[test]
    fn each_descfunc_gives_its_line() {
        let cases: [(u8, u8, i64, u32, &str); 24] = [
            (1, 1, 5, 0, "+5 Thing"),
            (2, 1, 5, 0, "5% Thing"),
            (3, 1, 5, 0, "5 Thing"),
            (4, 1, 5, 0, "+5% Thing"),
            (5, 1, 64, 0, "50% Thing"),
            (6, 1, 5, 0, "+5 Thing (per level)"),
            (7, 1, 5, 0, "5% Thing (per level)"),
            (8, 1, 5, 0, "+5% Thing (per level)"),
            (9, 1, 5, 0, "5 Thing (per level)"),
            (10, 1, 64, 0, "50% Thing (per level)"),
            (11, 0, 20, 0, "Repairs 1 Durability In 5 Seconds"),
            (12, 1, 5, 0, "+5 Thing"),
            (13, 0, 2, 1, "+2 to Sorceress Skill Levels"),
            (14, 0, 3, 8, "+3 to Fire Skills (Sorceress Only)"),
            (
                15,
                0,
                10,
                (3 << 6) | 5,
                "10% Chance to cast level 3 Fire Bolt Thing",
            ),
            (16, 0, 4, 5, "Level 4 Fire Bolt Aura When Equipped"),
            (20, 1, 7, 0, "-7% Thing"),
            (21, 1, 7, 0, "-7 Thing"),
            (22, 1, 7, 0, "7% Thing"),
            (
                24,
                0,
                0x0A05,
                (2 << 6) | 5,
                "Level 2 Fire Bolt (5/10 Charges)",
            ),
            (27, 0, 1, 5, "+1 to Fire Bolt (Sorceress Only)"),
            (28, 0, 1, 5, "+1 to Fire Bolt"),
            (2, 2, 5, 0, "Thing 5%"),
            (4, 0, 5, 0, "Thing"),
        ];
        for (f, v, value, p, want) in cases {
            assert_eq!(line(f, v, value, p), want, "descfunc {f}");
        }
    }

    #[test]
    fn printf_shape_fills_the_value() {
        let s = Shape {
            func: 19,
            val: 0,
            value: 3,
            param: 0,
            name: "Adds %d Fire",
            name2: None,
        };
        assert_eq!(render(&s, &N), "Adds 3 Fire");
    }
}
