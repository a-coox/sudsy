//! Metavariables. `$X` matches one syntax element and `$$$ARGS` any run of siblings (names are
//! upper case; a name starting with `_` captures nothing). To parse a pattern with the
//! language's own parser each metavariable is first written as an identifier, or as a string
//! when the language has no bare identifiers in value position (JSON).

const SINGLE: &str = "__SUDSY_S_";
const MULTI: &str = "__SUDSY_M_";
const END: &str = "__";

/// How a metavariable is written in the snippet handed to the parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Identifier,
    String,
}

/// One metavariable found in text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub multi: bool,
    /// Upper case; digits only for a group of a `regex` (`$1`).
    pub name: String,
    /// `${NAME:transform}` in a template (see [`transform`]).
    pub transform: Option<String>,
}

impl Meta {
    /// Does the name take part in captures and equality? `_` and `_X` do not.
    pub fn captures(&self) -> bool {
        !self.name.starts_with('_')
    }
}

/// Rewrites every metavariable of a pattern with `f(meta)`; everything else is copied.
pub fn rewrite(text: &str, f: impl FnMut(&Meta) -> String) -> String {
    rewrite_in(text, false, f)
}

/// [`rewrite`] for a message, fix or regex template, which also has `$1` (a group of a `regex`),
/// `${NAME}` and `${NAME:transform}`; a pattern is code, where those are the code's own.
pub fn template(text: &str, f: impl FnMut(&Meta) -> String) -> String {
    rewrite_in(text, true, f)
}

/// `${NAME}` or `${NAME:transform}` at the start of `after` (past the `$`), with its length.
fn braced(after: &str) -> Option<(Meta, usize)> {
    let inner = &after.strip_prefix('{')?[..after.find('}')? - 1];
    let (name, transform) = match inner.split_once(':') {
        Some((n, t)) => (n, Some(t.to_string())),
        None => (inner, None),
    };
    let valid = name
        .bytes()
        .all(|b| b == b'_' || b.is_ascii_uppercase() || b.is_ascii_digit());
    (valid && !name.is_empty()).then(|| {
        let meta = Meta {
            multi: false,
            name: name.to_string(),
            transform,
        };
        (meta, inner.len() + 2)
    })
}

fn rewrite_in(text: &str, templates: bool, mut f: impl FnMut(&Meta) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let dollars = rest.bytes().take_while(|&b| b == b'$').count();
        let after = &rest[dollars..];
        if templates && dollars == 1 {
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            let group = (digits > 0).then(|| {
                let name = after[..digits].to_string();
                (
                    Meta {
                        multi: false,
                        name,
                        transform: None,
                    },
                    digits,
                )
            });
            if let Some((meta, len)) = braced(after).or(group) {
                out.push_str(&f(&meta));
                rest = &after[len..];
                continue;
            }
        }
        let name_len = after
            .bytes()
            .enumerate()
            .take_while(|&(i, b)| {
                b == b'_' || b.is_ascii_uppercase() || (i > 0 && b.is_ascii_digit())
            })
            .count();
        if (dollars == 1 || dollars == 3) && name_len > 0 {
            let meta = Meta {
                multi: dollars == 3,
                name: after[..name_len].to_string(),
                transform: None,
            };
            out.push_str(&f(&meta));
            rest = &after[name_len..];
        } else {
            out.push_str(&rest[..dollars]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// The names of the metavariables in `text`, a pattern or (`templates`) a message, fix or regex.
pub fn names(text: &str, templates: bool) -> Vec<Meta> {
    let mut found = Vec::new();
    if !text.contains('$') {
        return found;
    }
    rewrite_in(text, templates, |m| {
        found.push(m.clone());
        String::new()
    });
    found
}

/// What a template transform makes of captured text; `None` for a transform that does not exist.
/// `upper-snake` is buf's `ToUpperSnakeCase`: a word starts at a capital after a lower case letter
/// or digit, and at the last capital of a run that a lower case letter follows (`HTTPRequest` is
/// `HTTP_REQUEST`).
pub fn transform(name: &str, text: &str) -> Option<String> {
    if name != "upper-snake" {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let before = chars[i - 1];
            let after = chars.get(i + 1);
            if before.is_lowercase()
                || before.is_ascii_digit()
                || (before.is_uppercase() && after.is_some_and(|a| a.is_lowercase()))
            {
                out.push('_');
            }
        }
        out.extend(c.to_uppercase());
    }
    Some(out)
}

/// `text` with every metavariable written for the parser.
pub fn encode(text: &str, style: Style) -> String {
    rewrite(text, |m| {
        let id = format!("{}{}{END}", if m.multi { MULTI } else { SINGLE }, m.name);
        match style {
            Style::Identifier => id,
            Style::String => format!("\"{id}\""),
        }
    })
}

/// The metavariable that `text` (the significant tokens of a pattern element, joined) is: an
/// encoded metavariable, in a statement position followed by its `;`.
pub fn decode(text: &str) -> Option<Meta> {
    let text = text.strip_suffix(';').unwrap_or(text);
    let text = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text);
    let (multi, rest) = match text.strip_prefix(MULTI) {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix(SINGLE)?),
    };
    let name = rest.strip_suffix(END)?;
    // Not the joined tokens of `$A.$B`, which also start and end like one placeholder.
    let one = name
        .bytes()
        .all(|b| b == b'_' || b.is_ascii_uppercase() || b.is_ascii_digit());
    (!name.is_empty() && one).then(|| Meta {
        multi,
        name: name.to_string(),
        transform: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes() {
        let e = encode("f($A, $$$REST)", Style::Identifier);
        assert_eq!(e, "f(__SUDSY_S_A__, __SUDSY_M_REST__)");
        assert_eq!(
            decode("__SUDSY_M_REST__"),
            Some(Meta {
                multi: true,
                name: "REST".into(),
                transform: None
            })
        );
        assert_eq!(decode("\"__SUDSY_S_A__\"").unwrap().name, "A");
        assert_eq!(decode("__SUDSY_S_A__;").unwrap().name, "A");
        assert_eq!(decode("plain"), None);
    }

    #[test]
    fn leaves_other_dollars_alone() {
        assert_eq!(
            encode("$\"hi {x}\" $$ $1 $$$$A", Style::Identifier),
            "$\"hi {x}\" $$ $1 $$$$A"
        );
        assert_eq!(names("$A $_ $$$B_2", false).len(), 3);
    }

    #[test]
    fn templates_have_groups_braces_and_transforms() {
        let f = |m: &Meta| format!("<{}:{}>", m.name, m.transform.as_deref().unwrap_or(""));
        assert_eq!(
            template("[$1] ${A}x $B ${C:upper-snake}_ ${x}", f),
            "[<1:>] <A:>x <B:> <C:upper-snake>_ ${x}"
        );
        assert_eq!(rewrite("[$1] ${A}", |m| m.name.clone()), "[$1] ${A}");
        assert_eq!(
            transform("upper-snake", "HTTPRequest2Foo").unwrap(),
            "HTTP_REQUEST2_FOO"
        );
    }

    #[test]
    fn rewrites_with_a_function() {
        assert_eq!(rewrite("x($A,$$$B)", |m| m.name.to_lowercase()), "x(a,b)");
    }
}
