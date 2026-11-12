//! A language that is not installed: the server offers to install it and serves the open
//! documents once the user chose the action (no download happens before).

use crate::common::{Client, apply};

fn prompts(client: &Client) -> Vec<serde_json::Value> {
    (client.server_request_params.iter())
        .filter(|(m, _)| m == "window/showMessageRequest")
        .map(|(_, p)| p.clone())
        .collect()
}

#[test]
fn a_missing_language_is_offered_once_and_served_after_the_install() {
    let t = tempfile::tempdir().unwrap();
    // The cache directory is process-wide and `set_var` is unsafe (forbidden here): run this
    // test again in a child process that has it set.
    if std::env::var_os("SUDSY_CACHE_DIR").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "languages::a_missing_language_is_offered_once_and_served_after_the_install",
            ])
            .env("SUDSY_CACHE_DIR", t.path().join("cache"))
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    languages::installed::remove("toml").unwrap();

    let mut client = Client::start(t.path(), languages::providers());
    let a = client.open("a.toml", "toml", "a=1\n");
    let b = client.open("b.toml", "toml", "b=1\n");
    // The harness chooses the first action; the server installs, then serves `a` and `b`.
    assert!(client.format(&a).is_empty());
    assert_eq!(apply("b=1\n", &client.format(&b)), "b = 1\n");
    assert_eq!(apply("a=1\n", &client.format(&a)), "a = 1\n");

    // One prompt for the language, though two files of it were opened.
    let prompts = prompts(&client);
    assert_eq!(prompts.len(), 1, "{prompts:?}");
    let messages: Vec<&str> = prompts
        .iter()
        .map(|p| p["message"].as_str().unwrap())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("toml is not installed")),
        "{messages:?}"
    );
    assert_eq!(prompts[0]["actions"][0]["title"], "Install");
    assert!(
        client
            .server_requests
            .iter()
            .any(|m| m == "workspace/diagnostic/refresh")
    );
    assert!(languages::installed::check("toml").is_ok());
}
