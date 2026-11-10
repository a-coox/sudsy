use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["codegen"] => codegen(),
        ["codegen", "--check"] => {
            let stale = xtask::stale_files()?;
            if stale.is_empty() {
                return Ok(());
            }
            bail!("stale generated files (run `cargo xtask codegen`): {stale:?}");
        }
        ["affected", rest @ ..] => affected(rest),
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

/// This binary embeds the providers, whose glue `codegen` rewrites (`provider.generated.rs`), so
/// outputs derived from them (the VS Code selector, docs) are stale after a change. A run that
/// rewrote crates/ or the root manifest re-runs once in a fresh build, which must not change crates/ again.
fn codegen() -> Result<()> {
    const PASS: &str = "SUDSY_CODEGEN_PASS2";
    let changed = xtask::write_generated()?;
    let second = std::env::var_os(PASS).is_some();
    let rebuilds = changed
        .iter()
        .any(|p| p.starts_with("crates") || p == "Cargo.toml");
    if second && rebuilds {
        bail!("codegen did not converge: the second run changed {changed:?}");
    }
    if !second && rebuilds {
        // Windows will not let cargo replace the running exe, but it allows renaming it.
        let exe = std::env::current_exe()?;
        std::fs::rename(&exe, exe.with_extension("old"))?;
        let status = std::process::Command::new("cargo")
            .args(["run", "--quiet", "--package", "xtask", "--", "codegen"])
            .env(PASS, "1")
            .status()?;
        anyhow::ensure!(status.success(), "second codegen pass failed");
    }
    Ok(())
}

fn affected(args: &[&str]) -> Result<()> {
    let (base, all) = match args {
        [] => (None, false),
        ["--all"] => (None, true),
        ["--base", rev] => (Some(*rev), false),
        ["--base", rev, "--all"] | ["--all", "--base", rev] => (Some(*rev), true),
        _ => bail!("usage: cargo xtask affected [--base REV] [--all]"),
    };
    print!("{}", xtask::affected::run(&xtask::repo_root(), base, all)?);
    Ok(())
}
