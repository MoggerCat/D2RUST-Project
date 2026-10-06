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
    // M08: promoting the first D2MOO-only server-do row (6) without a
    // body is reported.
    let bad = SRVDO_TSV.replacen("\tD2MOO-only\n", "\tspec'd-here\n", 1);
    assert_eq!(
        check(&bad, &SRV_DO_IMPLEMENTED),
        Err(
            "spec'd-here [1, 2, 3, 5, 6, 7, 8, 10, 25], implemented [1, 2, 3, 5, 7, 8, 10, 25]"
                .into()
        )
    );
    // A body without a spec'd-here row is reported.
    assert_eq!(
        check(SRVHIT_TSV, &[1, 2, 4, 12, 13]),
        Err("spec'd-here [1, 4, 12, 13], implemented [1, 2, 4, 12, 13]".into())
    );
}
