//! What every command that works on files shares: the project, the database with its config and
//! the files to process (discovered, filtered by the config's include and exclude).

use anyhow::{Context, Result};
use core_config::{Depth, EditorConfig};
use core_incremental::{Database, File};
use languages::{Project, providers};
use provider_api::SourceFile;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Session {
    pub project: Project,
    pub db: Database,
    /// Absolute paths, in the order they are reported.
    pub paths: Vec<PathBuf>,
}

impl Session {
    /// One database per run: the config is an input, and so is every file, created by the thread
    /// that reads it. `depth` is the command line's `--analysis`, which beats the config.
    pub fn open(args: &[PathBuf], depth: Option<Depth>) -> Result<Session> {
        let providers = providers();
        let mut project = Project::load(&std::env::current_dir()?, &providers)?;
        if let Some(depth) = depth {
            project.chain = project.chain.clone().with_depth(depth);
        }
        let mut db = Database::new(providers);
        db.set_config(Ok(project.chain.clone()));
        // The files semantic rules bind against: what was named on the command line, else the
        // working directory.
        let roots = if args.is_empty() {
            vec![std::env::current_dir()?]
        } else {
            args.to_vec()
        };
        db.open_workspace(&roots);
        let filter = project.filter();
        let paths = core_workspace::discover_files(args, &filter)?
            .into_iter()
            .filter(|path| db.supports(path))
            .collect();
        Ok(Session { project, db, paths })
    }

    /// Reads every file into the database and runs `warm` on it, in parallel; the results of the
    /// queries `warm` runs stay memoized for the caller. Returned in `paths` order.
    pub fn load_all(&self, warm: impl Fn(&Database, File) + Sync) -> Vec<Result<File>> {
        let project = &self.project;
        par_map(&self.db, &self.paths, |db, path| {
            let file = load(db, project, path)?;
            warm(db, file);
            Ok(file)
        })
    }
}

/// Reads one file into the database.
pub fn load(db: &Database, project: &Project, path: &Path) -> Result<File> {
    let bytes = std::fs::read(path).context("reading the file")?;
    let source = SourceFile::from_bytes(project.show(path), 1, bytes)?;
    Ok(File::new(
        db,
        path.to_path_buf(),
        1,
        source.text,
        source.had_bom,
        EditorConfig::for_file(path),
    ))
}

/// Runs `work` on every path on all cores. Inputs are only ever created, never set, while the
/// workers run: each works through its own handle on the database (a salsa handle shares the
/// memos) and claims the next path from a shared counter. Results come back in `paths` order.
pub fn par_map<T: Send>(
    db: &Database,
    paths: &[PathBuf],
    work: impl Fn(&Database, &Path) -> T + Sync,
) -> Vec<T> {
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(paths.len());
    let next = AtomicUsize::new(0);
    let mut slots: Vec<Option<T>> = paths.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                let db = db.clone();
                let (next, work) = (&next, &work);
                scope.spawn(move || {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = paths.get(index) else {
                            break done;
                        };
                        done.push((index, work(&db, path)));
                    }
                })
            })
            .collect();
        for worker in workers {
            match worker.join() {
                Ok(done) => {
                    for (index, result) in done {
                        slots[index] = Some(result);
                    }
                }
                Err(panic) => std::panic::resume_unwind(panic),
            }
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.expect("every path is claimed by one worker"))
        .collect()
}
