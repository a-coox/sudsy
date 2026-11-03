//! Out-of-process proxy: spawns the worker lazily, handshakes, forwards requests, and on a
//! crash or hang restarts it once before reporting it unavailable. A worker that answers again
//! has its restart allowance back, so a worker that lives as long as an editor session survives
//! any number of unrelated crashes.

use crate::protocol::{
    self, Empty, FLOW_GRAPH_FORMAT_VERSION, Hello, PROTOCOL_VERSION, ParseRequest, Parsed,
    ProtocolError, Request, Response,
};
use crate::provider::Capabilities;
use core_syntax::wire::WIRE_VERSION;
use std::ffi::OsString;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    /// Program followed by its arguments, e.g. `["dotnet", "Sudsy.Worker.dll"]`.
    pub command: Vec<OsString>,
    /// The grammar version the worker must have been built from.
    pub grammar_hash: String,
    pub handshake_timeout: Duration,
    /// A request taking longer than this counts as a hang.
    pub request_timeout: Duration,
    /// How often a crashed or hung worker is restarted in a row before giving up; a reply resets
    /// the count.
    pub max_restarts: u32,
    /// Environment variables set for the worker process.
    pub env: Vec<(OsString, OsString)>,
    /// How many worker processes may run side by side. A worker handles one request at a time;
    /// with more of them, threads that parse at once do not queue behind each other. They start
    /// when a request finds all the others busy.
    pub pool_size: usize,
    /// Added to the message when the worker's program cannot be started, to say what to install.
    pub install_hint: String,
}

impl WorkerConfig {
    pub fn new(command: Vec<OsString>, grammar_hash: impl Into<String>) -> Self {
        WorkerConfig {
            command,
            grammar_hash: grammar_hash.into(),
            handshake_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(120),
            max_restarts: 1,
            env: Vec::new(),
            pool_size: 1,
            install_hint: String::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// No further requests can succeed in this run.
    #[error("{0}")]
    Unavailable(String),
    /// The worker answered with a protocol-level error; it is still running.
    #[error("worker error {}: {}", .0.code, .0.message)]
    Worker(ProtocolError),
}

/// A failure of the transport itself (as opposed to an error reply).
enum Transport {
    /// Could not even start the process; restarting will not help.
    Spawn(String),
    /// Refused the handshake; restarting will not help.
    Refused(String),
    /// Crashed, closed its pipes, hung, or sent garbage.
    Broken(String),
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    frames: Receiver<Result<Option<Vec<u8>>, protocol::FrameError>>,
    capabilities: Capabilities,
}

enum State {
    NotStarted,
    Running(Running),
    Unavailable(String),
}

struct Inner {
    state: State,
    restarts: u32,
}

pub struct WorkerProxy {
    config: WorkerConfig,
    /// One entry per worker process of the pool.
    slots: Vec<Mutex<Inner>>,
    /// Where the search for a free slot starts, so that waiting requests spread over the pool.
    next: AtomicUsize,
}

impl WorkerProxy {
    pub fn new(config: WorkerConfig) -> Self {
        let slots = (0..config.pool_size.max(1))
            .map(|_| {
                Mutex::new(Inner {
                    state: State::NotStarted,
                    restarts: 0,
                })
            })
            .collect();
        WorkerProxy {
            config,
            slots,
            next: AtomicUsize::new(0),
        }
    }

    /// Starts the first worker if needed and returns what it advertises.
    pub fn capabilities(&self) -> Result<Capabilities, ProxyError> {
        self.with_worker(|_| Ok(()))?;
        match &Self::lock(&self.slots[0]).state {
            State::Running(r) => Ok(r.capabilities),
            _ => Err(ProxyError::Unavailable("worker is not running".into())),
        }
    }

    pub fn parse(&self, request: ParseRequest) -> Result<Parsed, ProxyError> {
        match self.request(Request::Parse(request))? {
            Response::Parsed(p) => Ok(p),
            other => Err(ProxyError::Unavailable(format!(
                "unexpected reply to Parse: {other:?}"
            ))),
        }
    }

    /// Sends any request (used by contract tests for the not-yet-supported messages).
    pub fn request(&self, request: Request) -> Result<Response, ProxyError> {
        let timeout = self.config.request_timeout;
        let mut inner = self.claim();
        loop {
            let worker = self.ensure_running(&mut inner)?;
            match exchange(worker, &request, timeout) {
                Ok(response) => {
                    inner.restarts = 0;
                    return match response {
                        Response::Error(e) => Err(ProxyError::Worker(e)),
                        response => Ok(response),
                    };
                }
                Err(why) => {
                    self.recover(&mut inner, &why)
                        .map_err(ProxyError::Unavailable)?;
                }
            }
        }
    }

    /// Asks the worker to exit and waits briefly, then makes sure it is gone.
    pub fn shutdown(&self) {
        for slot in &self.slots {
            let mut inner = Self::lock(slot);
            if let State::Running(mut r) = std::mem::replace(&mut inner.state, State::NotStarted) {
                let _ = protocol::send(&mut r.stdin, &Request::Shutdown(Empty {}));
                let _ = r.frames.recv_timeout(Duration::from_secs(2));
                kill(&mut r.child);
            }
            inner.state = State::Unavailable("worker was shut down".into());
        }
    }

    fn lock(slot: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
        slot.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The first worker that is not busy (so a worker starts only when the ones before it are
    /// occupied), else, waiting for it, the next one in turn. Workers that have become
    /// unavailable are passed over while another one is left.
    fn claim(&self) -> MutexGuard<'_, Inner> {
        let n = self.slots.len();
        let mut busy = false;
        for slot in &self.slots {
            match slot.try_lock() {
                Ok(guard) if !matches!(guard.state, State::Unavailable(_)) => return guard,
                Err(std::sync::TryLockError::Poisoned(e)) => return e.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => busy = true,
                Ok(_) => {}
            }
        }
        // Everything is busy (wait for one) or unavailable (report the first one's reason).
        let turn = if busy {
            self.next.fetch_add(1, Ordering::Relaxed) % n
        } else {
            0
        };
        Self::lock(&self.slots[turn])
    }

    fn with_worker(
        &self,
        f: impl FnOnce(&mut Running) -> Result<(), String>,
    ) -> Result<(), ProxyError> {
        let mut inner = Self::lock(&self.slots[0]);
        let worker = self.ensure_running(&mut inner)?;
        f(worker).map_err(ProxyError::Unavailable)
    }

    /// Starts the worker if it is not running; returns it. Applies the restart policy to
    /// start-up failures too.
    fn ensure_running<'a>(&self, inner: &'a mut Inner) -> Result<&'a mut Running, ProxyError> {
        loop {
            match inner.state {
                State::Running(_) => break,
                State::Unavailable(ref why) => return Err(ProxyError::Unavailable(why.clone())),
                State::NotStarted => {}
            }
            match self.start() {
                Ok(r) => inner.state = State::Running(r),
                Err(Transport::Spawn(why) | Transport::Refused(why)) => {
                    inner.state = State::Unavailable(why.clone());
                    return Err(ProxyError::Unavailable(why));
                }
                Err(Transport::Broken(why)) => {
                    self.recover(inner, &why).map_err(ProxyError::Unavailable)?;
                }
            }
        }
        match &mut inner.state {
            State::Running(r) => Ok(r),
            _ => unreachable!("loop only breaks when running"),
        }
    }

    /// Drops the broken worker. `Ok` means a restart is allowed (the state becomes `NotStarted`);
    /// `Err` carries the message of the now permanent unavailability.
    fn recover(&self, inner: &mut Inner, why: &str) -> Result<(), String> {
        if let State::Running(mut r) = std::mem::replace(&mut inner.state, State::NotStarted) {
            kill(&mut r.child);
        }
        if inner.restarts < self.config.max_restarts {
            inner.restarts += 1;
            return Ok(());
        }
        let message = format!("worker failed after {} restart(s): {why}", inner.restarts);
        inner.state = State::Unavailable(message.clone());
        Err(message)
    }

    fn start(&self) -> Result<Running, Transport> {
        let (program, args) = self
            .config
            .command
            .split_first()
            .ok_or_else(|| Transport::Spawn("no worker command".into()))?;
        let mut child = Command::new(program)
            .args(args)
            .envs(self.config.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                Transport::Spawn(format!(
                    "cannot start the worker ({}): {e}{}",
                    program.to_string_lossy(),
                    self.config.install_hint
                ))
            })?;
        let stdin = child.stdin.take().expect("stdin is piped");
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let (tx, frames) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let frame = protocol::read_frame(&mut stdout);
                let done = !matches!(frame, Ok(Some(_)));
                if tx.send(frame).is_err() || done {
                    break;
                }
            }
        });
        let mut running = Running {
            child,
            stdin,
            frames,
            capabilities: Capabilities::NONE,
        };
        let ours = Hello {
            protocol_version: PROTOCOL_VERSION,
            grammar_hash: self.config.grammar_hash.clone(),
            flow_graph_format: FLOW_GRAPH_FORMAT_VERSION,
            wire_format: WIRE_VERSION,
            capabilities: vec![],
        };
        let reply = exchange(
            &mut running,
            &Request::Hello(ours.clone()),
            self.config.handshake_timeout,
        );
        let fail = |mut r: Running, t: Transport| {
            kill(&mut r.child);
            t
        };
        match reply {
            Ok(Response::Hello(theirs)) => {
                let same = theirs.protocol_version == ours.protocol_version
                    && theirs.grammar_hash == ours.grammar_hash
                    && theirs.flow_graph_format == ours.flow_graph_format
                    && theirs.wire_format == ours.wire_format;
                if !same {
                    return Err(fail(
                        running,
                        Transport::Refused(format!("worker version mismatch: {theirs:?}")),
                    ));
                }
                running.capabilities = Capabilities::from_names(&theirs.capabilities);
                Ok(running)
            }
            Ok(Response::Error(e)) if e.code == "version_mismatch" => Err(fail(
                running,
                Transport::Refused(format!("worker refused the handshake: {}", e.message)),
            )),
            Ok(other) => Err(fail(
                running,
                Transport::Broken(format!("unexpected handshake reply: {other:?}")),
            )),
            Err(why) => Err(fail(running, Transport::Broken(why))),
        }
    }
}

impl Drop for WorkerProxy {
    fn drop(&mut self) {
        for slot in &self.slots {
            if let State::Running(r) = &mut Self::lock(slot).state {
                kill(&mut r.child);
            }
        }
    }
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// One request/reply round trip; the error string describes a broken transport.
fn exchange(
    worker: &mut Running,
    request: &Request,
    timeout: Duration,
) -> Result<Response, String> {
    protocol::send(&mut worker.stdin, request)
        .map_err(|e| format!("cannot send to the worker: {e}"))?;
    match worker.frames.recv_timeout(timeout) {
        Ok(Ok(Some(frame))) => {
            protocol::decode(&frame).map_err(|e| format!("bad reply from the worker: {e}"))
        }
        Ok(Ok(None)) => Err("the worker exited".into()),
        Ok(Err(e)) => Err(format!("the worker's output failed: {e}")),
        Err(RecvTimeoutError::Timeout) => {
            Err(format!("the worker did not answer within {timeout:?}"))
        }
        Err(RecvTimeoutError::Disconnected) => Err("the worker exited".into()),
    }
}
