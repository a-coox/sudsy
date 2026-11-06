//! The JSON Schema of `sudsy.yaml`, generated from the providers' option declarations
//! Editors register it for completion and validation of the config.

use crate::model::{Depth, OptionValue};
use crate::schema::{OptionDef, OptionKind, OptionSchema, shared};
use serde_json::{Map, Value, json};

fn value_json(v: &OptionValue) -> Value {
    match v {
        OptionValue::Bool(b) => json!(b),
        OptionValue::Int(n) => json!(n),
        OptionValue::Float(f) => json!(f),
        OptionValue::String(s) => json!(s),
        OptionValue::List(items) => Value::Array(items.iter().map(value_json).collect()),
        OptionValue::Map(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), value_json(v)))
                .collect::<Map<_, _>>(),
        ),
    }
}

fn option_json(def: &OptionDef) -> Value {
    let mut out = match &def.kind {
        OptionKind::Bool => json!({ "type": "boolean" }),
        OptionKind::Int { min, max } => {
            json!({ "type": "integer", "minimum": min, "maximum": max })
        }
        OptionKind::Enum(values) => json!({ "type": "string", "enum": values }),
        OptionKind::Words => json!({ "type": "array", "items": { "type": "string" } }),
        OptionKind::WordSets => json!({
            "type": "array",
            "items": { "type": "array", "items": { "type": "string" } }
        }),
    };
    out["description"] = json!(def.description);
    out["default"] = value_json(&def.default);
    out
}

fn options_object(defs: Vec<&OptionDef>) -> Value {
    let properties: Map<String, Value> = defs
        .into_iter()
        .map(|d| (d.name.to_string(), option_json(d)))
        .collect();
    json!({ "type": "object", "additionalProperties": false, "properties": properties })
}

/// A rule as the config schema needs it: its id and the options it accepts.
#[derive(Clone, Debug)]
pub struct RuleSchema {
    pub id: String,
    pub description: String,
    pub options: Vec<RuleOptionSchema>,
}

/// One option of a rule; `default` decides the type (`Bool`, `Int`, `String` or a list of strings).
#[derive(Clone, Debug)]
pub struct RuleOptionSchema {
    pub name: String,
    pub description: String,
    pub default: OptionValue,
    /// The only accepted values (of a string, or of each item of a list); empty when any is.
    pub choices: Vec<String>,
}

fn rule_option_json(option: &RuleOptionSchema) -> Value {
    let mut out = match &option.default {
        OptionValue::Bool(_) => json!({ "type": "boolean" }),
        OptionValue::Int(_) => json!({ "type": "integer" }),
        OptionValue::List(_) => json!({ "type": "array", "items": { "type": "string" } }),
        _ => json!({ "type": "string" }),
    };
    if !option.choices.is_empty() {
        let target = if out["type"] == "array" {
            &mut out["items"]
        } else {
            &mut out
        };
        target["enum"] = json!(option.choices);
    }
    out["description"] = json!(option.description);
    out["default"] = value_json(&option.default);
    out
}

/// One rule's entry under `lint.rules`: a level, or a level with the rule's own options.
fn rule_json(rule: &RuleSchema) -> Value {
    let mut out = if rule.options.is_empty() {
        json!({ "$ref": "#/definitions/rule" })
    } else {
        let options: Map<String, Value> = rule
            .options
            .iter()
            .map(|o| (o.name.clone(), rule_option_json(o)))
            .collect();
        json!({
            "oneOf": [
                { "$ref": "#/definitions/level" },
                {
                    "type": "object",
                    "required": ["level"],
                    "additionalProperties": false,
                    "properties": {
                        "level": { "$ref": "#/definitions/level" },
                        "options": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": options
                        }
                    }
                }
            ]
        })
    };
    out["description"] = json!(rule.description);
    out
}

/// The `lint` section: the known rules by id (with their options); any other id (`plugin/rule`)
/// takes a level or a level with untyped options.
fn lint_definition(rules: &[RuleSchema]) -> Value {
    let properties: Map<String, Value> =
        rules.iter().map(|r| (r.id.clone(), rule_json(r))).collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "rules": {
                "type": "object",
                "description": "Rule id (`no-empty-catch`, or `plugin/rule`) to a level or a level with options.",
                "properties": properties,
                "additionalProperties": { "$ref": "#/definitions/rule" }
            }
        }
    })
}

fn semantics_section() -> Value {
    let names = json!({
        "type": "array",
        "items": { "type": "string" },
        "description": "Qualified method names without parameter lists; every overload is covered."
    });
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": "Facts about methods that the flow analyses use.",
        "properties": {
            "does_not_return": names.clone(),
            "pure": names
        }
    })
}

fn lint_section() -> Value {
    json!({ "$ref": "#/definitions/lint" })
}

/// The shared options: the first declaration of each name among the languages.
fn shared_defs<'a>(languages: &[(&str, &'a OptionSchema)]) -> Vec<&'a OptionDef> {
    shared::NAMES
        .iter()
        .filter_map(|name| languages.iter().find_map(|(_, s)| s.get(name)))
        .collect()
}

/// The schema for the given languages (`(language, schema)`); languages without a schema get
/// sections that only accept `lint`.
pub fn json_schema(languages: &[(&str, &OptionSchema)], rules: &[RuleSchema]) -> Value {
    let mut presets: Vec<String> = languages
        .iter()
        .flat_map(|(_, s)| s.presets().iter().map(|p| format!("preset:{}", p.name)))
        .collect();
    presets.sort();

    let language_section = |name: &str| -> Value {
        let format = match languages.iter().find(|(l, _)| *l == name) {
            Some((_, schema)) => options_object(schema.defs().iter().collect()),
            None => json!({ "type": "object" }),
        };
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": { "format": format, "lint": lint_section(), "semantics": semantics_section() }
        })
    };
    let sections: Map<String, Value> = languages
        .iter()
        .map(|(l, _)| ((*l).to_string(), language_section(l)))
        .collect();

    let mut override_properties = Map::new();
    override_properties.insert(
        "files".into(),
        json!({
            "type": "array",
            "minItems": 1,
            "items": { "type": "string" },
            "description": "Globs relative to the config file; the override applies to files that match any."
        }),
    );
    override_properties.insert("format".into(), options_object(shared_defs(languages)));
    override_properties.insert("lint".into(), lint_section());
    override_properties.extend(sections.clone());

    let mut properties = Map::new();
    properties.insert(
        "extends".into(),
        json!({
            "type": "array",
            "description": "Presets (`preset:<language>/<name>`) or config files, applied in order.",
            "items": { "type": "string", "examples": presets }
        }),
    );
    for (key, text) in [
        (
            "include",
            "Globs of the files to format and lint (everything when empty).",
        ),
        (
            "exclude",
            "Globs of files to leave alone, in addition to ignore files.",
        ),
    ] {
        properties.insert(
            key.into(),
            json!({ "type": "array", "items": { "type": "string" }, "description": text }),
        );
    }
    properties.insert(
        "analysis".into(),
        json!({
            "type": "string",
            "enum": [Depth::Fast.as_str(), Depth::Standard.as_str(), Depth::Deep.as_str()],
            "default": Depth::Standard.as_str(),
            "description": "Analysis depth for rules: fast (syntax only), standard (within each file), deep (adds project scope)."
        }),
    );
    properties.insert(
        "languages".into(),
        json!({
            "type": "object",
            "propertyNames": { "enum": languages.iter().map(|(l, _)| *l).collect::<Vec<_>>() },
            "additionalProperties": { "enum": ["required", "ignored"] },
            "description": "Languages the project needs: `required` ones are installed by `sudsy languages sync`; files of an `ignored` language are skipped silently when it is not installed."
        }),
    );
    properties.insert("format".into(), options_object(shared_defs(languages)));
    properties.insert("lint".into(), lint_section());
    properties.insert(
        "plugins".into(),
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "patterns": { "type": "array", "items": { "type": "string" } },
                "wasm": { "type": "array", "items": { "type": "string" } }
            }
        }),
    );
    properties.extend(sections);
    properties.insert(
        "overrides".into(),
        json!({
            "type": "array",
            "items": {
                "type": "object",
                "required": ["files"],
                "additionalProperties": false,
                "properties": override_properties
            }
        }),
    );

    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "sudsy.yaml",
        "description": "Configuration of Sudsy, the formatter and linter for source and data files.",
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "definitions": {
            "level": { "type": "string", "enum": ["off", "warn", "error"] },
            "lint": lint_definition(rules),
            "rule": {
                "oneOf": [
                    { "$ref": "#/definitions/level" },
                    {
                        "type": "object",
                        "required": ["level"],
                        "additionalProperties": false,
                        "properties": {
                            "level": { "$ref": "#/definitions/level" },
                            "options": { "type": "object" }
                        }
                    }
                ]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::Preset;

    #[test]
    fn options_and_presets_come_from_the_declarations() {
        let schema = OptionSchema::new(vec![
            shared::line_width(80),
            OptionDef::choice("mode", &["a", "b"], "a", "A mode."),
        ])
        .with_presets(vec![Preset::new("x/p", "")]);
        let doc = json_schema(&[("x", &schema)], &[]);
        assert_eq!(
            doc["properties"]["format"]["properties"]["line_width"]["default"],
            80
        );
        assert_eq!(
            doc["properties"]["extends"]["items"]["examples"][0],
            "preset:x/p"
        );
        assert!(
            doc["properties"]["format"]["properties"]
                .get("mode")
                .is_none()
        );
    }

    #[test]
    fn rule_options_are_declared_with_their_types() {
        let rule = RuleSchema {
            id: "r".into(),
            description: "A rule.".into(),
            options: vec![RuleOptionSchema {
                name: "order".into(),
                description: "The order.".into(),
                default: OptionValue::List(vec![]),
                choices: vec!["a".into(), "b".into()],
            }],
        };
        let doc = json_schema(&[], &[rule]);
        let entry = &doc["definitions"]["lint"]["properties"]["rules"]["properties"]["r"];
        assert_eq!(
            entry["oneOf"][1]["properties"]["options"]["properties"]["order"]["type"],
            "array"
        );
        assert_eq!(entry["description"], "A rule.");
        assert_eq!(
            entry["oneOf"][1]["properties"]["options"]["properties"]["order"]["items"]["enum"],
            json!(["a", "b"])
        );
    }
}
