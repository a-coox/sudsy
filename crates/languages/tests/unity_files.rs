//! Unity's text assets map to their language; its serialized YAML (`.unity`, `.prefab`, `.meta`, ...
//! starts with a `%TAG` directive and `!u!` tags, not plain YAML) is claimed by no provider.

use std::path::Path;

fn language_of(file: &str) -> Option<String> {
    languages::providers()
        .iter()
        .find(|p| p.file_matchers().iter().any(|m| m.matches(Path::new(file))))
        .map(|p| p.language().to_string())
}

#[test]
fn unity_text_assets_have_a_language_and_serialized_yaml_has_none() {
    for (file, language) in [
        ("Assets/Game.asmdef", "json"),
        ("Assets/Game.asmref", "json"),
        ("Assets/Controls.inputactions", "json"),
        ("Assets/UI/Main.uss", "css"),
        ("Assets/UI/Main.uxml", "xml"),
    ] {
        assert_eq!(language_of(file).as_deref(), Some(language), "{file}");
    }
    for ext in [
        "unity",
        "prefab",
        "asset",
        "meta",
        "mat",
        "anim",
        "controller",
        "physicMaterial",
        "overrideController",
        "mask",
        "flare",
        "cubemap",
        "lighting",
        "playable",
    ] {
        assert_eq!(language_of(&format!("Assets/x.{ext}")), None, ".{ext}");
    }
}
