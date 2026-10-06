// Spec: specs/tools/scenario.md §2, §3 (round trip of the canonical form)
//! Random scripts, written as text in any accepted spelling (hex or
//! decimal numbers, fields in any order, lines shuffled before the
//! steps, comments), parse to a scenario whose canonical text parses
//! back to the same scenario and is a fixed point of the writer.

use conformance::scenario::Scenario;
use proptest::prelude::*;

fn number(v: u32) -> impl Strategy<Value = String> {
    prop_oneof![
        Just(v.to_string()),
        Just(format!("0x{v:x}")),
        Just(format!("0x{v:X}"))
    ]
}

fn reference() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("@player".to_owned()),
        (prop_oneof![Just("x"), Just("y")], -300i32..300).prop_map(|(a, d)| match d {
            0 => format!("@{a}"),
            d if d > 0 => format!("@{a}+{d}"),
            d => format!("@{a}{d}"),
        }),
        (0u8..6, proptest::option::of(0u32..700), 0u32..4).prop_map(|(t, c, n)| {
            let mut s = format!("@{t}");
            if let Some(c) = c {
                s.push_str(&format!(":{c}"));
            }
            if n > 0 {
                s.push_str(&format!("#{n}"));
            }
            s
        }),
        (0u32..3).prop_map(|n| if n == 0 {
            "@wp".to_owned()
        } else {
            format!("@wp#{n}")
        }),
    ]
}

/// A value for a field of `max`: a number (any spelling) or a reference.
fn value(max: u32) -> impl Strategy<Value = String> {
    prop_oneof![(0..=max).prop_flat_map(number), reference()]
}

/// A typed or hex step body.
fn body() -> impl Strategy<Value = String> {
    let point = |name: &'static str| {
        (value(0xFFFF), value(0xFFFF), any::<bool>()).prop_map(move |(x, y, swap)| {
            if swap {
                format!("msg {name} y={y} x={x}")
            } else {
                format!("msg {name} x={x} y={y}")
            }
        })
    };
    prop_oneof![
        point("Walk"),
        point("Run"),
        point("RightSkill"),
        (value(5), value(u32::MAX))
            .prop_map(|(t, id)| format!("msg InteractWithEntity id={id} type={t}")),
        (0u32..0x8000_0000, 0u32..2, value(u32::MAX))
            .prop_map(|(s, l, i)| format!("msg SelectSkill left={l} skill={s} item={i}")),
        (value(u32::MAX), 0u32..200)
            .prop_map(|(wp, l)| format!("msg TakeOrCloseWp wp={wp} level={l}")),
        proptest::collection::vec(any::<u8>(), 1..20).prop_map(|b| {
            let h: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
            format!("hex {}", h.join(" "))
        }),
    ]
}

fn script() -> impl Strategy<Value = String> {
    (
        (
            any::<u32>(),
            0usize..3,
            any::<bool>(),
            0u32..500,
            any::<u32>(),
        ),
        (
            0u8..7,
            1u8..100,
            proptest::option::of((0u32..5000, 0u32..5000)),
        ),
        proptest::collection::btree_map(0u16..400, -100_000i32..100_000, 0..4),
        proptest::collection::btree_map(0u16..400, 1u8..=255, 0..4),
        proptest::sample::subsequence(vec!["s2c", "rng", "units", "stats", "frames"], 0..=5),
        (
            proptest::option::of(1u32..100),
            proptest::collection::btree_set(0u32..500, 0..4),
        ),
        proptest::collection::vec((0u32..500, body()), 0..12),
        any::<u64>(),
    )
        .prop_map(
            |(
                (seed, diff, exp, end, map),
                (class, level, at),
                stats,
                skills,
                record,
                (every, at_ticks),
                steps,
                shuffle,
            )| {
                let mut head = vec![
                    "name prop".to_owned(),
                    "game 1.14d".to_owned(),
                    format!("seed {seed}"),
                    format!("map {map}"),
                    format!("difficulty {}", ["normal", "nightmare", "hell"][diff]),
                    format!("expansion {}", if exp { "yes" } else { "no" }),
                    format!("end {end}"),
                    format!("char class {class}"),
                    format!("char level {level}"),
                    "char area 0 1".to_owned(),
                ];
                if let Some((x, y)) = at {
                    head.push(format!("char at {x} {y}   # a comment"));
                }
                head.extend(stats.iter().map(|(s, v)| format!("char stat {s} {v}")));
                head.extend(skills.iter().map(|(s, l)| format!("char skill {s} {l}")));
                head.push("char item hp1 belt 3".to_owned());
                let snaps = record.contains(&"units") || record.contains(&"stats");
                if !record.is_empty() {
                    head.push(format!("record {}", record.join(" ")));
                }
                if snaps {
                    if let Some(n) = every {
                        head.push(format!("snapshot every {n}"));
                    }
                    let t: Vec<String> = at_ticks
                        .iter()
                        .filter(|&&t| t <= end)
                        .map(u32::to_string)
                        .collect();
                    if !t.is_empty() {
                        head.push(format!("snapshot at {}", t.join(" ")));
                    }
                }
                // Header lines in any order (`scenario.md` §2 rule 3).
                let mut k = shuffle;
                for i in (1..head.len()).rev() {
                    head.swap(i, (k % (i as u64 + 1)) as usize);
                    k /= i as u64 + 1;
                }
                let mut steps: Vec<(u32, String)> =
                    steps.into_iter().map(|(t, b)| (t % (end + 1), b)).collect();
                steps.sort_by_key(|s| s.0);
                let mut text = String::from("# generated\nscenario 1\n");
                for l in head {
                    text.push_str(&l);
                    text.push('\n');
                }
                text.push('\n');
                for (t, b) in steps {
                    text.push_str(&format!("at {t} {b}\n"));
                }
                text
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Covers: specs/tools/scenario.md §2 r1, §2 r3, §2 r4, §2 r6, §3 r2
    #[test]
    fn canonical_form_round_trips(text in script()) {
        let s = Scenario::parse(&text).map_err(|e| TestCaseError::fail(format!("{e}\n{text}")))?;
        let canonical = s.to_text();
        let again = Scenario::parse(&canonical).map_err(|e| TestCaseError::fail(format!("{e}\n{canonical}")))?;
        prop_assert_eq!(&again, &s);
        prop_assert_eq!(again.to_text(), canonical);
    }
}
