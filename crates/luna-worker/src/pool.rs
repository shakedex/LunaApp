//! Long-lived worker processes, one clip at a time each. A crash or a hang costs
//! that clip only: the worker is replaced and the run continues.

use std::collections::VecDeque;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::protocol::{read_message, write_message};
use crate::{ClipJob, ClipResult, Request};

#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// The executable to start (Luna itself in production).
    pub exe: PathBuf,
    /// Arguments that put it in worker mode (e.g. `--worker`).
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub workers: usize,
    /// How long one clip may take before its worker is killed.
    pub timeout: Duration,
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

    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| drive(config, &queue, &results, cancel, on_done));
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
    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let Some(job) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front() else {
            break;
        };
        let id = job.id;
        let outcome = match worker.take().map_or_else(|| Worker::spawn(config), Ok) {
            Err(_) => JobOutcome::Crashed,
            Ok(mut w) => match w.run(job, config.timeout, cancel) {
                Step::Done(r) => {
                    worker = Some(w);
                    JobOutcome::Done(r)
                }
                Step::Died => {
                    w.kill();
                    JobOutcome::Crashed
                }
                Step::TimedOut => {
                    w.kill();
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
}

impl Worker {
    fn spawn(config: &PoolConfig) -> Result<Worker, String> {
        let mut cmd = Command::new(&config.exe);
        cmd.args(&config.args)
            .envs(config.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (tx, rx) = mpsc::channel();
        // A reader per process: blocking reads end when the process exits or is killed.
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
        })
    }

    fn run(&mut self, job: ClipJob, timeout: Duration, cancel: &AtomicBool) -> Step {
        let id = job.id;
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
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }

    fn shutdown(mut self) {
        let _ = write_message(&mut self.stdin, &Request::Shutdown);
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        // Give it a moment to exit on its own, then make sure.
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}
