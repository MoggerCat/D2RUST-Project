//! The per-skill bodies of `bodies` (`functions.tsv` notes), run through
//! the start core on the `tests` fake.

use super::super::bodies::{self, DO_BODIES, START_BODIES};
use super::*;

/// The `functions.tsv` note that states each body here, by (kind, index).
const BODY_NOTES: &[(&str, u16, &str)] = &[("srvst", 18, "\"return 1\"")];

/// Every row of `functions.tsv` whose notes state a body ("1.14d body is
/// …") must have that body in `bodies`, on a filled slot; every other
/// slot must have none. Returns the mismatches.
fn bodies_mismatches(tsv: &str) -> Vec<String> {
    let mut out = Vec::new();
    for l in tsv.lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        let (Some(kind), Some(Ok(index))) = (c.first(), c.get(1).map(|i| i.parse::<u16>())) else {
            continue;
        };
        let has = match *kind {
            "srvst" => bodies::start(index).is_some(),
            "srvdo" => bodies::do_(index).is_some(),
            _ => continue,
        };
        let notes = c.get(6).copied().unwrap_or("");
        // A note states a body as "1.14d body is …"; the code's body must
        // be the one stated.
        let noted = notes.contains("1.14d body is ");
        let same = BODY_NOTES
            .iter()
            .any(|&(k, i, n)| k == *kind && i == index && notes.contains(n));
        if has != noted || (has && !same) {
            out.push(format!("{kind} {index}"));
            continue;
        }
        if has && c.get(2) == Some(&"null") {
            out.push(format!("{kind} {index}"));
        }
    }
    out
}

// Covers: specs/skills/use.md §8
#[test]
fn bodies_match_tsv_notes() {
    assert_eq!(bodies_mismatches(FUNCTIONS_TSV), Vec::<String>::new());
    assert_eq!(START_BODIES, [18]);
    assert!(DO_BODIES.is_empty());
    for &i in START_BODIES {
        assert!(table::lookup(Kind::Start, i).is_some());
    }
    for i in 0..table::START_SLOTS {
        assert_eq!(bodies::start(i).is_some(), START_BODIES.contains(&i));
    }
    for i in 0..table::DO_SLOTS {
        assert_eq!(bodies::do_(i).is_some(), DO_BODIES.contains(&i));
    }
}

// Covers: specs/skills/use.md §8
#[test]
fn bodies_check_reports_perturbations() {
    // M08: the note of srvst 18 no longer states the body.
    let bad = FUNCTIONS_TSV.replacen("1.14d body is \"return 1\"", "1.14d body is unknown", 1);
    assert_ne!(bad, FUNCTIONS_TSV);
    assert_eq!(bodies_mismatches(&bad), ["srvst 18"]);
    // A slot that states a body the code lacks.
    let bad = FUNCTIONS_TSV.replacen(
        "SrvSt02_Kick\tmapped\tKick\t",
        "SrvSt02_Kick\tmapped\tKick\t1.14d body is \"return 1\"",
        1,
    );
    assert_ne!(bad, FUNCTIONS_TSV);
    assert_eq!(bodies_mismatches(&bad), ["srvst 2"]);
}

// Covers: specs/skills/use.md §5.3 r6
#[test]
fn attract_start_returns_one_and_charges_at_start() {
    let t = tables(
        3,
        &[
            (1, &|r: &mut Skills| {
                r.srvstfunc = 18;
                mana(r, 4, 1, 8, 0);
            }),
            (2, &|r: &mut Skills| {
                r.srvstfunc = 18;
                r.periodic = true;
            }),
        ],
    );
    let mut f = F::new();
    // The seam would refuse: the body answers instead and the seam is
    // never called.
    f.srvst_ret = 0;
    let p = caster(&mut f, 1, 1, 2000);
    f.take_log();
    assert_eq!(start(&mut f, &t, p), 1);
    assert!(f.take_log().is_empty());
    // r ≠ 0, no usemanaondo: (4 + 0) << 8 charged at start.
    assert_eq!(f.get(p, stat::MANA), 2000 - 1024);
    // r ≠ 0 and periodic: type-8 timers with arg 0 deleted after it.
    let q = caster(&mut f, 2, 1, 0);
    f.take_log();
    assert_eq!(start(&mut f, &t, q), 1);
    assert_eq!(f.take_log(), ["delete 1 8 0"]);
}

// Covers: specs/skills/use.md §5.3 r6
#[test]
fn attract_start_with_usemanaondo_charges_nothing() {
    let t = tables(
        2,
        &[(1, &|r: &mut Skills| {
            r.srvstfunc = 18;
            r.usemanaondo = true;
            mana(r, 4, 1, 8, 0);
        })],
    );
    let mut f = F::new();
    let p = caster(&mut f, 1, 1, 2000);
    assert_eq!(start(&mut f, &t, p), 1);
    assert_eq!(f.get(p, stat::MANA), 2000);
}
