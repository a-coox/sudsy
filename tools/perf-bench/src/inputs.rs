//! Deterministic benchmark inputs.

/// xorshift64*: no dependency, same output everywhere.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
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

/// `items` tables with strings, numbers, an inline table and an array each. About 190 bytes per
/// item.
pub fn toml(items: usize) -> String {
    let mut r = Rng::new(0x70c1);
    let mut s = String::with_capacity(items * 200);
    for i in 0..items {
        let (a, b, c) = (r.word(), r.word(), r.word());
        s.push_str(&format!(
            "[package{i}]\nname = \"{a}-{b}\"\nversion = \"0.{}.{}\"\nfeatures = [\"{a}\", \"{b}\", \"{c}\"]\ndep = {{ version = \"1\", optional = true, features = [\"{c}\"] }}\n# {c}\nweight = {}\n\n",
            r.below(100),
            r.below(100),
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

/// Objects nested `depth` levels deep, a few members at each level.
pub fn json_nested(depth: usize) -> String {
    let mut s = String::new();
    for _ in 0..depth {
        s.push_str("{\"a\": 1, \"b\": [1, 2], \"c\": ");
    }
    s.push_str("null");
    s.push_str(&"}".repeat(depth));
    s.push('\n');
    s
}

/// Block mappings nested `depth` levels deep, a few entries at each level.
pub fn yaml_nested(depth: usize) -> String {
    let mut s = String::new();
    for i in 0..depth {
        let pad = "  ".repeat(i);
        s.push_str(&format!("{pad}a: 1\n{pad}b: [1, 2]\n{pad}c:\n"));
    }
    s.push_str(&format!("{}d: 1\n", "  ".repeat(depth)));
    s
}
