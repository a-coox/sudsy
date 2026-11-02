use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["codegen"] => xtask::write_generated(),
        ["codegen", "--check"] => {
            let stale = xtask::stale_files()?;
            if stale.is_empty() {
                return Ok(());
            }
            bail!("stale generated files (run `cargo xtask codegen`): {stale:?}");
        }
        _ => bail!("usage: cargo xtask codegen [--check]"),
    }
}
