//! The analysis thread. Diagnostics and code actions run the rules, which for C# can mean a
//! semantic request to the worker (the first one builds the whole compilation: seconds). They run
//! here, one at a time in the order asked, so that the message loop stays free for formatting,
//! edits and cancellations.
//!
//! The thread works on a snapshot of the [`Workspace`](crate::Workspace): a clone of the database
//! handle and of the document list. The message loop keeps editing its own copy; while the thread
//! holds a handle it never sets a database input (that would wait for the thread, see
//! `Workspace::change`). A request whose document changes before or while it runs is dropped:
//! queued ones never start, a running one is abandoned at its next checkpoint (the worker call
//! in progress cannot be interrupted) and its result is not sent.

use core_incremental::Cancelled;
use crossbeam_channel::Sender;
use lsp_server::{ErrorCode, Message, RequestId, Response, ResponseError};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::JoinHandle;

/// Why a request should stop, if it should.
#[derive(Default)]
pub(crate) struct Cancel(AtomicU8);

const BY_CLIENT: u8 = 1;
const SUPERSEDED: u8 = 2;

impl Cancel {
    /// The error to answer with, once the request is cancelled. A cancel from the client is
    /// `RequestCancelled`; the server dropping a request that its document's next version
    /// replaces is `ServerCancelled`, which asks the client to ask again.
    pub(crate) fn error(&self) -> Option<ResponseError> {
        match self.0.load(Ordering::Acquire) {
            BY_CLIENT => Some(ResponseError {
                code: ErrorCode::RequestCanceled as i32,
                message: "cancelled by the client".into(),
                data: None,
            }),
            SUPERSEDED => Some(superseded()),
            _ => None,
        }
    }

    fn set(&self, why: u8) {
        let _ = self
            .0
            .compare_exchange(0, why, Ordering::AcqRel, Ordering::Acquire);
    }
}

fn superseded() -> ResponseError {
    ResponseError {
        code: ErrorCode::ServerCancelled as i32,
        message: "the document changed".into(),
        data: Some(serde_json::json!({ "retriggerRequest": true })),
    }
}

type Work = Box<dyn FnOnce(&Cancel) -> serde_json::Value + Send>;

struct Job {
    id: RequestId,
    cancel: Arc<Cancel>,
    work: Work,
}

/// The requests queued or running, with the document each is about.
type Requests = HashMap<RequestId, (Option<PathBuf>, Arc<Cancel>)>;
type Live = Arc<Mutex<Requests>>;

fn lock(live: &Live) -> MutexGuard<'_, Requests> {
    live.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Analysis {
    jobs: Option<mpsc::Sender<Job>>,
    /// Jobs queued or running; each holds a snapshot, hence a database handle.
    outstanding: Arc<AtomicUsize>,
    live: Live,
    thread: Option<JoinHandle<()>>,
}

impl Analysis {
    pub(crate) fn start(sender: Sender<Message>) -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        let outstanding = Arc::new(AtomicUsize::new(0));
        let live = Live::default();
        let thread = {
            let (outstanding, live) = (outstanding.clone(), live.clone());
            std::thread::spawn(move || {
                for Job { id, cancel, work } in queue {
                    let result = match cancel.error() {
                        Some(e) => Err(e),
                        None => match Cancelled::catch(AssertUnwindSafe(|| work(&cancel))) {
                            // A config reload cancelled the queries under it.
                            Err(_) => Err(superseded()),
                            Ok(value) => cancel.error().map_or(Ok(value), Err),
                        },
                    };
                    lock(&live).remove(&id);
                    // The snapshot is gone with `work`: the loop may set inputs again.
                    outstanding.fetch_sub(1, Ordering::AcqRel);
                    let response = match result {
                        Ok(value) => Response::new_ok(id, value),
                        Err(e) => Response {
                            id,
                            response_result: Err(e),
                        },
                    };
                    let _ = sender.send(response.into());
                }
            })
        };
        Analysis {
            jobs: Some(jobs),
            outstanding,
            live,
            thread: Some(thread),
        }
    }

    /// Does the thread hold (or will it hold) a handle of the database?
    pub(crate) fn is_busy(&self) -> bool {
        self.outstanding.load(Ordering::Acquire) > 0
    }

    /// Queues `work` as the answer to request `id` about the document at `path`.
    pub(crate) fn submit(
        &self,
        id: RequestId,
        path: Option<PathBuf>,
        work: impl FnOnce(&Cancel) -> serde_json::Value + Send + 'static,
    ) {
        let cancel = Arc::new(Cancel::default());
        lock(&self.live).insert(id.clone(), (path, cancel.clone()));
        self.outstanding.fetch_add(1, Ordering::AcqRel);
        let job = Job {
            id,
            cancel,
            work: Box::new(work),
        };
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// `$/cancelRequest`.
    pub(crate) fn cancel(&self, id: &RequestId) {
        if let Some((_, cancel)) = lock(&self.live).get(id) {
            cancel.set(BY_CLIENT);
        }
    }

    /// The document at `path` changed or closed: what was asked about the old text is stale.
    pub(crate) fn supersede(&self, path: &Path) {
        for (p, cancel) in lock(&self.live).values() {
            if p.as_deref() == Some(path) {
                cancel.set(SUPERSEDED);
            }
        }
    }

    /// Stops the thread after the request it is in the middle of.
    pub(crate) fn finish(&mut self) {
        for (_, cancel) in lock(&self.live).values() {
            cancel.set(BY_CLIENT);
        }
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
