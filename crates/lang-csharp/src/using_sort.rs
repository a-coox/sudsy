//! The order of `using` directives, shared by the `using-order` rule and the formatter's
//! `sort_usings`: one place decides which group a directive belongs to and how names compare.

use crate::generated::{
    CompilationUnit, FileScopedNamespaceDeclaration, NamespaceDeclaration, UsingDirective, kind,
};
use core_rules::tree::{code_start, entry_range, significant_range};
use core_syntax::ast::AstNode;
use core_syntax::{Grammar, SyntaxNode, TextRange};
use std::cmp::Ordering;

/// How namespace names compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    /// Code points.
    Ordinal,
    /// Letters ignoring case, with punctuation before digits before letters; names that differ
    /// only in case put the lower-case one first.
    Culture,
    /// As `Culture`, but names that differ only in case stay in their order.
    CultureIgnoreCase,
}

/// What the order is made of.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    /// `System` and `System.*` come before other namespaces (and `using static System.*` before
    /// other `using static`).
    pub system_first: bool,
    pub compare: Compare,
    /// `using static` directives move behind the plain usings.
    pub statics_after: bool,
    /// Aliases move behind everything else.
    pub aliases_after: bool,
}

impl Order {
    /// The order of the formatter's `sort_usings`.
    pub const FORMATTER: Order = Order {
        system_first: true,
        compare: Compare::CultureIgnoreCase,
        statics_after: true,
        aliases_after: true,
    };
}

/// One directive of a list.
pub struct Entry {
    pub node: SyntaxNode,
    pub is_alias: bool,
    pub is_static: bool,
    /// `global using` and `using unsafe` keep their place.
    pub fixed: bool,
    /// The first name is `System` (`global::System` does not count: see `qualified`).
    pub system: bool,
    /// The name has a `global::` or other alias qualifier.
    pub qualified: bool,
    /// The namespace or type, without blanks.
    pub name: String,
    /// The alias of an alias directive.
    pub alias: String,
    /// A preprocessor directive stands before it.
    pub directive_before: bool,
}

fn compact(s: &str) -> String {
    s.split_whitespace().collect()
}

fn text<'a>(node: &SyntaxNode, source: &'a str, g: &Grammar) -> &'a str {
    let r = significant_range(node, g);
    &source[usize::from(r.start())..usize::from(r.end())]
}

/// The directives of the list `usings`, whose source is `source`.
pub fn entries(usings: &[UsingDirective], source: &str, g: &Grammar) -> Vec<Entry> {
    usings
        .iter()
        .map(|u| {
            let name = u
                .namespace_or_type()
                .map_or(String::new(), |t| compact(text(t.syntax(), source, g)));
            let alias = u
                .alias()
                .and_then(|a| a.name())
                .map_or(String::new(), |n| compact(text(n.syntax(), source, g)));
            let lead_start = usize::from(u.syntax().text_range().start());
            let lead_end = usize::from(code_start(u.syntax(), g));
            let is_alias = u.alias().is_some();
            Entry {
                node: u.syntax().clone(),
                is_alias,
                is_static: u.static_keyword().is_some(),
                fixed: u.global_keyword().is_some() || u.unsafe_keyword().is_some(),
                system: !is_alias && name.split(['.', '<']).next() == Some("System"),
                qualified: name.contains("::"),
                name,
                alias,
                directive_before: has_directive(&source[lead_start..lead_end]),
            }
        })
        .collect()
}

/// Does a line of `trivia` start with `#`?
fn has_directive(trivia: &str) -> bool {
    trivia.lines().any(|l| l.trim_start().starts_with('#'))
}

/// Primary weight of a character under the invariant culture: punctuation, digits, letters.
fn weight(c: char) -> (u8, u32) {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    if let Some(i) = PUNCTUATION.find(c) {
        (0, i as u32)
    } else if c.is_alphabetic() {
        (2, c.to_lowercase().next().map_or(c as u32, |l| l as u32))
    } else if c.is_numeric() {
        (1, c as u32)
    } else {
        (0, 1000 + c as u32)
    }
}

fn culture(a: &str, b: &str, tie_break_case: bool) -> Ordering {
    let primary = a.chars().map(weight).cmp(b.chars().map(weight));
    primary.then_with(|| {
        if !tie_break_case {
            return Ordering::Equal;
        }
        let differ = a.chars().zip(b.chars()).find(|(x, y)| x != y);
        match differ {
            Some((x, _)) if x.is_lowercase() => Ordering::Less,
            Some(_) => Ordering::Greater,
            None => Ordering::Equal,
        }
    })
}

/// Compares two namespace names.
pub fn compare_names(a: &str, b: &str, how: Compare) -> Ordering {
    match how {
        Compare::Ordinal => a.cmp(b),
        Compare::Culture => culture(a, b, true),
        Compare::CultureIgnoreCase => culture(a, b, false),
    }
}

/// Alias names compare by their lower-case code points.
fn compare_aliases(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// The group a directive is sorted into: plain System, plain, static System, static, alias.
fn group(e: &Entry, o: &Order) -> u8 {
    let system = o.system_first && e.system;
    match (e.is_alias, e.is_static) {
        (true, _) => 4,
        (_, true) => 2 + u8::from(!system),
        _ => u8::from(!(system && !e.qualified)),
    }
}

fn moves(e: &Entry, o: &Order) -> bool {
    !e.fixed
        && if e.is_alias {
            o.aliases_after
        } else if e.is_static {
            o.statics_after
        } else {
            true
        }
}

/// Where each directive goes: position `i` of the sorted list holds the directive `result[i]`.
/// Directives that keep their place (and statics or aliases the order leaves alone) stay at their
/// positions; the others fill the remaining positions in order.
pub fn sorted(entries: &[Entry], o: &Order) -> Vec<usize> {
    let slots: Vec<usize> = (0..entries.len())
        .filter(|&i| moves(&entries[i], o))
        .collect();
    let mut ordered = slots.clone();
    ordered.sort_by(|&a, &b| {
        let (x, y) = (&entries[a], &entries[b]);
        group(x, o).cmp(&group(y, o)).then_with(|| {
            if x.is_alias {
                compare_aliases(&x.alias, &y.alias)
            } else {
                compare_names(&x.name, &y.name, o.compare)
            }
        })
    });
    let mut result: Vec<usize> = (0..entries.len()).collect();
    for (&slot, &item) in slots.iter().zip(&ordered) {
        result[slot] = item;
    }
    result
}

/// A directive that is out of place: which, and why.
pub struct Misplaced {
    pub at: usize,
    pub message: String,
}

fn misplaced(at: usize, message: String) -> Misplaced {
    Misplaced { at, message }
}

/// The directives that are out of place under `o`, found the way StyleCop's SA1208 to SA1217
/// look for them: a System directive after another one, an alias before a plain directive,
/// neighbours that compare the wrong way round, an alias, a `using static` that is not where it
/// belongs. Lists restart at a directive that has a preprocessor directive in front of it.
pub fn misplaced_entries(entries: &[Entry], o: &Order) -> Vec<Misplaced> {
    let mut out = Vec::new();
    if o.system_first {
        system_before_others(entries, &mut out);
    }
    if o.aliases_after {
        aliases_after_others(entries, &mut out);
    }
    names_in_order(entries, o, &mut out);
    if o.aliases_after {
        aliases_in_order(entries, &mut out);
    }
    if o.statics_after {
        statics_placed(entries, &mut out);
        statics_in_order(entries, o, &mut out);
    }
    out.sort_by_key(|m| m.at);
    out
}

fn system_before_others(entries: &[Entry], out: &mut Vec<Misplaced>) {
    let mut before: Option<String> = None;
    for i in 1..entries.len() {
        let e = &entries[i];
        if e.is_alias || e.is_static || e.directive_before || !e.system || e.qualified {
            continue;
        }
        if before.is_none() {
            let previous = &entries[i - 1];
            if previous.system && !previous.qualified && !previous.is_static {
                continue;
            }
            before = Some(previous.name.clone());
        }
        let name = before.as_deref().unwrap_or_default();
        out.push(misplaced(
            i,
            format!(
                "`{}` should come before `{name}`: System usings go first",
                e.name
            ),
        ));
    }
}

fn aliases_after_others(entries: &[Entry], out: &mut Vec<Misplaced>) {
    for (i, pair) in entries.windows(2).enumerate() {
        let next = &pair[1];
        if pair[0].is_alias && !next.is_alias && !next.is_static && !next.directive_before {
            out.push(misplaced(
                i,
                "alias directives belong after the other usings".into(),
            ));
        }
    }
}

fn names_in_order(entries: &[Entry], o: &Order, out: &mut Vec<Misplaced>) {
    fn flush(list: &mut Vec<usize>, entries: &[Entry], o: &Order, out: &mut Vec<Misplaced>) {
        for pair in list.windows(2) {
            let (a, b) = (&entries[pair[0]], &entries[pair[1]]);
            if compare_names(&a.name, &b.name, o.compare) == Ordering::Greater {
                out.push(misplaced(
                    pair[0],
                    format!("`{}` should come after `{}`", a.name, b.name),
                ));
            }
        }
        list.clear();
    }
    let (mut others, mut system) = (Vec::new(), Vec::new());
    for (i, e) in entries.iter().enumerate() {
        if e.directive_before {
            flush(&mut others, entries, o, out);
            flush(&mut system, entries, o, out);
        }
        if e.is_alias || e.is_static {
            continue;
        }
        if e.qualified || !e.system || !o.system_first {
            others.push(i);
        } else {
            system.push(i);
        }
    }
    flush(&mut others, entries, o, out);
    flush(&mut system, entries, o, out);
}

fn aliases_in_order(entries: &[Entry], out: &mut Vec<Misplaced>) {
    let mut previous: Option<&Entry> = None;
    for (i, e) in entries.iter().enumerate() {
        if e.directive_before {
            previous = None;
        }
        if !e.is_alias {
            continue;
        }
        if let Some(p) = previous
            && compare_aliases(&p.alias, &e.alias) == Ordering::Greater
        {
            out.push(misplaced(
                i,
                format!("alias `{}` should come before `{}`", e.alias, p.alias),
            ));
            return;
        }
        previous = Some(e);
    }
}

fn statics_placed(entries: &[Entry], out: &mut Vec<Misplaced>) {
    let (mut last_static, mut last_alias) = (None, None);
    for (i, e) in entries.iter().enumerate() {
        if e.directive_before {
            (last_static, last_alias) = (None, None);
        }
        if e.is_static {
            if last_alias.is_some() {
                out.push(misplaced(
                    i,
                    "`using static` belongs before the aliases".into(),
                ));
                return;
            }
            last_static = Some(i);
        } else if e.is_alias {
            last_alias = Some(i);
        } else if let Some(s) = last_static {
            out.push(misplaced(
                s,
                "`using static` belongs after the other usings".into(),
            ));
            return;
        }
    }
}

fn statics_in_order(entries: &[Entry], o: &Order, out: &mut Vec<Misplaced>) {
    let (mut last_static, mut last_system, mut first_other): (
        Option<usize>,
        Option<usize>,
        Option<usize>,
    ) = (None, None, None);
    for (i, e) in entries.iter().enumerate() {
        if e.directive_before {
            (last_static, last_system, first_other) = (None, None, None);
        }
        if !e.is_static {
            continue;
        }
        let after = |from: usize| {
            let a = &entries[from];
            compare_names(&a.name, &e.name, o.compare) == Ordering::Greater
        };
        if o.system_first && e.system {
            if let Some(f) = first_other {
                out.push(misplaced(
                    f,
                    format!(
                        "`using static {}` should come after the System statics",
                        entries[f].name
                    ),
                ));
                return;
            }
            if let Some(l) = last_system.filter(|&l| after(l)) {
                out.push(misplaced(
                    l,
                    format!(
                        "`using static {}` should come after `{}`",
                        entries[l].name, e.name
                    ),
                ));
                return;
            }
            last_system = Some(i);
        } else {
            if let Some(l) = last_static.filter(|&l| after(l)) {
                out.push(misplaced(
                    l,
                    format!(
                        "`using static {}` should come after `{}`",
                        entries[l].name, e.name
                    ),
                ));
                return;
            }
            last_static = Some(i);
            first_other = first_other.or(Some(i));
        }
    }
}

/// The usings of a compilation unit or namespace.
pub fn usings_of(node: &SyntaxNode) -> Option<Vec<UsingDirective>> {
    if let Some(n) = CompilationUnit::cast(node.clone()) {
        Some(n.usings().collect())
    } else if let Some(n) = NamespaceDeclaration::cast(node.clone()) {
        Some(n.usings().collect())
    } else {
        FileScopedNamespaceDeclaration::cast(node.clone()).map(|n| n.usings().collect())
    }
}

/// The range from the first directive to the end of the last (with its trailing comment), and
/// whether a preprocessor directive stands inside it.
pub fn list_span(entries: &[Entry], source: &str, g: &Grammar) -> Option<(TextRange, bool)> {
    let first = entries.first()?;
    let last = entries.last()?;
    let span = TextRange::new(
        significant_range(&first.node, g).start(),
        entry_range(&last.node, g).end(),
    );
    let inside = &source[usize::from(span.start())..usize::from(span.end())];
    Some((span, has_directive(inside)))
}

/// `source` with the using lists sorted by [`Order::FORMATTER`]: every directive keeps the
/// comments on the lines above it, and the blank lines between directives go. Lists with
/// preprocessor directives are left alone.
pub fn sort_all(root: &SyntaxNode, source: &str, g: &Grammar) -> Option<String> {
    let mut edits: Vec<(TextRange, String)> = Vec::new();
    for node in root.descendants() {
        if !matches!(
            node.kind(),
            kind::COMPILATION_UNIT
                | kind::NAMESPACE_DECLARATION
                | kind::FILE_SCOPED_NAMESPACE_DECLARATION
        ) {
            continue;
        }
        let Some(usings) = usings_of(&node) else {
            continue;
        };
        let entries = entries(&usings, source, g);
        let Some((span, directives)) = list_span(&entries, source, g) else {
            continue;
        };
        if directives {
            continue;
        }
        edits.push((span, sorted_list(&entries, source, g)));
    }
    if edits.is_empty() {
        return None;
    }
    let mut out = source.to_string();
    for (span, new) in edits.into_iter().rev() {
        out.replace_range(usize::from(span.start())..usize::from(span.end()), &new);
    }
    Some(out)
}

fn sorted_list(entries: &[Entry], source: &str, g: &Grammar) -> String {
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let bodies: Vec<TextRange> = entries.iter().map(|e| entry_range(&e.node, g)).collect();
    // The comment lines above each directive but the first, whose are left in front of the list.
    let leads: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            if i == 0 {
                return String::new();
            }
            let from = usize::from(bodies[i - 1].end());
            let to = usize::from(code_start(&e.node, g));
            source[from..to]
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let t = l.trim_start();
                    let l = if t.starts_with("//") || t.starts_with("/*") {
                        t
                    } else {
                        l
                    };
                    format!("{}{newline}", l.trim_end())
                })
                .collect()
        })
        .collect();
    sorted(entries, &Order::FORMATTER)
        .into_iter()
        .map(|i| {
            let b = bodies[i];
            format!(
                "{}{}",
                leads[i],
                &source[usize::from(b.start())..usize::from(b.end())]
            )
        })
        .collect::<Vec<_>>()
        .join(newline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn culture_puts_punctuation_before_digits_before_letters_and_ignores_case() {
        let ordered = [
            "_x",
            "A_b",
            "A.B",
            "a.b",
            "A1",
            "Ab",
            "b",
            "Microsoft.A",
            "z",
            "Z",
            "Z1",
        ];
        for pair in ordered.windows(2) {
            assert_ne!(
                compare_names(pair[0], pair[1], Compare::CultureIgnoreCase),
                Ordering::Greater,
                "{} before {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn culture_puts_lower_case_first_only_when_asked() {
        assert_eq!(compare_names("aa", "Aa", Compare::Culture), Ordering::Less);
        assert_eq!(
            compare_names("aa", "Aa", Compare::CultureIgnoreCase),
            Ordering::Equal
        );
        assert_eq!(
            compare_names("Zeta", "alpha", Compare::Ordinal),
            Ordering::Less
        );
        assert_eq!(
            compare_names("System", "System.A", Compare::Culture),
            Ordering::Less
        );
    }
}
