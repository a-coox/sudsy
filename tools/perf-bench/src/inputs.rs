//! Deterministic benchmark inputs.

/// xorshift64*: no dependency, same output everywhere.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn word(&mut self) -> &'static str {
        const WORDS: [&str; 12] = [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
            "juliet", "kilo", "lima",
        ];
        WORDS[self.below(WORDS.len() as u64) as usize]
    }
}

/// Pretty-printed JSON: an array of `objects` objects with nested objects and short arrays.
/// About 265 bytes per object.
pub fn json(objects: usize) -> String {
    let mut r = Rng::new(0x5ed5);
    let mut s = String::with_capacity(objects * 280);
    s.push_str("[\n");
    for i in 0..objects {
        if i > 0 {
            s.push_str(",\n");
        }
        let (a, b, c) = (r.word(), r.word(), r.word());
        let n = r.below(100_000);
        s.push_str(&format!(
            "  {{\n    \"id\": {i},\n    \"name\": \"{a}-{b}-{i}\",\n    \
             \"tags\": [\n      \"{a}\",\n      \"{b}\"\n    ],\n    \
             \"owner\": {{\n      \"first\": \"{b}\",\n      \
             \"address\": {{\n        \"city\": \"{c}\",\n        \"zip\": {n}\n      }}\n    }},\n    \
             \"scores\": [\n      {},\n      {},\n      {}\n    ]\n  }}",
            r.below(100),
            r.below(100),
            r.below(100),
        ));
    }
    s.push_str("\n]\n");
    s
}

/// A block sequence of `items` mappings, each with a flow list and a nested mapping. About 85
/// bytes per item.
pub fn yaml(items: usize) -> String {
    let mut r = Rng::new(0x7a31);
    let mut s = String::with_capacity(items * 90);
    for i in 0..items {
        let (a, b, c) = (r.word(), r.word(), r.word());
        s.push_str(&format!(
            "- id: {i}\n  tags: [{a}, {b}, {c}]\n  owner:\n    first: {b}\n    zip: {}\n",
            r.below(100_000),
        ));
    }
    s
}

/// One-line minified JSON: an object of `members` members with small flat arrays. The printer
/// has to break the object and measure every array on its own.
pub fn json_flat(members: usize) -> String {
    let mut r = Rng::new(0x1f1a);
    let mut s = String::with_capacity(members * 24);
    s.push('{');
    for i in 0..members {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "\"k{i}\":[{},{},{}]",
            r.below(1000),
            r.below(1000),
            r.below(1000)
        ));
    }
    s.push_str("}\n");
    s
}
