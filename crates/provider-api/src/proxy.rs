//! Out-of-process proxy: spawns the worker lazily, handshakes, forwards requests, and on a
//! crash or hang restarts it once before reporting it unavailable. A worker that answers again
//! has its restart allowance back, so a worker that lives as long as an editor session survives
//! any number of unrelated crashes.

use crate::protocol::{
    self, Empty, FLOW_GRAPH_FORMAT_VERSION, FlowGraphs, Graphs, Hello, OpenWorkspace,
    PROTOCOL_VERSION, ParseRequest, Parsed, ProtocolError, Query, Request, Response, UpdateText,
};
use crate::provider::Capabilities;
use core_semantics::Answers;
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
    /// Source parsed once by each prestarted worker, so that the first real request does not pay
    /// for JIT compilation; empty for none.
    pub warmup_source: String,
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
            warmup_source: String::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// No further requests can succeed in this process.
    #[error("{0}")]
    Unavailable(String),
    /// The runtime the worker needs is not installed (program not found, or the `dotnet` host
    /// reports a missing framework); the reason says which. No further requests can succeed.
    #[error("the worker's runtime is missing: {0}")]
    RuntimeMissing(String),
    /// The worker answered with a protocol-level error; it is still running.
    #[error("worker error {}: {}", .0.code, .0.message)]
    Worker(ProtocolError),
}

/// A failure of the transport itself (as opposed to an error reply).
enum Transport {
    /// Could not even start the process; restarting will not help.
    Spawn(String),
    /// The runtime is missing, see [`ProxyError::RuntimeMissing`].
    Runtime(String),
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
    /// Has this process been told the workspace? A restarted worker has not.
    workspace_sent: bool,
}

enum State {
    NotStarted,
    Running(Running),
    Unavailable(String),
    RuntimeMissing(String),
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
    /// The workspace the first worker holds its semantic state for; sent again after a restart.
    workspace: Mutex<Option<OpenWorkspace>>,
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
            workspace: Mutex::new(None),
        }
    }

    /// Starts the first two workers in the background, so that a format request that arrives while
    /// the first is busy with a build finds the second already running.
    pub fn prestart(self: &std::sync::Arc<Self>) {
        for i in 0..self.slots.len().min(2) {
            let proxy = self.clone();
            std::thread::spawn(move || {
                let mut inner = Self::lock(&proxy.slots[i]);
                let timeout = proxy.config.request_timeout;
                if let Ok(worker) = proxy.ensure_running(&mut inner)
                    && !proxy.config.warmup_source.is_empty()
                {
                    let _ = exchange(
                        worker,
                        &Request::Parse(ParseRequest {
                            file_id: "warmup".into(),
                            version: 1,
                            text: proxy.config.warmup_source.clone(),
                            language_version: None,
                            preprocessor_symbols: Vec::new(),
                        }),
                        timeout,
                    );
                }
            });
        }
    }

    /// Tells the worker which files make up the project. Semantic state lives in the first
    /// worker only; it is told lazily, before its next semantic request, and again after a restart.
    pub fn open_workspace(&self, workspace: OpenWorkspace) {
        *self.workspace.lock().unwrap_or_else(|e| e.into_inner()) = Some(workspace);
        if let State::Running(r) = &mut Self::lock(&self.slots[0]).state {
            r.workspace_sent = false;
        }
    }

    /// Gives the first worker the file's current text, then asks `items` about it and, if
    /// `flow`, for its flow graphs. The three are one uninterrupted exchange.
    pub fn analyze(
        &self,
        update: UpdateText,
        query: Option<Query>,
        flow: bool,
    ) -> Result<(Option<Answers>, Option<Graphs>), ProxyError> {
        let mut requests = vec![Request::UpdateText(update.clone())];
        requests.extend(query.map(Request::Query));
        if flow {
            requests.push(Request::FlowGraphs(FlowGraphs {
                file_id: update.file_id.clone(),
                version: update.version,
            }));
        }
        let (mut answers, mut graphs) = (None, None);
        for response in self.semantic(&requests)? {
            match response {
                Response::Answers(a) => answers = Some(a),
                Response::Graphs(g) => graphs = Some(g),
                Response::Ok(_) => {}
                other => {
                    return Err(ProxyError::Unavailable(format!(
                        "unexpected semantic reply: {other:?}"
                    )));
                }
            }
        }
        Ok((answers, graphs))
    }

    /// Runs the requests in order on the first worker (which holds the workspace). A crash
    /// restarts it, replays the workspace and starts the sequence again.
    fn semantic(&self, requests: &[Request]) -> Result<Vec<Response>, ProxyError> {
        let timeout = self.config.request_timeout;
        let mut inner = Self::lock(&self.slots[0]);
        loop {
            let worker = self.ensure_running(&mut inner)?;
            let workspace = self
                .workspace
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let mut run = || -> Result<Vec<Response>, String> {
                if !worker.workspace_sent {
                    if let Some(w) = &workspace {
                        reply_or_error(exchange(
                            worker,
                            &Request::OpenWorkspace(w.clone()),
                            timeout,
                        )?)?;
                    }
                    worker.workspace_sent = true;
                }
                requests
                    .iter()
                    .map(|r| exchange(worker, r, timeout))
                    .collect()
            };
            match run() {
                Ok(responses) => {
                    inner.restarts = 0;
                    return responses
                        .into_iter()
                        .map(|r| match r {
                            Response::Error(e) => Err(ProxyError::Worker(e)),
                            r => Ok(r),
                        })
                        .collect();
                }
                Err(why) => self
                    .recover(&mut inner, &why)
                    .map_err(ProxyError::Unavailable)?,
            }
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
                Ok(guard)
                    if !matches!(
                        guard.state,
                        State::Unavailable(_) | State::RuntimeMissing(_)
                    ) =>
                {
                    return guard;
                }
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
                State::RuntimeMissing(ref why) => {
                    return Err(ProxyError::RuntimeMissing(why.clone()));
                }
                State::NotStarted => {}
            }
            match self.start() {
                Ok(r) => inner.state = State::Running(r),
                Err(Transport::Runtime(why)) => {
                    inner.state = State::RuntimeMissing(why.clone());
                    return Err(ProxyError::RuntimeMissing(why));
                }
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
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                let name = program.to_string_lossy();
                if e.kind() == std::io::ErrorKind::NotFound {
                    Transport::Runtime(format!("cannot start `{name}`: {e}"))
                } else {
                    Transport::Spawn(format!("cannot start the worker ({name}): {e}"))
                }
            })?;
        let stderr = tee_stderr(child.stderr.take().expect("stderr is piped"));
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
            workspace_sent: false,
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
        // A host that cannot find its runtime prints why and exits before the handshake.
        let died = |mut r: Running, why: String| {
            kill(&mut r.child);
            let code = r.child.wait().ok().and_then(|s| s.code());
            let text = stderr.join().unwrap_or_default();
            match missing_runtime(code, &text) {
                Some(reason) => Transport::Runtime(reason),
                None => Transport::Broken(why),
            }
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
            Err(why) => Err(died(running, why)),
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

/// The exit code of the .NET host when no matching framework is installed (0x80008096).
const HOST_FRAMEWORK_MISSING: i32 = 0x8000_8096_u32 as i32;

/// Forwards the worker's stderr to ours and keeps the first part of it.
fn tee_stderr(stderr: std::process::ChildStderr) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut kept = String::new();
        for line in BufReader::new(stderr).split(10).map_while(Result::ok) {
            let line = String::from_utf8_lossy(&line);
            eprintln!("{line}");
            if kept.len() < 8192 {
                kept.push_str(&line);
                kept.extend(['\x0a']);
            }
        }
        kept
    })
}

/// Why the `dotnet` host exited without a runtime, from its exit code (on Windows) or its
/// message (everywhere; other platforms truncate the code), if that is why.
fn missing_runtime(code: Option<i32>, stderr: &str) -> Option<String> {
    let by_text = stderr.contains("You must install")
        || (stderr.contains("framework") && stderr.contains("was not found"));
    if code != Some(HOST_FRAMEWORK_MISSING) && !by_text {
        return None;
    }
    let line = |needle: &str| stderr.lines().find(|l| l.contains(needle));
    Some(
        line("was not found")
            .or_else(|| line("You must install"))
            .map_or("the .NET host found no matching runtime", str::trim)
            .to_string(),
    )
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

/// A protocol error reply to a setup request is a broken worker, not a result.
fn reply_or_error(response: Response) -> Result<(), String> {
    match response {
        Response::Error(e) => Err(format!("worker error {}: {}", e.code, e.message)),
        _ => Ok(()),
    }
}
