//! Formatting the `#if` branches that the file's own symbols leave inactive.
//!
//! An inactive branch is one verbatim disabled-text token. The file is parsed again under other
//! symbol sets (the `preprocessor_symbol_sets` option, then sets derived from the conditions)
//! until every branch is active in some parse; each parse is formatted, and the text between
//! two directive lines of the base output is replaced by the same stretch of the output of a
//! parse where it was active. The directive lines of all outputs must match one to one, or
//! nothing is merged. The caller checks the result against every parse it took text from.

use super::options::{DERIVE_SYMBOL_SETS, PREPROCESSOR_SYMBOL_SETS};
use crate::generated::kind as k;
use core_config::ResolvedOptions;
use core_format::Variants;
use core_syntax::SyntaxNode;

/// Derived symbol sets tried at most.
const MAX_DERIVED_SETS: usize = 16;

/// The `#if`/`#elif`/`#else`/`#endif` lines of a parse and the branches between them.
struct Shape {
    /// The text of each directive, in order.
    directives: Vec<String>,
    /// Whether the branch before directive `i` (the last one: after the final directive) is
    /// disabled text with something in it.
    dead: Vec<bool>,
}

impl Shape {
    fn of(tree: &SyntaxNode) -> Shape {
        let mut starts = Vec::new();
        let mut directives = Vec::new();
        for node in tree.descendants() {
            let kind = node.kind();
            if kind == k::IF_DIRECTIVE_TRIVIA
                || kind == k::ELIF_DIRECTIVE_TRIVIA
                || kind == k::ELSE_DIRECTIVE_TRIVIA
                || kind == k::END_IF_DIRECTIVE_TRIVIA
            {
                starts.push(usize::from(node.text_range().start()));
                directives.push(node.text().to_string().trim().to_string());
            }
        }
        let mut dead = vec![false; directives.len() + 1];
        for token in tree
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::DISABLED_TEXT_TRIVIA)
        {
            if !token.text().trim().is_empty() {
                let at = usize::from(token.text_range().start());
                dead[starts.partition_point(|s| *s <= at)] = true;
            }
        }
        Shape { directives, dead }
    }
}

/// `#if X`, `# elif Y`, `#else`, `#endif`: a line of a conditional directive.
fn is_directive_line(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim_start();
    ["if", "elif", "else", "endif"].iter().any(|word| {
        rest.strip_prefix(word)
            .is_some_and(|after| !after.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    })
}

/// The output cut at its directive lines: the text between them (one more piece than lines)
/// and the lines.
fn split(output: &str) -> (Vec<&str>, Vec<&str>) {
    let (mut pieces, mut lines) = (Vec::new(), Vec::new());
    let (mut piece_start, mut at) = (0, 0);
    for line in output.split_inclusive('\n') {
        if is_directive_line(line) {
            pieces.push(&output[piece_start..at]);
            lines.push(line);
            piece_start = at + line.len();
        }
        at += line.len();
    }
    pieces.push(&output[piece_start..]);
    (pieces, lines)
}

/// The identifiers of the conditions of `#if` and `#elif`, each once, in order.
fn condition_symbols(directives: &[String]) -> Vec<String> {
    let mut symbols: Vec<String> = Vec::new();
    for directive in directives {
        let body = directive.trim_start_matches('#').trim_start();
        let Some(condition) = body
            .strip_prefix("elif")
            .or_else(|| body.strip_prefix("if"))
        else {
            continue;
        };
        for word in condition.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            let is_name = word.starts_with(|c: char| c.is_alphabetic() || c == '_');
            if is_name && word != "true" && word != "false" && !symbols.iter().any(|s| s == word) {
                symbols.push(word.to_string());
            }
        }
    }
    symbols
}

/// Up to [`MAX_DERIVED_SETS`] subsets of `symbols`: none, all, then by growing size.
fn derived_sets(symbols: &[String]) -> Vec<Vec<String>> {
    let mut sets = vec![Vec::new(), symbols.to_vec()];
    let mut size = 1;
    while size < symbols.len() && sets.len() < MAX_DERIVED_SETS {
        let mut chosen: Vec<usize> = (0..size).collect();
        loop {
            sets.push(chosen.iter().map(|i| symbols[*i].clone()).collect());
            // The next combination of `size` indices, in lexicographic order.
            let Some(i) = (0..size)
                .rev()
                .find(|i| chosen[*i] < symbols.len() - size + i)
            else {
                break;
            };
            chosen[i] += 1;
            for j in i + 1..size {
                chosen[j] = chosen[j - 1] + 1;
            }
            if sets.len() >= MAX_DERIVED_SETS {
                break;
            }
        }
        size += 1;
    }
    sets.truncate(MAX_DERIVED_SETS);
    sets
}

/// A parse that supplies the text of some branches.
struct Source {
    symbols: Vec<String>,
    /// The branches it supplies.
    takes: Vec<bool>,
    output: String,
}

pub(super) fn merge(
    tree: &SyntaxNode,
    output: &str,
    options: &ResolvedOptions,
    variants: &mut dyn Variants,
) -> Option<(String, Vec<Vec<String>>)> {
    let explicit = options.string_lists(PREPROCESSOR_SYMBOL_SETS);
    let derive = options.bool(DERIVE_SYMBOL_SETS);
    if (explicit.is_empty() && !derive) || !output.contains('#') {
        return None;
    }
    let base = Shape::of(tree);
    let mut missing = base.dead.clone();
    if !missing.contains(&true) {
        return None;
    }
    let mut candidates = explicit;
    if derive {
        candidates.extend(derived_sets(&condition_symbols(&base.directives)));
    }
    let mut sources: Vec<(Vec<String>, SyntaxNode, Vec<bool>)> = Vec::new();
    for symbols in candidates {
        if !missing.contains(&true) {
            break;
        }
        let Some(parsed) = variants.parse(&symbols) else {
            continue;
        };
        let shape = Shape::of(&parsed);
        if shape.directives != base.directives {
            continue;
        }
        let takes: Vec<bool> = missing
            .iter()
            .zip(&shape.dead)
            .map(|(&missing, &dead)| missing && !dead)
            .collect();
        if takes.contains(&true) {
            for (m, t) in missing.iter_mut().zip(&takes) {
                *m &= !t;
            }
            sources.push((symbols, parsed, takes));
        }
    }
    if sources.is_empty() {
        return None;
    }

    let (base_pieces, base_lines) = split(output);
    if base_lines.len() != base.directives.len() {
        return None;
    }
    let mut formatted: Vec<Source> = Vec::new();
    for (symbols, parsed, takes) in sources {
        let output = variants.format(&parsed)?;
        let (_, lines) = split(&output);
        let same = lines.len() == base_lines.len()
            && lines
                .iter()
                .zip(&base_lines)
                .all(|(a, b)| a.trim() == b.trim());
        if !same {
            return None;
        }
        formatted.push(Source {
            symbols,
            takes,
            output,
        });
    }
    let pieces: Vec<Vec<&str>> = formatted.iter().map(|s| split(&s.output).0).collect();

    let mut merged = String::with_capacity(output.len());
    for (i, base_piece) in base_pieces.iter().enumerate() {
        let from = formatted.iter().position(|s| s.takes[i]);
        merged.push_str(from.map_or(base_piece, |f| pieces[f][i]));
        if let Some(line) = base_lines.get(i) {
            merged.push_str(line);
        }
    }
    Some((merged, formatted.into_iter().map(|s| s.symbols).collect()))
}
