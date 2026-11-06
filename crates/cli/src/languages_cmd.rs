//! `sudsy languages list|add|remove|sync`: the installed set (`docs/architecture/language-packs.md`).

use anyhow::{Result, anyhow, bail};
use clap::Subcommand;
use core_config::Need;
use languages::{Project, installed, providers};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Subcommand)]
pub enum LanguagesCommand {
    /// Show every language: installed or available, and the size of its pack.
    List,
    /// Install languages (downloads their packs from the release of this version).
    Add {
        languages: Vec<String>,
        /// Install every language.
        #[arg(long)]
        all: bool,
        /// Take the packs from this directory instead of downloading them.
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Remove languages; their files are skipped until they are added again.
    Remove { languages: Vec<String> },
    /// Install the languages that `languages:` in sudsy.yaml marks as required.
    Sync,
}

pub fn run(command: LanguagesCommand) -> Result<ExitCode> {
    let providers = providers();
    let known: Vec<&str> = providers.iter().map(|p| p.language()).collect();
    let check = |names: &[String]| -> Result<()> {
        for name in names {
            if !known.contains(&name.as_str()) {
                bail!(core_config::unknown_name(
                    "language",
                    name,
                    known.iter().copied()
                ));
            }
        }
        Ok(())
    };
    let add = |names: &[String], from: Option<&PathBuf>| -> Result<()> {
        for name in names {
            installed::add(name, from.map(PathBuf::as_path)).map_err(|e| anyhow!("{name}: {e}"))?;
            eprintln!("installed {name}");
        }
        Ok(())
    };
    match command {
        LanguagesCommand::List => {
            for row in installed::list(&providers) {
                let state = if row.installed {
                    "installed"
                } else {
                    "available"
                };
                println!("{:<14}{state:<11}{}", row.language, row.size);
            }
        }
        LanguagesCommand::Add {
            languages,
            all,
            from,
        } => {
            let names = if all {
                known.iter().map(|l| l.to_string()).collect()
            } else {
                languages
            };
            check(&names)?;
            add(&names, from.as_ref())?;
        }
        LanguagesCommand::Remove { languages } => {
            check(&languages)?;
            for name in &languages {
                installed::remove(name).map_err(|e| anyhow!("{name}: {e}"))?;
                eprintln!("removed {name}");
            }
        }
        LanguagesCommand::Sync => {
            let project = Project::load(&std::env::current_dir()?, &providers)?;
            let wanted: Vec<String> = project
                .chain
                .needs()
                .into_iter()
                .filter(|(name, need)| *need == Need::Required && installed::check(name).is_err())
                .map(|(name, _)| name.to_string())
                .collect();
            add(&wanted, None)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}
