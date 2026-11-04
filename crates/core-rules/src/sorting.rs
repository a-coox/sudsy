//! Ordering shared by the `sorted-keys` rules (JSON, YAML) and `sorted-array-values`: the options,
//! the comparison and the plan that says which entries are out of place and where each belongs.

use crate::options::{Opt, OptionDecl, RuleOptions};
use regex::Regex;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;

pub const ORDER: OptionDecl = OptionDecl {
    name: "order",
    default: Opt::List(&[]),
    description: "Keys that come first, in this order; the rest follow in the configured order.",
};
pub const DIRECTION: OptionDecl = OptionDecl {
    name: "direction",
    default: Opt::Str("asc"),
    description: "`asc` or `desc`.",
};
pub const COMPARE: OptionDecl = OptionDecl {
    name: "compare",
    default: Opt::Str("ordinal"),
    description: "`ordinal` (code points), `case_insensitive`, or `natural` (digit runs compare \
                  as numbers: `a2` before `a10`).",
};
pub const IGNORED_KEYS: OptionDecl = OptionDecl {
    name: "ignored_keys",
    default: Opt::List(&[]),
    description: "Regular expressions; matching keys stay where they are and are not compared.",
};
pub const MIN_KEYS: OptionDecl = OptionDecl {
    name: "min_keys",
    default: Opt::Int(2),
    description: "Mappings with fewer keys are not checked.",
};
pub const BLANK_LINE_GROUPS: OptionDecl = OptionDecl {
    name: "blank_line_groups",
    default: Opt::Bool(false),
    description: "Sort each run of entries between blank lines on its own.",
};

#[derive(Clone, Copy, PartialEq)]
enum Compare {
    Ordinal,
    CaseInsensitive,
    Natural,
}

/// The ordering options of a rule, read once per check.
pub struct Sorting<'a> {
    order: Vec<&'a str>,
    descending: bool,
    compare: Compare,
    ignored: Vec<Regex>,
    min: usize,
    blank_line_groups: bool,
}

/// What [`Sorting::plan`] found: `sorted[p]` is the index of the entry that belongs at position
/// `p`; `misplaced` the entries that sort before an entry above them (what yamllint
/// reports); `moved` the fewest entries that have to move (what eslint-plugin-jsonc reports).
pub struct Plan {
    pub sorted: Vec<usize>,
    pub misplaced: Vec<usize>,
    pub moved: Vec<usize>,
}

/// Is there an empty line in `gap`, the text between two entries?
pub fn has_blank_line(gap: &str) -> bool {
    let lines: Vec<&str> = gap.split('\n').collect();
    lines.len() > 2
        && lines[1..lines.len() - 1]
            .iter()
            .any(|l| l.trim().is_empty())
}

thread_local! {
    static REGEXES: RefCell<HashMap<String, Option<Regex>>> = RefCell::new(HashMap::new());
}

/// A compiled pattern, compiled once per thread; a pattern that does not compile matches nothing.
pub fn cached_regex(pattern: &str) -> Option<Regex> {
    REGEXES.with(|c| {
        c.borrow_mut()
            .entry(pattern.to_string())
            .or_insert_with(|| Regex::new(pattern).ok())
            .clone()
    })
}

impl<'a> Sorting<'a> {
    /// Unknown `direction` and `compare` values mean the defaults. `min_option` names the
    /// threshold option of the rule (`min_keys`, `min_values`).
    pub fn new(options: &'a RuleOptions, min_option: &str) -> Self {
        Sorting {
            order: options.list("order"),
            descending: options.string("direction") == "desc",
            compare: match options.string("compare") {
                "case_insensitive" => Compare::CaseInsensitive,
                "natural" => Compare::Natural,
                _ => Compare::Ordinal,
            },
            ignored: options
                .list("ignored_keys")
                .into_iter()
                .filter_map(cached_regex)
                .collect(),
            min: usize::try_from(options.int(min_option)).unwrap_or(0),
            blank_line_groups: options.bool("blank_line_groups"),
        }
    }

    /// `ord` for the configured direction.
    pub fn directed(&self, ord: Ordering) -> Ordering {
        if self.descending { ord.reverse() } else { ord }
    }

    fn text(&self, a: &str, b: &str) -> Ordering {
        self.directed(match self.compare {
            Compare::Ordinal => a.cmp(b),
            Compare::CaseInsensitive => a
                .chars()
                .flat_map(char::to_lowercase)
                .cmp(b.chars().flat_map(char::to_lowercase)),
            Compare::Natural => natural(a, b),
        })
    }

    /// The plan for entries with these keys. `blank_before(i)` says whether a blank line precedes
    /// entry `i` (`i > 0`); it is only asked with `blank_line_groups`. `None` when nothing is out
    /// of place.
    pub fn plan_keys<K: AsRef<str>>(
        &self,
        keys: &[K],
        blank_before: impl Fn(usize) -> bool,
    ) -> Option<Plan> {
        let rank = |k: &str| {
            self.order
                .iter()
                .position(|o| *o == k)
                .unwrap_or(self.order.len())
        };
        self.plan(
            keys.len(),
            blank_before,
            |i| {
                !self.ignored.is_empty()
                    && self.ignored.iter().any(|r| r.is_match(keys[i].as_ref()))
            },
            |a, b| {
                let (a, b) = (keys[a].as_ref(), keys[b].as_ref());
                let (ra, rb) = (rank(a), rank(b));
                if ra == rb && ra == self.order.len() {
                    self.text(a, b)
                } else {
                    ra.cmp(&rb)
                }
            },
        )
    }

    /// The plan for `n` entries ordered by `cmp` (final order, direction included). Entries for
    /// which `skip` hold stay where they are. Sorted input costs one pass and no allocation.
    pub fn plan(
        &self,
        n: usize,
        blank_before: impl Fn(usize) -> bool,
        skip: impl Fn(usize) -> bool,
        cmp: impl Fn(usize, usize) -> Ordering,
    ) -> Option<Plan> {
        if n < self.min {
            return None;
        }
        let starts_group = |i: usize| self.blank_line_groups && i > 0 && blank_before(i);
        // Below the greatest entry so far in the group, not only below its neighbour.
        let mut misplaced = Vec::new();
        let mut top: Option<usize> = None;
        for i in 0..n {
            if starts_group(i) {
                top = None;
            }
            if skip(i) {
                continue;
            }
            match top {
                Some(t) if cmp(t, i) == Ordering::Greater => misplaced.push(i),
                _ => top = Some(i),
            }
        }
        if misplaced.is_empty() {
            return None;
        }
        let mut sorted: Vec<usize> = (0..n).collect();
        let mut moved = Vec::new();
        let mut group: Vec<usize> = Vec::new();
        for i in 0..=n {
            if i == n || starts_group(i) {
                let mut by = group.clone();
                by.sort_by(|&a, &b| cmp(a, b));
                if by != group {
                    moved.extend(moved_from(&group, &cmp));
                }
                for (slot, item) in group.iter().zip(by) {
                    sorted[*slot] = item;
                }
                group.clear();
            }
            if i < n && !skip(i) {
                group.push(i);
            }
        }
        Some(Plan {
            sorted,
            misplaced,
            moved,
        })
    }
}

/// The entries of `group` outside a longest in-order subsequence: the fewest that have to move
/// for the group to be sorted. Of several longest subsequences the patience-sorting one is taken,
/// which keeps later entries (`b, a` moves `b`).
fn moved_from(group: &[usize], cmp: &impl Fn(usize, usize) -> Ordering) -> Vec<usize> {
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; group.len()];
    for (p, &item) in group.iter().enumerate() {
        let at = tails.partition_point(|&t| cmp(group[t], item) != Ordering::Greater);
        prev[p] = at.checked_sub(1).map(|k| tails[k]);
        if at == tails.len() {
            tails.push(p);
        } else {
            tails[at] = p;
        }
    }
    let mut kept = vec![false; group.len()];
    let mut at = tails.last().copied();
    while let Some(p) = at {
        kept[p] = true;
        at = prev[p];
    }
    (0..group.len())
        .filter(|&p| !kept[p])
        .map(|p| group[p])
        .collect()
}

/// The end of the digit run at `from` and its digits without leading zeros.
fn run(s: &[u8], from: usize) -> (usize, &[u8]) {
    let len = s[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let digits = &s[from..from + len];
    let zeros = digits.iter().take_while(|&&c| c == b'0').count();
    (from + len, &digits[zeros..])
}

/// Digit runs compare as numbers, everything else by code point.
fn natural(a: &str, b: &str) -> Ordering {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < x.len() && j < y.len() {
        if x[i].is_ascii_digit() && y[j].is_ascii_digit() {
            let (ni, dx) = run(x, i);
            let (nj, dy) = run(y, j);
            let ord = dx.len().cmp(&dy.len()).then_with(|| dx.cmp(dy));
            if ord != Ordering::Equal {
                return ord;
            }
            (i, j) = (ni, nj);
        } else if x[i] != y[j] {
            return x[i].cmp(&y[j]);
        } else {
            (i, j) = (i + 1, j + 1);
        }
    }
    (x.len() - i).cmp(&(y.len() - j)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_config::OptionValue;
    use std::collections::BTreeMap;

    fn sorting(set: &[(&str, OptionValue)]) -> RuleOptions {
        let decls = [
            ORDER,
            DIRECTION,
            COMPARE,
            IGNORED_KEYS,
            MIN_KEYS,
            BLANK_LINE_GROUPS,
        ];
        let configured: BTreeMap<_, _> = set
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        RuleOptions::resolve(&decls, &configured).unwrap()
    }

    fn s(v: &str) -> OptionValue {
        OptionValue::String(v.into())
    }

    fn sorted(opts: &RuleOptions, keys: &[&str]) -> Vec<usize> {
        let plan = Sorting::new(opts, "min_keys").plan_keys(keys, |_| false);
        plan.map_or_else(|| (0..keys.len()).collect(), |p| p.sorted)
    }

    #[test]
    fn modes_and_direction() {
        let keys = ["b", "A", "a10", "a2"];
        assert_eq!(sorted(&sorting(&[]), &keys), [1, 2, 3, 0]);
        let ci = sorting(&[("compare", s("case_insensitive"))]);
        assert_eq!(sorted(&ci, &["b", "A", "a1"]), [1, 2, 0]);
        let nat = sorting(&[("compare", s("natural"))]);
        assert_eq!(sorted(&nat, &keys), [1, 3, 2, 0]);
        let desc = sorting(&[("direction", s("desc"))]);
        assert_eq!(sorted(&desc, &keys), [0, 3, 2, 1]);
        assert_eq!(natural("a2", "a10"), Ordering::Less);
        assert_eq!(natural("a02", "a2"), Ordering::Less);
    }

    #[test]
    fn blank_lines() {
        assert!(has_blank_line(",\n\n  "));
        assert!(has_blank_line(",\r\n  \r\n  "));
        assert!(!has_blank_line(",\n  // c\n  "));
        assert!(!has_blank_line(", "));
    }

    #[test]
    fn ignored_keys_stay_and_groups_sort_alone() {
        let ignored = sorting(&[("ignored_keys", OptionValue::List(vec![s("^x")]))]);
        assert_eq!(sorted(&ignored, &["c", "x1", "a"]), [2, 1, 0]);
        let groups = sorting(&[("blank_line_groups", OptionValue::Bool(true))]);
        let plan = Sorting::new(&groups, "min_keys")
            .plan_keys(&["b", "a", "d", "c"], |i| i == 2)
            .unwrap();
        assert_eq!(
            (plan.sorted, plan.misplaced),
            (vec![1, 0, 3, 2], vec![1, 3])
        );
        assert_eq!(plan.moved, [0, 2]);
    }
}
