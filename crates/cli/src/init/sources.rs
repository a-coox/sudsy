//! Reading another tool's config file. These files come from the repository, so they are
//! size-capped like Sudsy's own config and every failure is a clean error.

use anyhow::{Context, Result, anyhow, bail};
use core_config::{MAX_CONFIG_BYTES, read_capped};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The first of `names` that is a file in the working directory.
pub fn find(names: &[&str]) -> Option<PathBuf> {
    names.iter().map(PathBuf::from).find(|p| p.is_file())
}

pub fn read(path: &Path) -> Result<String> {
    let bytes =
        read_capped(path, MAX_CONFIG_BYTES).map_err(|e| anyhow!("{}: {e}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| anyhow!("{}: not valid UTF-8", path.display()))
}

/// JSON with comments and trailing commas, or YAML, chosen by the extension or first character.
pub fn parse(path: &Path) -> Result<Value> {
    let text = read(path)?;
    let text = text.trim_start_matches('\u{feff}');
    let is_json = path
        .extension()
        .is_some_and(|e| e == "json" || e == "jsonc")
        || text.trim_start().starts_with('{');
    let value = if is_json {
        serde_json::from_str(&strip_jsonc(text)).map_err(|e| anyhow!("{e}"))
    } else {
        serde_saphyr::from_str(text).map_err(|e| anyhow!("{e}"))
    };
    value.with_context(|| format!("{}: cannot parse", path.display()))
}

/// The value at `key` of the object in a `package.json`, or `None` when absent.
pub fn package_json_key(key: &str) -> Result<Option<Value>> {
    let path = Path::new("package.json");
    if !path.is_file() {
        return Ok(None);
    }
    match parse(path)? {
        Value::Object(mut map) => Ok(map.remove(key)),
        _ => bail!("package.json: not an object"),
    }
}

/// Removes `//` and `/* */` comments and commas before `}` or `]`, leaving strings alone.
fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut in_str) = (0, false);
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if in_str {
            out.push(c);
            if c == '\\' {
                out.extend(next);
                i += 1;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        } else if (c == '}' || c == ']') && out.trim_end().ends_with(',') {
            let kept = out.trim_end().trim_end_matches(',').len();
            out.truncate(kept);
            out.push(c);
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::strip_jsonc;

    #[test]
    fn strips_comments_and_trailing_commas_outside_strings() {
        let text = "{ // c\n \"a\": \"x // y,}\", /* b */ \"b\": [1, 2,], }";
        let value: serde_json::Value = serde_json::from_str(&strip_jsonc(text)).unwrap();
        assert_eq!(value["a"], "x // y,}");
        assert_eq!(value["b"], serde_json::json!([1, 2]));
    }
}
