use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["codegen"] => {
            xtask::write(xtask_full::generate()?)?;
            Ok(())
        }
        ["codegen", "--check"] => {
            let stale = xtask::stale(xtask_full::generate()?);
            if stale.is_empty() {
                return Ok(());
            }
            bail!("stale generated files (run `cargo xtask codegen`): {stale:?}");
        }
        ["affected", rest @ ..] => affected(rest),
        _ => bail!("usage: xtask-full codegen [--check] | affected [--base REV] [--all]"),
    }
}

fn affected(args: &[&str]) -> Result<()> {
    let (base, all) = match args {
        [] => (None, false),
        ["--all"] => (None, true),
        ["--base", rev] => (Some(*rev), false),
        ["--base", rev, "--all"] | ["--all", "--base", rev] => (Some(*rev), true),
        _ => bail!("usage: cargo xtask affected [--base REV] [--all]"),
    };
    print!(
        "{}",
        xtask_full::affected::run(&xtask::repo_root(), base, all)?
    );
    Ok(())
}
