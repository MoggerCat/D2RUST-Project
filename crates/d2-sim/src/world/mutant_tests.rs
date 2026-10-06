// Spec: specs/world/quests.md, specs/world/waypoints.md, specs/world/cube.md
//! Mutation-testing kills for the shared TSV reader (METHODS M08). The
//! line numbers are the reader's own contract (1-based, counting the
//! header and blank lines), not a spec rule, so no coverage claim.

use super::{tsv_rows, TsvError};

const HEADER: &[&str] = &["a", "b"];

#[test]
fn tsv_rows_report_one_based_file_lines() {
    let rows = tsv_rows("t", "a\tb\n1\t2\n\n3\t4\n", HEADER).unwrap();
    let lines: Vec<usize> = rows.iter().map(|(l, _)| *l).collect();
    assert_eq!(lines, [2, 4]);
}

#[test]
fn tsv_rows_column_error_names_its_line() {
    let err = tsv_rows("t", "a\tb\n1\t2\n\n3\n", HEADER).unwrap_err();
    assert_eq!(
        err,
        TsvError::Columns {
            table: "t",
            line: 4,
            got: 1,
            want: 2,
        }
    );
}
