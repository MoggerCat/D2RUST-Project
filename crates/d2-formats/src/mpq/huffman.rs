// Spec: specs/formats/mpq.md (§11 Huffman, Storm adaptive Huffman)

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use super::bits::BitReader;
use super::tables::HUFFMAN_WEIGHTS;
use super::CodecError;

const SYM_END: u16 = 0x100;
const SYM_ESCAPE: u16 = 0x101;

/// Output bytes per input byte, at most: every symbol takes at least one
/// bit. Bounds the preallocation, since `max_out` is untrusted.
const MAX_RATIO: usize = 8;

fn err(reason: &'static str) -> CodecError {
    CodecError {
        codec: "huffman",
        reason,
    }
}

#[derive(Clone)]
struct Node {
    weight: u32,
    /// `Some(symbol)` for a leaf, `None` for a branch.
    symbol: Option<u16>,
    /// Branch children: [child0, child1].
    child: [usize; 2],
    parent: Option<usize>,
    /// Neighbours in the weight-ordered list (head = heaviest).
    prev: Option<usize>,
    next: Option<usize>,
}

/// The tree plus its weight-ordered node list. Cloned from a per-table
/// template for every stream.
#[derive(Clone)]
struct Tree {
    nodes: Vec<Node>,
    head: usize,
    tail: usize,
    /// For each weight, the first node in the list (closest to the head)
    /// with that weight.
    leader: HashMap<u32, usize>,
}

impl Tree {
    /// Build (§11): leaves in symbol order, then pairwise branches from
    /// the tail.
    fn build(weights: &[u8; 256]) -> Tree {
        let mut b = Builder::default();
        for (sym, &w) in weights.iter().enumerate() {
            if w != 0 {
                b.insert_leaf(sym as u16, u32::from(w));
            }
        }
        b.insert_leaf(SYM_END, 1);
        b.insert_leaf(SYM_ESCAPE, 1);

        // Inserting branches can move the head, so re-read it every pass.
        let mut c = b.tail.expect("tree has leaves");
        while Some(c) != b.head {
            let a = c;
            let bb = b.nodes[a].prev.expect("a is not the head");
            let w = b.nodes[a].weight + b.nodes[bb].weight;
            let m = b.push(Node {
                weight: w,
                symbol: None,
                child: [a, bb],
                parent: None,
                prev: None,
                next: None,
            });
            b.nodes[a].parent = Some(m);
            b.nodes[bb].parent = Some(m);
            b.insert(m);
            match b.nodes[bb].prev {
                Some(p) => c = p,
                None => break,
            }
        }

        let head = b.head.expect("tree has nodes");
        let tail = b.tail.expect("tree has nodes");
        let mut leader = HashMap::new();
        let mut cur = Some(head);
        while let Some(i) = cur {
            leader.entry(b.nodes[i].weight).or_insert(i);
            cur = b.nodes[i].next;
        }
        Tree {
            nodes: b.nodes,
            head,
            tail,
            leader,
        }
    }

    fn root(&self) -> usize {
        self.head
    }

    /// Exchanges the list positions of `x` and `y`.
    fn swap_list(&mut self, x: usize, y: usize) {
        if self.nodes[x].next == Some(y) {
            self.move_before(y, x);
        } else if self.nodes[y].next == Some(x) {
            self.move_before(x, y);
        } else {
            let (xp, xn) = (self.nodes[x].prev, self.nodes[x].next);
            let (yp, yn) = (self.nodes[y].prev, self.nodes[y].next);
            self.link(xp, Some(y));
            self.link(Some(y), xn);
            self.link(yp, Some(x));
            self.link(Some(x), yn);
        }
    }

    /// Swaps two adjacent nodes: `anchor` immediately precedes `node`, and
    /// afterwards `node` immediately precedes `anchor`.
    fn move_before(&mut self, node: usize, anchor: usize) {
        let before = self.nodes[anchor].prev;
        let after = self.nodes[node].next;
        self.link(before, Some(node));
        self.link(Some(node), Some(anchor));
        self.link(Some(anchor), after);
    }

    /// Makes `b` follow `a` (either may be `None` for a list end).
    fn link(&mut self, a: Option<usize>, b: Option<usize>) {
        match a {
            Some(a) => self.nodes[a].next = b,
            None => {
                if let Some(b) = b {
                    self.head = b;
                }
            }
        }
        match b {
            Some(b) => self.nodes[b].prev = a,
            None => {
                if let Some(a) = a {
                    self.tail = a;
                }
            }
        }
    }

    /// Exchanges the tree positions of `x` and `y`.
    fn swap_tree(&mut self, x: usize, y: usize) {
        let px = self.nodes[x].parent.expect("non-root");
        let py = self.nodes[y].parent.expect("non-root");
        if px == py {
            self.nodes[px].child.swap(0, 1);
        } else {
            let sx = usize::from(self.nodes[px].child[1] == x);
            let sy = usize::from(self.nodes[py].child[1] == y);
            self.nodes[px].child[sx] = y;
            self.nodes[py].child[sy] = x;
            self.nodes[x].parent = Some(py);
            self.nodes[y].parent = Some(px);
        }
    }

    /// Increment (§11) for `n` and each ancestor up to the root.
    fn increment(&mut self, mut n: usize) -> Result<(), CodecError> {
        loop {
            let w = self.nodes[n].weight;
            let w1 = w.checked_add(1).ok_or(err("weight overflow"))?;
            let lead = *self.leader.get(&w).ok_or(err("inconsistent tree"))?;
            if lead != n {
                if self.nodes[lead].parent.is_none()
                    || self.nodes[n].parent.is_none()
                    || self.nodes[n].parent == Some(lead)
                {
                    return Err(err("swap with an ancestor"));
                }
                self.swap_list(n, lead);
                self.swap_tree(n, lead);
            }
            // `n` now heads its weight block; the block's new leader is
            // whatever follows it with the same weight.
            match self.nodes[n].next {
                Some(x) if self.nodes[x].weight == w => {
                    self.leader.insert(w, x);
                }
                _ => {
                    self.leader.remove(&w);
                }
            }
            self.nodes[n].weight = w1;
            self.leader.entry(w1).or_insert(n);
            match self.nodes[n].parent {
                Some(p) => n = p,
                None => return Ok(()),
            }
        }
    }

    /// AddValue (§11): a new leaf for `v` paired with the old tail.
    fn add_value(&mut self, v: u8) -> usize {
        let b = self.tail;
        let a = self.nodes.len();
        self.nodes.push(Node {
            weight: 0,
            symbol: Some(u16::from(v)),
            child: [0, 0],
            parent: None,
            prev: None,
            next: None,
        });
        self.link(Some(b), Some(a));
        self.link(Some(a), None);
        self.leader.entry(0).or_insert(a);

        let bw = self.nodes[b].weight;
        let parent = self.nodes[b].parent;
        let m = self.nodes.len();
        self.nodes.push(Node {
            weight: bw,
            symbol: None,
            child: [a, b],
            parent,
            prev: None,
            next: None,
        });
        match parent {
            Some(p) => {
                let side = usize::from(self.nodes[p].child[1] == b);
                self.nodes[p].child[side] = m;
            }
            None => self.head = m,
        }
        let before = self.nodes[b].prev;
        self.link(before, Some(m));
        self.link(Some(m), Some(b));
        if self.leader.get(&bw) == Some(&b) {
            self.leader.insert(bw, m);
        }
        self.nodes[a].parent = Some(m);
        self.nodes[b].parent = Some(m);
        a
    }
}

/// List construction for `Tree::build`, with the insert rule from §11.
#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    head: Option<usize>,
    tail: Option<usize>,
    /// For each weight, the last node in the list with that weight.
    last_of: BTreeMap<u32, usize>,
}

impl Builder {
    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn insert_leaf(&mut self, sym: u16, weight: u32) {
        let i = self.push(Node {
            weight,
            symbol: Some(sym),
            child: [0, 0],
            parent: None,
            prev: None,
            next: None,
        });
        self.insert(i);
    }

    /// Places node `i` right after the last node with weight ≥ its weight,
    /// or at the head if there is none.
    fn insert(&mut self, i: usize) {
        let w = self.nodes[i].weight;
        // The last node with weight ≥ w is the last node of the smallest
        // weight ≥ w present.
        let after = self.last_of.range(w..).next().map(|(_, &n)| n);
        match after {
            Some(a) => {
                let next = self.nodes[a].next;
                self.nodes[i].prev = Some(a);
                self.nodes[i].next = next;
                self.nodes[a].next = Some(i);
                match next {
                    Some(n) => self.nodes[n].prev = Some(i),
                    None => self.tail = Some(i),
                }
            }
            None => {
                self.nodes[i].prev = None;
                self.nodes[i].next = self.head;
                match self.head {
                    Some(h) => self.nodes[h].prev = Some(i),
                    None => self.tail = Some(i),
                }
                self.head = Some(i);
            }
        }
        self.last_of.insert(w, i);
    }
}

fn template(table: usize) -> &'static Tree {
    static TEMPLATES: OnceLock<Vec<Tree>> = OnceLock::new();
    &TEMPLATES.get_or_init(|| HUFFMAN_WEIGHTS.iter().map(Tree::build).collect())[table]
}

/// Decodes a Huffman stream into at most `max_out` bytes.
pub(crate) fn decompress(input: &[u8], max_out: usize) -> Result<Vec<u8>, CodecError> {
    let mut r = BitReader::new(input);
    let table = r.read(8)? as usize;
    if table >= HUFFMAN_WEIGHTS.len() {
        return Err(err("invalid table type"));
    }
    let adaptive = table == 0;
    let mut tree = template(table).clone();

    let mut out = Vec::with_capacity(max_out.min(input.len().saturating_mul(MAX_RATIO)));
    while out.len() < max_out {
        let mut n = tree.root();
        while tree.nodes[n].symbol.is_none() {
            let bit = r.read(1)? as usize;
            n = tree.nodes[n].child[bit];
        }
        let sym = tree.nodes[n].symbol.expect("leaf");
        let byte = match sym {
            SYM_END => break,
            SYM_ESCAPE => {
                let v = r.read(8)? as u8;
                n = tree.add_value(v);
                tree.increment(n)?;
                if !adaptive {
                    tree.increment(n)?;
                }
                v
            }
            s => s as u8,
        };
        out.push(byte);
        if adaptive {
            tree.increment(n)?;
        }
    }
    Ok(out)
}

/// Encodes `data` with weight table `table`, mirroring [`decompress`]'s
/// tree updates, and appends the end symbol. Test helper for building
/// valid streams.
#[cfg(any(test, feature = "bench-fixtures"))]
pub(crate) fn compress(table: u8, data: &[u8]) -> Vec<u8> {
    use super::bits::BitWriter;

    fn leaf(tree: &Tree, sym: u16) -> Option<usize> {
        (0..tree.nodes.len()).find(|&i| tree.nodes[i].symbol == Some(sym))
    }
    fn emit(tree: &Tree, w: &mut BitWriter, mut n: usize) {
        let mut path = Vec::new();
        while let Some(p) = tree.nodes[n].parent {
            path.push(u32::from(tree.nodes[p].child[1] == n));
            n = p;
        }
        for &bit in path.iter().rev() {
            w.write(bit, 1);
        }
    }

    let mut w = BitWriter::default();
    w.write(u32::from(table), 8);
    let adaptive = table == 0;
    let mut tree = template(usize::from(table)).clone();
    for &b in data {
        let n = match leaf(&tree, u16::from(b)) {
            Some(n) => {
                emit(&tree, &mut w, n);
                n
            }
            None => {
                let esc = leaf(&tree, SYM_ESCAPE).expect("escape leaf");
                emit(&tree, &mut w, esc);
                w.write(u32::from(b), 8);
                let n = tree.add_value(b);
                tree.increment(n).expect("valid tree");
                if !adaptive {
                    tree.increment(n).expect("valid tree");
                }
                n
            }
        };
        if adaptive {
            tree.increment(n).expect("valid tree");
        }
    }
    let end = leaf(&tree, SYM_END).expect("end leaf");
    emit(&tree, &mut w, end);
    w.bytes
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn compress_round_trip() {
        let data: Vec<u8> = b"the quick brown fox \x00\xff jumps over the lazy dog"
            .iter()
            .copied()
            .chain((0..=255u8).step_by(7))
            .collect();
        for t in 0..HUFFMAN_WEIGHTS.len() as u8 {
            let stream = compress(t, &data);
            assert_eq!(decompress(&stream, 4096).unwrap(), data, "table {t}");
        }
    }

    /// Checks the invariants every tree must keep: the list is non-increasing
    /// in weight, branches weigh the sum of their children, parent links
    /// match, and `leader` points at the first node of each weight.
    fn check(tree: &Tree) {
        let mut cur = Some(tree.head);
        let mut prev_w = u32::MAX;
        let mut seen = HashMap::new();
        let mut count = 0;
        while let Some(i) = cur {
            let n = &tree.nodes[i];
            assert!(n.weight <= prev_w, "list order");
            prev_w = n.weight;
            seen.entry(n.weight).or_insert(i);
            if n.symbol.is_none() {
                let [c0, c1] = n.child;
                assert_eq!(n.weight, tree.nodes[c0].weight + tree.nodes[c1].weight);
                assert_eq!(tree.nodes[c0].parent, Some(i));
                assert_eq!(tree.nodes[c1].parent, Some(i));
            }
            count += 1;
            cur = n.next;
        }
        assert_eq!(count, tree.nodes.len(), "every node is in the list");
        assert_eq!(tree.nodes[tree.head].parent, None, "head is the root");
        assert_eq!(seen, tree.leader, "leader map");
    }

    #[test]
    fn templates_are_consistent() {
        for t in 0..HUFFMAN_WEIGHTS.len() {
            check(template(t));
        }
    }

    #[test]
    fn invariants_hold_under_updates() {
        for t in [0usize, 1, 3, 6] {
            let mut tree = template(t).clone();
            for v in [7u8, 200, 7, 33, 0, 255] {
                let n = tree.add_value(v);
                tree.increment(n).unwrap();
                tree.increment(n).unwrap();
                check(&tree);
            }
            // Increment some existing leaves many times.
            let leaves: Vec<usize> = (0..tree.nodes.len())
                .filter(|&i| tree.nodes[i].symbol.is_some())
                .take(12)
                .collect();
            for round in 0..40 {
                let n = leaves[round % leaves.len()];
                tree.increment(n).unwrap();
            }
            check(&tree);
        }
    }

    /// Weights are u32 and grow with every decoded symbol; past u32::MAX
    /// (only reachable with a multi-GiB stream) `w + 1` overflowed.
    #[test]
    fn regress_weight_overflow_is_an_error() {
        let mut tree = template(0).clone();
        let root = tree.root();
        let w = tree.nodes[root].weight;
        tree.leader.remove(&w);
        tree.nodes[root].weight = u32::MAX;
        tree.leader.insert(u32::MAX, root);
        assert!(tree.increment(root).is_err());
    }

    // Covers: specs/formats/mpq.md §11 text
    #[test]
    fn rejects_bad_table_type() {
        assert!(decompress(&[9, 0, 0], 16).is_err());
    }

    // -----------------------------------------------------------------
    // A plain reference model of §11, written from the spec text: the
    // ordered list `L` is a `Vec` and every lookup is a linear scan.

    #[derive(Clone, Debug, PartialEq)]
    pub(crate) struct Model {
        w: Vec<u32>,
        sym: Vec<Option<u16>>,
        child: Vec<[usize; 2]>,
        parent: Vec<Option<usize>>,
        /// `L`, head first.
        l: Vec<usize>,
    }

    impl Model {
        fn node(&mut self, w: u32, sym: Option<u16>, child: [usize; 2]) -> usize {
            self.w.push(w);
            self.sym.push(sym);
            self.child.push(child);
            self.parent.push(None);
            self.w.len() - 1
        }

        fn pos(&self, n: usize) -> usize {
            self.l.iter().position(|&x| x == n).expect("in L")
        }

        /// Insert rule: right after the last node with weight ≥ w, else
        /// at the head.
        fn insert(&mut self, n: usize) {
            let w = self.w[n];
            let at = self
                .l
                .iter()
                .rposition(|&x| self.w[x] >= w)
                .map_or(0, |p| p + 1);
            self.l.insert(at, n);
        }

        /// Build, steps 1–3.
        fn build(weights: &[u8; 256]) -> Model {
            let mut m = Model {
                w: vec![],
                sym: vec![],
                child: vec![],
                parent: vec![],
                l: vec![],
            };
            for (s, &w) in weights.iter().enumerate() {
                if w != 0 {
                    let n = m.node(u32::from(w), Some(s as u16), [0, 0]);
                    m.insert(n);
                }
            }
            for s in [SYM_END, SYM_ESCAPE] {
                let n = m.node(1, Some(s), [0, 0]);
                m.insert(n);
            }
            let mut c = *m.l.last().unwrap();
            while c != m.l[0] {
                let a = c;
                let b = m.l[m.pos(a) - 1];
                let br = m.node(m.w[a] + m.w[b], None, [a, b]);
                m.parent[a] = Some(br);
                m.parent[b] = Some(br);
                m.insert(br);
                let pb = m.pos(b);
                if pb == 0 {
                    break;
                }
                c = m.l[pb - 1];
            }
            m
        }

        fn root(&self) -> usize {
            self.l[0]
        }

        /// Increment(n), steps 1–3, up to and including the root.
        fn increment(&mut self, mut n: usize) -> Result<(), ()> {
            loop {
                let w = self.w[n];
                let lead = *self.l.iter().find(|&&x| self.w[x] == w).unwrap();
                if lead != n {
                    // (The implementation rejects a swap with the root or
                    // with n's own parent; valid streams never reach it.)
                    let (pn, pl) = (self.parent[n].ok_or(())?, self.parent[lead].ok_or(())?);
                    if pn == lead {
                        return Err(());
                    }
                    let (a, b) = (self.pos(n), self.pos(lead));
                    self.l.swap(a, b);
                    if pn == pl {
                        self.child[pn].swap(0, 1);
                    } else {
                        let sn = usize::from(self.child[pn][1] == n);
                        let sl = usize::from(self.child[pl][1] == lead);
                        self.child[pn][sn] = lead;
                        self.child[pl][sl] = n;
                        self.parent[n] = Some(pl);
                        self.parent[lead] = Some(pn);
                    }
                }
                self.w[n] = w + 1;
                match self.parent[n] {
                    Some(p) => n = p,
                    None => return Ok(()),
                }
            }
        }

        /// AddValue(v), steps 1–3.
        fn add_value(&mut self, v: u8) -> usize {
            let a = self.node(0, Some(u16::from(v)), [0, 0]);
            self.insert(a);
            assert_eq!(*self.l.last().unwrap(), a, "the new leaf is the tail");
            let b = self.l[self.pos(a) - 1];
            let m = self.node(self.w[b], None, [a, b]);
            let pb = self.parent[b];
            if let Some(p) = pb {
                let side = usize::from(self.child[p][1] == b);
                self.child[p][side] = m;
            }
            self.parent[m] = pb;
            let at = self.pos(b);
            self.l.insert(at, m);
            self.parent[a] = Some(m);
            self.parent[b] = Some(m);
            a
        }

        /// Decode, steps 1–5. `escape_increments` is 1 or 2 for tables
        /// 1–8 (the spec says 2); table 0 always uses the adaptive rule.
        pub(crate) fn decode(
            input: &[u8],
            max_out: usize,
            escape_increments: u32,
        ) -> Result<Vec<u8>, ()> {
            let mut bit = 0usize;
            let mut read = |n: u32| -> Result<u32, ()> {
                let mut v = 0;
                for k in 0..n {
                    let byte = *input.get(bit / 8).ok_or(())?;
                    v |= u32::from(byte >> (bit % 8) & 1) << k;
                    bit += 1;
                }
                Ok(v)
            };
            let t = read(8)? as usize;
            if t > 8 {
                return Err(());
            }
            let adaptive = t == 0;
            let mut m = Model::build(&HUFFMAN_WEIGHTS[t]);
            let mut out = Vec::new();
            while out.len() < max_out {
                let mut n = m.root();
                while m.sym[n].is_none() {
                    n = m.child[n][read(1)? as usize];
                }
                let sym = match m.sym[n].unwrap() {
                    SYM_END => break,
                    SYM_ESCAPE => {
                        let v = read(8)? as u8;
                        n = m.add_value(v);
                        m.increment(n)?;
                        if !adaptive && escape_increments == 2 {
                            m.increment(n)?;
                        }
                        v
                    }
                    s => s as u8,
                };
                out.push(sym);
                if adaptive {
                    m.increment(n)?;
                }
            }
            Ok(out)
        }
    }

    /// The implementation's tree in model form (node indices match: both
    /// create nodes in the order the spec does).
    fn as_model(t: &Tree) -> Model {
        let mut l = vec![];
        let mut cur = Some(t.head);
        while let Some(i) = cur {
            l.push(i);
            cur = t.nodes[i].next;
        }
        let branch = |n: &Node| if n.symbol.is_none() { n.child } else { [0, 0] };
        Model {
            w: t.nodes.iter().map(|n| n.weight).collect(),
            sym: t.nodes.iter().map(|n| n.symbol).collect(),
            child: t.nodes.iter().map(branch).collect(),
            parent: t.nodes.iter().map(|n| n.parent).collect(),
            l,
        }
    }

    /// Deterministic byte source for the property checks.
    fn lcg_bytes(seed: u32, len: usize) -> Vec<u8> {
        let mut x = seed;
        (0..len)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (x >> 16) as u8
            })
            .collect()
    }

    // Covers: specs/formats/mpq.md §11 r1, §11 r2, §11 r3
    #[test]
    fn build_matches_reference_model() {
        for (t, weights) in HUFFMAN_WEIGHTS.iter().enumerate() {
            let model = Model::build(weights);
            let tree = template(t);
            assert_eq!(as_model(tree), model, "table {t}");
            assert_eq!(tree.root(), model.root());
            // Leaves: every non-zero weight in symbol order, then 0x100, 0x101.
            let leaves: Vec<u16> = model.sym.iter().map_while(|s| *s).collect();
            let mut want: Vec<u16> = (0..256u16)
                .filter(|&s| weights[usize::from(s)] != 0)
                .collect();
            want.extend([SYM_END, SYM_ESCAPE]);
            assert_eq!(leaves, want);
        }
    }

    // Covers: specs/formats/mpq.md §11 l2 r1, §11 l2 r2, §11 l2 r3, §11 l3 r1, §11 l3 r2, §11 l3 r3
    #[test]
    fn updates_match_reference_model() {
        for (t, weights) in HUFFMAN_WEIGHTS.iter().enumerate() {
            let mut tree = template(t).clone();
            let mut model = Model::build(weights);
            let ops = lcg_bytes(t as u32 + 1, 600);
            for (k, &op) in ops.iter().enumerate() {
                if k % 5 == 0 {
                    // AddValue, then Increment of the new leaf.
                    let a = tree.add_value(op);
                    assert_eq!(a, model.add_value(op));
                    assert_eq!(as_model(&tree), model, "table {t} add {k}");
                    tree.increment(a).unwrap();
                    model.increment(a).unwrap();
                } else {
                    // Increment an existing leaf picked by `op`.
                    let leaves: Vec<usize> = (0..model.w.len())
                        .filter(|&i| model.sym[i].is_some())
                        .collect();
                    let n = leaves[usize::from(op) % leaves.len()];
                    tree.increment(n).unwrap();
                    model.increment(n).unwrap();
                }
                assert_eq!(as_model(&tree), model, "table {t} op {k}");
            }
        }
    }

    // Covers: specs/formats/mpq.md §11 l4 r1, §11 l4 r2, §11 l4 r3, §11 l4 r4, §11 l4 r5
    #[test]
    fn decode_matches_reference_model() {
        let data: Vec<u8> = b"Stay awhile and listen. \x00\x01\xfe\xff"
            .iter()
            .copied()
            .chain(lcg_bytes(99, 300))
            .collect();
        for t in 0..HUFFMAN_WEIGHTS.len() as u8 {
            // Valid streams (every escape path), full and capped output.
            let stream = compress(t, &data);
            for max in [0, 1, 17, data.len(), 4096] {
                let got = decompress(&stream, max).map_err(|_| ());
                assert_eq!(got, Model::decode(&stream, max, 2), "table {t} max {max}");
            }
            assert_eq!(Model::decode(&stream, 4096, 2).unwrap(), data);
            // Arbitrary bit streams: same output or both an error.
            for seed in 0..40 {
                let mut s = vec![t];
                s.extend(lcg_bytes(seed * 9 + u32::from(t), 64));
                let got = decompress(&s, 4096).map_err(|_| ());
                assert_eq!(got, Model::decode(&s, 4096, 2), "table {t} seed {seed}");
            }
        }
    }
}
