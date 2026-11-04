//! Rule options: declared per rule, stored as data in the config, checked here.

use core_config::OptionValue;
use std::collections::BTreeMap;

/// A declared default; its variant is the option's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opt {
    Bool(bool),
    Int(i64),
    Str(&'static str),
    List(&'static [&'static str]),
}

impl Opt {
    pub fn type_name(self) -> &'static str {
        match self {
            Opt::Bool(_) => "a boolean",
            Opt::Int(_) => "an integer",
            Opt::Str(_) => "a string",
            Opt::List(_) => "a list of strings",
        }
    }

    /// The default as written in a config.
    pub fn display(self) -> String {
        let quote = |s: &str| format!("\"{s}\"");
        match self {
            Opt::Bool(b) => b.to_string(),
            Opt::Int(i) => i.to_string(),
            Opt::Str(s) => quote(s),
            Opt::List(l) => format!(
                "[{}]",
                l.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    pub fn value(self) -> OptionValue {
        match self {
            Opt::Bool(b) => OptionValue::Bool(b),
            Opt::Int(i) => OptionValue::Int(i),
            Opt::Str(s) => OptionValue::String(s.into()),
            Opt::List(l) => {
                OptionValue::List(l.iter().map(|s| OptionValue::String((*s).into())).collect())
            }
        }
    }

    fn accepts(self, v: &OptionValue) -> bool {
        match (self, v) {
            (Opt::Bool(_), OptionValue::Bool(_))
            | (Opt::Int(_), OptionValue::Int(_))
            | (Opt::Str(_), OptionValue::String(_)) => true,
            (Opt::List(_), OptionValue::List(items)) => {
                items.iter().all(|i| matches!(i, OptionValue::String(_)))
            }
            _ => false,
        }
    }
}

/// What a string option (or each item of a list option) must satisfy besides being a string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Any,
    /// One of these values; validation and the JSON Schema both come from this list.
    OneOf(&'static [&'static str]),
    /// A regular expression that compiles.
    Regex,
}

impl Check {
    fn run(self, v: &str) -> Result<(), String> {
        match self {
            Check::Any => Ok(()),
            Check::OneOf(values) if values.contains(&v) => Ok(()),
            Check::OneOf(values) => Err(format!(
                "must be one of {}, not `{v}`",
                values
                    .iter()
                    .map(|s| format!("`{s}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Check::Regex => regex::Regex::new(v)
                .map(drop)
                .map_err(|e| format!("has an invalid regular expression `{v}`: {e}")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OptionDecl {
    pub name: &'static str,
    pub default: Opt,
    pub check: Check,
    pub description: &'static str,
}

/// A rule's options after checking: every declared option has a value.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RuleOptions {
    values: BTreeMap<String, OptionValue>,
}

impl RuleOptions {
    /// The declared defaults overridden by the configured values. Unknown names and values of the
    /// wrong type are errors.
    pub fn resolve(
        decls: &[OptionDecl],
        configured: &BTreeMap<String, OptionValue>,
    ) -> Result<RuleOptions, String> {
        let mut values: BTreeMap<String, OptionValue> = decls
            .iter()
            .map(|d| (d.name.to_string(), d.default.value()))
            .collect();
        for (name, value) in configured {
            let Some(decl) = decls.iter().find(|d| d.name == name) else {
                let known: Vec<&str> = decls.iter().map(|d| d.name).collect();
                return Err(core_config::unknown_name("option", name, known));
            };
            if !decl.default.accepts(value) {
                return Err(format!(
                    "option `{name}` must be {}",
                    decl.default.type_name()
                ));
            }
            let texts: Vec<&str> = match value {
                OptionValue::String(s) => vec![s],
                OptionValue::List(items) => items
                    .iter()
                    .filter_map(|i| match i {
                        OptionValue::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            for t in texts {
                decl.check
                    .run(t)
                    .map_err(|e| format!("option `{name}` {e}"))?;
            }
            values.insert(name.clone(), value.clone());
        }
        Ok(RuleOptions { values })
    }

    pub fn bool(&self, name: &str) -> bool {
        matches!(self.values.get(name), Some(OptionValue::Bool(true)))
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.values.get(name) {
            Some(OptionValue::Int(i)) => *i,
            _ => 0,
        }
    }

    pub fn string(&self, name: &str) -> &str {
        match self.values.get(name) {
            Some(OptionValue::String(s)) => s,
            _ => "",
        }
    }

    pub fn list(&self, name: &str) -> Vec<&str> {
        match self.values.get(name) {
            Some(OptionValue::List(items)) => items
                .iter()
                .filter_map(|i| match i {
                    OptionValue::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLS: [OptionDecl; 2] = [
        OptionDecl {
            name: "mode",
            default: Opt::Str("a"),
            check: Check::OneOf(&["a", "b"]),
            description: "",
        },
        OptionDecl {
            name: "patterns",
            default: Opt::List(&[]),
            check: Check::Regex,
            description: "",
        },
    ];

    fn resolve(name: &str, v: OptionValue) -> Result<RuleOptions, String> {
        RuleOptions::resolve(&DECLS, &BTreeMap::from([(name.to_string(), v)]))
    }

    fn s(v: &str) -> OptionValue {
        OptionValue::String(v.into())
    }

    #[test]
    fn enum_and_regex_options_are_validated() {
        assert!(resolve("mode", s("b")).is_ok());
        let e = resolve("mode", s("c")).unwrap_err();
        assert_eq!(e, "option `mode` must be one of `a`, `b`, not `c`");
        assert!(resolve("patterns", OptionValue::List(vec![s("^x")])).is_ok());
        let e = resolve("patterns", OptionValue::List(vec![s("^x"), s("(")])).unwrap_err();
        assert!(
            e.starts_with("option `patterns` has an invalid regular expression `(`"),
            "{e}"
        );
    }
}
