//! Reports started over MCP, run one at a time in start order on a single
//! runner thread, so parallel requests never exceed the worker pool size.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use luna_core::run::{Progress, RunRequest, WorkerSpec, run};
use serde_json::{Value, json};

use crate::headless::result_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Done => "done",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobStatus {
    pub state: JobState,
    pub files_found: Option<usize>,
    pub clips_done: usize,
    pub clips_total: Option<usize>,
    pub problems: usize,
    /// The headless result JSON once the run has ended.
    pub result: Option<Value>,
}

impl JobStatus {
    pub fn to_json(&self) -> Value {
        json!({
            "state": self.state.as_str(),
            "files_found": self.files_found,
            "clips_done": self.clips_done,
            "clips_total": self.clips_total,
            "problems": self.problems,
            "result": self.result,
        })
    }
}

struct Job {
    request: Option<RunRequest>,
    status: JobStatus,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    next: u64,
    queue: VecDeque<String>,
    jobs: HashMap<String, Job>,
    shutdown: bool,
}

struct Inner {
    state: Mutex<State>,
    wake: Condvar,
    workers: WorkerSpec,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut JobStatus)) {
        if let Some(job) = self.lock().jobs.get_mut(id) {
            f(&mut job.status);
        }
    }
}

pub struct Jobs {
    inner: Arc<Inner>,
    runner: Mutex<Option<JoinHandle<()>>>,
}

impl Jobs {
    pub fn new(workers: WorkerSpec) -> Self {
        let inner = Arc::new(Inner {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            workers,
        });
        let runner_inner = inner.clone();
        let runner = std::thread::spawn(move || runner_loop(&runner_inner));
        Self {
            inner,
            runner: Mutex::new(Some(runner)),
        }
    }

    /// Queues a report; returns its id (`r1`, `r2`, …).
    pub fn start(&self, request: RunRequest) -> String {
        let mut state = self.inner.lock();
        state.next += 1;
        let id = format!("r{}", state.next);
        state.jobs.insert(
            id.clone(),
            Job {
                request: Some(request),
                status: JobStatus {
                    state: JobState::Queued,
                    files_found: None,
                    clips_done: 0,
                    clips_total: None,
                    problems: 0,
                    result: None,
                },
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );
        state.queue.push_back(id.clone());
        self.inner.wake.notify_all();
        id
    }

    pub fn status(&self, id: &str) -> Option<JobStatus> {
        self.inner.lock().jobs.get(id).map(|j| j.status.clone())
    }

    /// A queued job is dropped without running; a running one is asked to
    /// stop and writes nothing. Returns the state it is left in.
    pub fn cancel(&self, id: &str) -> Option<JobState> {
        let mut state = self.inner.lock();
        let state = &mut *state;
        let job = state.jobs.get_mut(id)?;
        match job.status.state {
            JobState::Queued => {
                state.queue.retain(|q| q != id);
                job.request = None;
                job.status.state = JobState::Cancelled;
            }
            JobState::Running => {
                job.cancel.store(true, Ordering::SeqCst);
                job.status.state = JobState::Cancelled;
            }
            _ => {}
        }
        Some(job.status.state)
    }

    /// Cancels everything and waits for the runner to stop.
    pub fn shutdown(&self) {
        {
            let mut state = self.inner.lock();
            state.shutdown = true;
            for job in state.jobs.values_mut() {
                job.cancel.store(true, Ordering::SeqCst);
                if matches!(job.status.state, JobState::Queued | JobState::Running) {
                    job.request = None;
                    job.status.state = JobState::Cancelled;
                }
            }
            state.queue.clear();
            self.inner.wake.notify_all();
        }
        let runner = self.runner.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(runner) = runner {
            let _ = runner.join();
        }
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn runner_loop(inner: &Inner) {
    loop {
        let (id, request, cancel) = {
            let mut state = inner.lock();
            loop {
                if state.shutdown {
                    return;
                }
                if let Some(id) = state.queue.pop_front() {
                    let job = state.jobs.get_mut(&id).expect("queued job exists");
                    if let Some(request) = job.request.take() {
                        job.status.state = JobState::Running;
                        break (id, request, job.cancel.clone());
                    }
                    continue;
                }
                state = inner.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
        };
        let progress = |p: Progress| {
            inner.update(&id, |s| match p {
                Progress::FilesFound(n) => s.files_found = Some(n),
                Progress::ClipsTotal(n) => s.clips_total = Some(n),
                Progress::ClipDone { done, problems, .. } => {
                    s.clips_done = done;
                    s.problems = problems;
                }
                Progress::Probed { .. } => {}
            })
        };
        let result = run(&request, &inner.workers, &progress, &cancel);
        let json = result_json(&result);
        inner.update(&id, |s| {
            // A run that wrote its report is done even if cancel came late.
            s.state = match &result {
                Ok(_) => JobState::Done,
                Err(_) if cancel.load(Ordering::SeqCst) => JobState::Cancelled,
                Err(luna_core::run::RunError::Cancelled) => JobState::Cancelled,
                Err(_) => JobState::Failed,
            };
            s.result = Some(json);
        });
    }
}
