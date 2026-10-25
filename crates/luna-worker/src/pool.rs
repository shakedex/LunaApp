//! Long-lived worker processes, one clip at a time each. A crash or a hang costs
//! that clip only: the worker is replaced and the run continues.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tracing::{Level, Span, debug, error, info, info_span};

use crate::protocol::{read_message, write_message};
use crate::{ClipJob, ClipResult, Request};

/// Receives each line a worker writes to stderr.
pub type LineSink = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct PoolConfig {
    /// The executable to start (Luna itself in production).
    pub exe: PathBuf,
    /// Arguments that put it in worker mode (e.g. `--worker`).
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub workers: usize,
    /// How long one clip may take before its worker is killed.
    pub timeout: Duration,
    /// Where worker stderr lines go. `None` sends them to `tracing` as
    /// `luna::worker_line` events.
    pub on_line: Option<LineSink>,
}

impl std::fmt::Debug for PoolConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolConfig")
            .field("exe", &self.exe)
            .field("args", &self.args)
            .field("env", &self.env)
            .field("workers", &self.workers)
            .field("timeout", &self.timeout)
            .field("on_line", &self.on_line.as_ref().map(|_| "Fn"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    Done(Box<ClipResult>),
    Crashed,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    /// No worker could be started at all.
    Spawn(String),
    Cancelled,
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::Spawn(m) => write!(f, "could not start a worker: {m}"),
            PoolError::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for PoolError {}

/// How often a waiting thread looks at the cancel flag.
const POLL: Duration = Duration::from_millis(100);

/// How long a finished worker's last stderr lines may take to arrive.
const DRAIN: Duration = Duration::from_secs(2);

/// Runs every job, returning one outcome per job (in completion order).
/// `on_done` is called as each finishes, from pool threads.
pub fn run_pool(
    config: &PoolConfig,
    jobs: Vec<ClipJob>,
    cancel: &AtomicBool,
    on_done: &(dyn Fn(u64, &JobOutcome) + Sync),
) -> Result<Vec<(u64, JobOutcome)>, PoolError> {
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    // Fail fast when the worker can't start, rather than marking every clip crashed.
    Worker::spawn(config).map_err(PoolError::Spawn)?.shutdown();

    let workers = config.workers.clamp(1, jobs.len());
    let queue = Mutex::new(VecDeque::from(jobs));
    let results = Mutex::new(Vec::new());
    // Pool threads log inside the caller's span (the run).
    let span = Span::current();

    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                let _run = span.enter();
                drive(config, &queue, &results, cancel, on_done)
            });
        }
    });

    if cancel.load(Ordering::SeqCst) {
        return Err(PoolError::Cancelled);
    }
    Ok(results.into_inner().unwrap_or_else(|e| e.into_inner()))
}

fn drive(
    config: &PoolConfig,
    queue: &Mutex<VecDeque<ClipJob>>,
    results: &Mutex<Vec<(u64, JobOutcome)>>,
    cancel: &AtomicBool,
    on_done: &(dyn Fn(u64, &JobOutcome) + Sync),
) {
    let mut worker: Option<Worker> = None;
    // Set once a worker is killed, so the next start is a replacement.
    let mut lost = false;
    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let Some(job) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front() else {
            break;
        };
        let id = job.id;
        let main = job.files.main.clone();
        let path = main.display();
        // Started outside the clip: a worker outlives the clip it starts on.
        let next = match worker.take() {
            Some(w) => Ok(w),
            None => {
                if std::mem::take(&mut lost) {
                    info!("worker replaced");
                }
                Worker::spawn(config)
            }
        };
        let stem = main.file_stem().unwrap_or_default().to_string_lossy();
        let _clip = info_span!("clip", clip = %stem).entered();
        let start = Instant::now();
        let outcome = match next {
            Err(e) => {
                error!(path = %path, error = %e, "worker could not start");
                JobOutcome::Crashed
            }
            Ok(mut w) => match w.run(job, config.timeout, cancel) {
                Step::Done(r) => {
                    let ms = start.elapsed().as_millis() as u64;
                    debug!(id, path = %path, ms, "job done");
                    worker = Some(w);
                    JobOutcome::Done(r)
                }
                // Logged once the worker's last lines are in, so they come first.
                Step::Died => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    w.kill();
                    error!(path = %path, elapsed_ms, "worker crashed");
                    lost = true;
                    JobOutcome::Crashed
                }
                Step::TimedOut => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    w.kill();
                    error!(path = %path, elapsed_ms, "worker timed out");
                    lost = true;
                    JobOutcome::TimedOut
                }
                Step::Cancelled => {
                    w.kill();
                    break;
                }
            },
        };
        on_done(id, &outcome);
        results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((id, outcome));
    }
    if let Some(w) = worker {
        w.shutdown();
    }
}

enum Step {
    Done(Box<ClipResult>),
    Died,
    TimedOut,
    Cancelled,
}

struct Worker {
    child: Arc<Mutex<Child>>,
    stdin: BufWriter<ChildStdin>,
    results: Receiver<Option<ClipResult>>,
    /// Disconnects once the stderr reader has forwarded its last line.
    stderr_done: Receiver<()>,
}

impl Worker {
    fn spawn(config: &PoolConfig) -> Result<Worker, String> {
        let mut cmd = Command::new(&config.exe);
        cmd.args(&config.args)
            .envs(config.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        debug!(pid = child.id(), "worker started");
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = child.stderr.take().ok_or("no stderr")?;
        // Forwarded lines belong to the caller's span (the run, and the clip).
        let span = Span::current();

        // Readers per process: blocking reads end when the process exits or is killed.
        let (done_tx, stderr_done) = mpsc::channel::<()>();
        let on_line = config.on_line.clone();
        std::thread::spawn(move || {
            let _run = span.enter();
            forward_lines(stderr, on_line.as_deref());
            drop(done_tx);
        });

        let (tx, rx) = mpsc::channel();
        // Logs nothing, so it holds no span: it may outlive the run.
        std::thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            loop {
                match read_message::<ClipResult>(&mut r) {
                    Ok(Some(res)) => {
                        if tx.send(Some(res)).is_err() {
                            break;
                        }
                    }
                    _ => {
                        let _ = tx.send(None);
                        break;
                    }
                }
            }
        });
        Ok(Worker {
            child: Arc::new(Mutex::new(child)),
            stdin: BufWriter::new(stdin),
            results: rx,
            stderr_done,
        })
    }

    fn run(&mut self, job: ClipJob, timeout: Duration, cancel: &AtomicBool) -> Step {
        let id = job.id;
        debug!(id, path = %job.files.main.display(), "job started");
        if write_message(&mut self.stdin, &Request::Job(job)).is_err() {
            return Step::Died;
        }
        let deadline = Instant::now() + timeout;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Step::Cancelled;
            }
            let wait = deadline.saturating_duration_since(Instant::now()).min(POLL);
            if wait.is_zero() {
                return Step::TimedOut;
            }
            match self.results.recv_timeout(wait) {
                Ok(Some(r)) if r.id == id => return Step::Done(Box::new(r)),
                Ok(Some(_)) => continue,
                Ok(None) | Err(RecvTimeoutError::Disconnected) => return Step::Died,
                Err(RecvTimeoutError::Timeout) => continue,
            }
        }
    }

    fn kill(self) {
        {
            let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
            let _ = child.kill();
            let _ = child.wait();
        }
        self.drain_stderr();
    }

    fn shutdown(mut self) {
        let _ = write_message(&mut self.stdin, &Request::Shutdown);
        {
            let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
            // Give it a moment to exit on its own, then make sure.
            let start = Instant::now();
            let mut exited = false;
            while start.elapsed() < Duration::from_secs(2) {
                if let Ok(Some(_)) = child.try_wait() {
                    exited = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            if !exited {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.drain_stderr();
    }

    /// Waits briefly for the last stderr lines, so they land in the run log.
    fn drain_stderr(&self) {
        let _ = self.stderr_done.recv_timeout(DRAIN);
    }
}

/// Reads stderr until the process is gone, forwarding each line. It only ever
/// reads, so a chatty worker never blocks; a cut-off last line is still forwarded.
fn forward_lines(stderr: ChildStderr, on_line: Option<&(dyn Fn(&str) + Send + Sync)>) {
    let mut r = BufReader::new(stderr);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&buf);
        let line = text.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        match on_line {
            Some(f) => f(line),
            None => log_line(line),
        }
    }
}

/// Re-emits a worker's line at the level it was written with (WARN if unknown).
fn log_line(line: &str) {
    match line_level(line).unwrap_or(Level::WARN) {
        Level::TRACE => tracing::trace!(target: "luna::worker_line", line = %line),
        Level::DEBUG => tracing::debug!(target: "luna::worker_line", line = %line),
        Level::INFO => tracing::info!(target: "luna::worker_line", line = %line),
        Level::WARN => tracing::warn!(target: "luna::worker_line", line = %line),
        Level::ERROR => tracing::error!(target: "luna::worker_line", line = %line),
    }
}

/// The level token of a formatted log line (`HH:MM:SS.mmm LEVEL …`).
fn line_level(line: &str) -> Option<Level> {
    match line.split_whitespace().nth(1)? {
        "TRACE" => Some(Level::TRACE),
        "DEBUG" => Some(Level::DEBUG),
        "INFO" => Some(Level::INFO),
        "WARN" => Some(Level::WARN),
        "ERROR" => Some(Level::ERROR),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_level_reads_the_second_token() {
        assert_eq!(line_level("12:00:00.000 ERROR x: hi"), Some(Level::ERROR));
        assert_eq!(
            line_level("12:00:00.000 INFO  [c] x: hi"),
            Some(Level::INFO)
        );
        assert_eq!(line_level("thread 'main' panicked"), None);
        assert_eq!(line_level(""), None);
    }
}
