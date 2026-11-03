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
    pub name: String,
}

impl Meta {
    /// Does the name take part in captures and equality? `_` and `_X` do not.
    pub fn captures(&self) -> bool {
        !self.name.starts_with('_')
    }
}

/// Rewrites every metavariable of `text` with `f(meta)`; everything else is copied.
pub fn rewrite(text: &str, mut f: impl FnMut(&Meta) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let dollars = rest.bytes().take_while(|&b| b == b'$').count();
        let after = &rest[dollars..];
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

/// The names of the metavariables in `text`.
pub fn names(text: &str) -> Vec<Meta> {
    let mut found = Vec::new();
    rewrite(text, |m| {
        found.push(m.clone());
        String::new()
    });
    found
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
    (!name.is_empty()).then(|| Meta {
        multi,
        name: name.to_string(),
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
                name: "REST".into()
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
        assert_eq!(names("$A $_ $$$B_2").len(), 3);
    }

    #[test]
    fn rewrites_with_a_function() {
        assert_eq!(rewrite("x($A,$$$B)", |m| m.name.to_lowercase()), "x(a,b)");
    }
}
