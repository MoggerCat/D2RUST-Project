// Spec: specs/monsters/umod-callbacks.md §28.1; specs/monsters/umods.tsv (columns cl_phase0–cl_phase4)
//! The client umod hook table `0x00724E28` (§28.1 r1: 43 umods × 5
//! phases, 0 = no hook) and the dispatcher's visiting order
//! `0x004ADD90` (r2). The hook bodies (§28.2: client missiles, overlays,
//! sounds) are the effect layer's (Phase 6); this module only says which
//! hooks a monster's umod list calls in a phase, in order.

/// Rows of the table (umods 0…42).
pub const UMODS: usize = 43;
/// Phases 0–4 (§28.1 r3).
pub const PHASES: usize = 5;

const fn row(
    u: usize,
    phase: usize,
    hook: u32,
    mut t: [[u32; PHASES]; UMODS],
) -> [[u32; PHASES]; UMODS] {
    t[u][phase] = hook;
    t
}

/// `0x00724E28`: the hook of (umod, phase), 0 when null (§28.2).
pub const HOOKS: [[u32; PHASES]; UMODS] = {
    let t = [[0; PHASES]; UMODS];
    let t = row(9, 2, 0x004A_D1A0, t);
    let t = row(10, 2, 0x004A_D510, t);
    let t = row(11, 2, 0x004A_D5F0, t);
    let t = row(17, 2, 0x004A_D890, t);
    let t = row(17, 3, 0x004A_D8E0, t);
    let t = row(18, 2, 0x004A_D510, t);
    let t = row(20, 2, 0x004A_D920, t);
    let t = row(22, 1, 0x004A_D2F0, t);
    let t = row(23, 0, 0x004A_D890, t);
    let t = row(29, 4, 0x004A_D970, t);
    let t = row(31, 2, 0x004A_D0C0, t);
    let t = row(32, 2, 0x004A_DB20, t);
    let t = row(33, 2, 0x004A_DBC0, t);
    let t = row(40, 2, 0x004A_DD80, t);
    row(42, 2, 0x004A_D510, t)
};

/// The hooks `0x004ADD90` calls for a monster's 9-byte umod list in
/// `phase` (§28.1 r2): every byte in order, **including bytes after a
/// 0** (umod 0's row is null), each non-null (umod, hook). A byte past
/// the table (≥ 43) reads outside it in 1.14d: `None`.
pub fn hooks_for(umods: &[u8; 9], phase: usize) -> Option<Vec<(u8, u32)>> {
    let mut out = Vec::new();
    for &u in umods {
        let hook = *HOOKS.get(usize::from(u))?.get(phase)?;
        if hook != 0 {
            out.push((u, hook));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TSV: &str = include_str!("../../../../specs/monsters/umods.tsv");

    /// The (umod, phase, hook) mismatches between `umods.tsv`'s
    /// `cl_phase*` columns and [`HOOKS`] (M05).
    fn mismatches(tsv: &str) -> Vec<(usize, usize)> {
        let mut lines = tsv.lines();
        let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
        let col = |n: &str| header.iter().position(|h| *h == n).unwrap();
        let id = col("id");
        let cols: Vec<usize> = (0..PHASES).map(|p| col(&format!("cl_phase{p}"))).collect();
        let mut seen = [false; UMODS];
        let mut out = Vec::new();
        for l in lines {
            let f: Vec<&str> = l.split('\t').collect();
            let u: usize = f[id].parse().unwrap();
            if u >= UMODS {
                out.push((u, PHASES));
                continue;
            }
            seen[u] = true;
            for (p, &c) in cols.iter().enumerate() {
                let want = match f[c] {
                    "-" => 0,
                    s => u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap(),
                };
                if HOOKS[u][p] != want {
                    out.push((u, p));
                }
            }
        }
        out.extend((0..UMODS).filter(|&u| !seen[u]).map(|u| (u, PHASES)));
        out
    }

    // Covers: specs/monsters/umod-callbacks.md §28.1 r1, §28.2
    #[test]
    fn hooks_match_the_tsv() {
        assert_eq!(mismatches(TSV), []);
        // M08: a changed cell is reported exactly.
        let bad = TSV.replacen("0x004AD970", "0x004AD971", 1);
        assert_ne!(bad, TSV);
        assert_eq!(mismatches(&bad), [(29, 4)]);
    }

    // Covers: specs/monsters/umod-callbacks.md §28.1 r2
    #[test]
    fn dispatch_visits_every_byte_in_order() {
        // Bytes after a 0 still run; 0 and hookless umods add nothing.
        let list = [17, 0, 9, 1, 17, 0, 0, 0, 42];
        assert_eq!(
            hooks_for(&list, 2),
            Some(vec![
                (17, 0x004A_D890),
                (9, 0x004A_D1A0),
                (17, 0x004A_D890),
                (42, 0x004A_D510)
            ])
        );
        assert_eq!(
            hooks_for(&list, 3),
            Some(vec![(17, 0x004A_D8E0), (17, 0x004A_D8E0)])
        );
        assert_eq!(hooks_for(&[0; 9], 4), Some(vec![]));
        assert_eq!(hooks_for(&[43, 0, 0, 0, 0, 0, 0, 0, 0], 0), None);
    }
}
