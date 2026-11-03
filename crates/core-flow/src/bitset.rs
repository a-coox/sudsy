/// A fixed-size set of small integers (locals, definitions).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitSet(Vec<u64>);

impl BitSet {
    pub fn new(len: usize) -> Self {
        BitSet(vec![0; len.div_ceil(64)])
    }

    pub fn full(len: usize) -> Self {
        let mut s = BitSet(vec![u64::MAX; len.div_ceil(64)]);
        if let Some(last) = s.0.last_mut()
            && !len.is_multiple_of(64)
        {
            *last = (1u64 << (len % 64)) - 1;
        }
        s
    }

    pub fn insert(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }

    pub fn remove(&mut self, i: usize) {
        self.0[i / 64] &= !(1 << (i % 64));
    }

    pub fn contains(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }

    /// Adds the members of `other`; true if this set changed.
    pub fn union_with(&mut self, other: &BitSet) -> bool {
        let mut changed = false;
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            let n = *a | *b;
            changed |= n != *a;
            *a = n;
        }
        changed
    }

    /// Keeps only the members of `other`; true if this set changed.
    pub fn intersect_with(&mut self, other: &BitSet) -> bool {
        let mut changed = false;
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            let n = *a & *b;
            changed |= n != *a;
            *a = n;
        }
        changed
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(w, &bits)| {
            (0..64)
                .filter(move |b| bits & (1 << b) != 0)
                .map(move |b| w * 64 + b)
        })
    }
}
