use anyhow::{Result, bail, ensure};
use std::process::Command;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["codegen"] => {
            xtask::write(xtask::generate()?)?;
            full(&["codegen"])
        }
        ["codegen", "--check"] => {
            // Stage 2 builds the languages, which stale glue would break: report that first.
            let stale = xtask::stale(xtask::generate()?);
            if !stale.is_empty() {
                bail!("stale generated files (run `cargo xtask codegen`): {stale:?}");
            }
            full(&["codegen", "--check"])
        }
        ["affected", rest @ ..] => {
            let mut a = vec!["affected"];
            a.extend(rest);
            full(&a)
        }
        ["new-rule", language, id] => print_new_rule(language, id, false),
        ["new-rule", language, id, "--rust"] => print_new_rule(language, id, true),
        _ => bail!(
            "usage: cargo xtask codegen [--check] | affected [--base REV] [--all] | new-rule <language> <id> [--rust]"
        ),
    }
}

fn print_new_rule(language: &str, id: &str, rust: bool) -> Result<()> {
    println!(
        "{}",
        xtask::new_rule(&xtask::repo_root(), language, id, rust)?
    );
    Ok(())
}

/// Stage 2 and the commands that need the compiled languages: `xtask-full`, built after stage 1.
fn full(args: &[&str]) -> Result<()> {
    let status = Command::new("cargo")
        .args(["run", "--quiet", "--package", "xtask-full", "--"])
        .args(args)
        .status()?;
    ensure!(status.success(), "xtask-full {args:?} failed");
    Ok(())
}
