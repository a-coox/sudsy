//! `sudsy init`: writes a starter `sudsy.yaml`, or with `--from <tool>` the equivalent of another
//! tool's config in the working directory (docs/config/cli.md).

mod biome;
mod csharpier;
mod editorconfig;
mod eslint;
mod out;
mod prettier;
mod sources;
mod stylecop;
mod stylelint;
mod yamllint;

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};
use languages::{providers, schemas};
use out::{Out, Report};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, ValueEnum)]
enum Tool {
    Yamllint,
    Prettier,
    Biome,
    EslintJsonc,
    Stylecop,
    Stylelint,
    Csharpier,
}

#[derive(Args)]
pub struct InitArgs {
    /// A preset to extend, e.g. `csharp/google`; repeat for several languages. Without it the
    /// starter extends the default preset of each language.
    #[arg(long = "preset", conflicts_with = "from")]
    presets: Vec<String>,
    /// Migrate the config of another tool found in the working directory.
    #[arg(long)]
    from: Option<Tool>,
    /// Replace an existing sudsy config.
    #[arg(long)]
    force: bool,
    /// With `--from stylecop` or `csharpier`: append the `.editorconfig` entries to the file.
    #[arg(long, requires = "from")]
    write_editorconfig: bool,
}

/// The preset each language gets when none is named.
const DEFAULT_PRESETS: [&str; 3] = ["csharp/microsoft", "json/default", "yaml/default"];

pub fn run(args: InitArgs) -> Result<()> {
    let existing = ["sudsy.yaml", "sudsy.yml"]
        .map(PathBuf::from)
        .into_iter()
        .find(|p| p.exists());
    if existing.is_some() && !args.force {
        bail!("a sudsy config already exists here; not overwriting it (use --force to replace it)");
    }
    let path = existing.unwrap_or_else(|| PathBuf::from("sudsy.yaml"));
    let providers = providers();
    let schemas = schemas(&providers);
    let presets: Vec<(&str, &str)> = schemas
        .iter()
        .flat_map(|(_, s)| s.presets().iter().map(|p| (p.name.as_str(), p.text)))
        .collect();
    let Some(tool) = args.from else {
        let text = starter(args.presets, &presets)?;
        std::fs::write(&path, text)?;
        eprintln!("wrote {}", path.display());
        return Ok(());
    };
    if args.write_editorconfig && !matches!(tool, Tool::Stylecop | Tool::Csharpier) {
        bail!("--write-editorconfig applies to --from stylecop and --from csharpier");
    }
    let (out, source) = migrate(tool)?;
    let name = tool
        .to_possible_value()
        .expect("named")
        .get_name()
        .to_string();
    let (text, report) = out.render(&name, &presets);
    std::fs::write(&path, text)?;
    eprintln!("wrote {}", path.display());
    print_report(&source, &report);
    if args.write_editorconfig {
        let added = editorconfig::append(&report.editorconfig)?;
        println!(
            "{}",
            if added {
                "appended to .editorconfig"
            } else {
                ".editorconfig already has the entries"
            }
        );
    }
    Ok(())
}

fn starter(presets: Vec<String>, known: &[(&str, &str)]) -> Result<String> {
    let presets: Vec<String> = if presets.is_empty() {
        DEFAULT_PRESETS.iter().map(|p| (*p).to_string()).collect()
    } else {
        presets
    };
    let mut text = String::from("# https://github.com/a-coox/sudsy\nextends:\n");
    for preset in &presets {
        if !known.iter().any(|(name, _)| name == preset) {
            let names: Vec<&str> = known.iter().map(|(n, _)| *n).collect();
            bail!(
                "unknown preset `{preset}`; known presets: {}",
                names.join(", ")
            );
        }
        text.push_str(&format!("  - preset:{preset}\n"));
    }
    Ok(text)
}

/// Reads the tool's config and maps it. Returns what was read, for the report.
fn migrate(tool: Tool) -> Result<(Out, String)> {
    let found = |what: &str, names: &[&str]| {
        format!(
            "no {what} config found here (looked for {})",
            names.join(", ")
        )
    };
    match tool {
        Tool::Yamllint => {
            let path = sources::find(&yamllint::FILES)
                .with_context(|| found("yamllint", &yamllint::FILES))?;
            let mut out = Out::new("yamllint");
            yamllint::migrate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
        Tool::Prettier => {
            let mut out = Out::new("prettier");
            let (config, source) = match sources::find(&prettier::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("prettier")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => {
                        let other = sources::find(&prettier::UNSUPPORTED);
                        bail!(
                            "{}",
                            match other {
                                Some(p) => format!(
                                    "{} is not JSON or YAML; only .prettierrc, .prettierrc.json/.yaml/.yml and package.json \"prettier\" are read",
                                    p.display()
                                ),
                                None => found("Prettier", &prettier::FILES),
                            }
                        )
                    }
                },
            };
            prettier::migrate(&config, &mut out).with_context(|| source.clone())?;
            Ok((out, source))
        }
        Tool::Biome => {
            let path =
                sources::find(&biome::FILES).with_context(|| found("Biome", &biome::FILES))?;
            let mut out = Out::new("biome");
            biome::migrate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
        Tool::EslintJsonc => {
            let mut out = Out::new("eslint-plugin-jsonc");
            let (config, source) = match sources::find(&eslint::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("eslintConfig")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => match sources::find(&eslint::JS_CONFIGS) {
                        Some(p) => bail!(
                            "{} is JavaScript and cannot be read without running it; only JSON/YAML legacy configs (.eslintrc.json/.yaml/.yml, package.json \"eslintConfig\") are supported. Write the options by hand: docs/parity/eslint-plugin-jsonc.yaml maps each rule and option",
                            p.display()
                        ),
                        None => bail!("{}", found("ESLint", &eslint::FILES)),
                    },
                },
            };
            if let Some(js) = sources::find(&eslint::JS_CONFIGS) {
                out.note(format!("{} is JavaScript and was not read", js.display()));
            }
            eslint::migrate(&config, &mut out).with_context(|| source.clone())?;
            Ok((out, source))
        }
        Tool::Stylelint => {
            let mut out = Out::new("stylelint");
            let (config, source) = match sources::find(&stylelint::FILES) {
                Some(path) => (sources::parse(&path)?, path.display().to_string()),
                None => match sources::package_json_key("stylelint")? {
                    Some(v) => (v, "package.json".to_string()),
                    None => match sources::find(&stylelint::JS_CONFIGS) {
                        Some(p) => bail!(
                            "{} is JavaScript and cannot be read without running it; only JSON/YAML configs (.stylelintrc, .stylelintrc.json/.yaml/.yml, package.json \"stylelint\") are supported. Write the rules by hand: docs/parity/stylelint.yaml maps each rule",
                            p.display()
                        ),
                        None => bail!("{}", found("stylelint", &stylelint::FILES)),
                    },
                },
            };
            if let Some(js) = sources::find(&stylelint::JS_CONFIGS) {
                out.note(format!("{} is JavaScript and was not read", js.display()));
            }
            stylelint::migrate(&config, &mut out).with_context(|| source.clone())?;
            if Path::new(stylelint::IGNORE_FILE).is_file() {
                let text = sources::read(Path::new(stylelint::IGNORE_FILE))?;
                stylelint::ignore_file(&text, &mut out);
            }
            Ok((out, source))
        }
        Tool::Stylecop => {
            let file = sources::find(&stylecop::FILES);
            let settings = file.as_deref().map(sources::parse).transpose()?;
            if settings.is_none() && editorconfig::sa_severities()?.is_empty() {
                bail!(
                    "{}; and no dotnet_diagnostic.SA*.severity in .editorconfig",
                    found("StyleCop", &stylecop::FILES)
                );
            }
            let mut out = Out::new("stylecop");
            stylecop::migrate(settings.as_ref(), &mut out)?;
            let source = file
                .as_deref()
                .map_or(".editorconfig".into(), |p: &Path| p.display().to_string());
            Ok((out, source))
        }
        Tool::Csharpier => {
            let path = sources::find(&csharpier::FILES)
                .with_context(|| found("CSharpier", &csharpier::FILES))?;
            let mut out = Out::new("csharpier");
            csharpier::migrate(&sources::parse(&path)?, &mut out)
                .with_context(|| path.display().to_string())?;
            Ok((out, path.display().to_string()))
        }
    }
}

fn print_report(source: &str, report: &Report) {
    println!("migrated {source}");
    for preset in &report.extends {
        println!("extends: preset:{preset}");
    }
    for item in &report.not_migrated {
        println!("not migrated: {item}");
    }
    for note in &report.notes {
        println!("note: {note}");
    }
    if !report.editorconfig.is_empty() {
        println!("\nAdd to .editorconfig (or run again with --write-editorconfig):\n");
        println!("{}", editorconfig::block(&report.editorconfig));
        println!(
            "note: set EnforceCodeStyleInBuild to true (Directory.Build.props) so the SDK reports the other IDE* style rules during `dotnet build`"
        );
    }
}
