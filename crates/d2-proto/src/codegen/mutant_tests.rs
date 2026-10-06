// Spec: specs/sim/intents-events.md (§5)
//! Test written against a surviving mutant (METHODS M08): the generator's
//! rule for typed structs (module doc) on a row the TSVs do not have.

use super::*;

/// Only fixed fields get a typed struct: a `cstr` at an offset in a
/// fixed-size row does not.
#[test]
fn a_cstr_field_makes_a_row_untyped() {
    let at = |ty| Field {
        name: "a",
        ty,
        offset: Some(1),
    };
    assert_eq!(
        typed("X", &SizeRule::Fixed(5), &[at(FieldType::U32)]),
        Some(5)
    );
    assert_eq!(
        typed("X", &SizeRule::Fixed(5), &[at(FieldType::Cstr)]),
        None
    );
    assert_eq!(
        typed("X", &SizeRule::Fixed(5), &[at(FieldType::Tail)]),
        None
    );
}
