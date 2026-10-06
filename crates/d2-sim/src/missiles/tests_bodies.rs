// Spec: specs/missiles/missiles.md §R9.2 (catalogue `status` column)
//! The server-do / server-hit bodies implemented here are exactly the
//! non-null catalogue rows whose `status` is `spec'd-here` (METHODS M05):
//! a row promoted in a TSV without a body here, or a body landing for a
//! row the spec has not read in full, fails this check.

use super::catalogue::{SRVDO_TSV, SRVHIT_TSV, SRV_DO_IMPLEMENTED, SRV_HIT_IMPLEMENTED};

/// Indices of the non-null (`addr_114d` ≠ `-`) rows with status
/// `spec'd-here`.
fn spec_d_here(tsv: &str) -> Vec<i16> {
    tsv.lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            (c[1] != "-" && c[8] == "spec'd-here").then(|| c[0].parse().unwrap())
        })
        .collect()
}

fn check(tsv: &str, implemented: &[i16]) -> Result<(), String> {
    let spec = spec_d_here(tsv);
    let mut ours = implemented.to_vec();
    ours.sort_unstable();
    if spec == ours {
        Ok(())
    } else {
        Err(format!("spec'd-here {spec:?}, implemented {ours:?}"))
    }
}

// Covers: specs/missiles/missiles.md §r9-2-tsv-columns-srvdo-tsv-srvhit-tsv
#[test]
fn bodies_match_catalogue_status() {
    check(SRVDO_TSV, &SRV_DO_IMPLEMENTED).unwrap();
    check(SRVHIT_TSV, &SRV_HIT_IMPLEMENTED).unwrap();
}

#[test]
fn bodies_check_catches_perturbations() {
    // M08: promoting server-do 2 without a body is reported.
    let bad = SRVDO_TSV.replacen("\tD2MOO-only\n", "\tspec'd-here\n", 1);
    assert_eq!(
        check(&bad, &SRV_DO_IMPLEMENTED),
        Err("spec'd-here [1, 2], implemented [1]".into())
    );
    // A body without a spec'd-here row is reported.
    assert_eq!(
        check(SRVHIT_TSV, &[1]),
        Err("spec'd-here [], implemented [1]".into())
    );
}
