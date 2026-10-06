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
#[cfg(test)]
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
mod tests {
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

    #[test]
    fn rejects_bad_table_type() {
        assert!(decompress(&[9, 0, 0], 16).is_err());
    }
}
