use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use luna_formats::{ClipFiles, Container};
use luna_worker::ClipJob;
use luna_worker::pool::{JobOutcome, PoolConfig, PoolError, run_pool};

fn config(workers: usize, timeout: Duration) -> PoolConfig {
    PoolConfig {
        exe: env!("CARGO_BIN_EXE_luna-worker").into(),
        args: vec![],
        env: vec![("LUNA_WORKER_TEST_HOOKS".into(), "1".into())],
        workers,
        timeout,
    }
}

fn job(id: u64, name: &str) -> ClipJob {
    ClipJob {
        id,
        files: ClipFiles {
            container: Container::QuickTime,
            main: format!("Z:/does-not-exist/{name}").into(),
            segments: vec![],
            sidecars: vec![],
        },
        strip: 3,
        thumb_width: 640,
    }
}

fn sorted(mut v: Vec<(u64, JobOutcome)>) -> Vec<(u64, JobOutcome)> {
    v.sort_by_key(|(id, _)| *id);
    v
}

#[test]
fn every_job_gets_an_answer() {
    let jobs: Vec<_> = (0..12).map(|i| job(i, &format!("clip{i}.mov"))).collect();
    let seen = Mutex::new(0);
    let out = run_pool(
        &config(3, Duration::from_secs(30)),
        jobs,
        &AtomicBool::new(false),
        &|_, _| *seen.lock().unwrap() += 1,
    )
    .unwrap();
    let out = sorted(out);
    assert_eq!(out.len(), 12);
    assert_eq!(*seen.lock().unwrap(), 12);
    for (i, (id, outcome)) in out.iter().enumerate() {
        assert_eq!(*id, i as u64);
        let JobOutcome::Done(r) = outcome else {
            panic!("job {id}: {outcome:?}")
        };
        assert_eq!(r.id, *id);
        assert_eq!(r.thumbnails.len(), 3);
    }
}

#[test]
fn a_crashing_worker_costs_one_clip() {
    let jobs = vec![job(0, "a.mov"), job(1, "__crash__.mov"), job(2, "b.mov")];
    let out = sorted(
        run_pool(
            &config(1, Duration::from_secs(30)),
            jobs,
            &AtomicBool::new(false),
            &|_, _| {},
        )
        .unwrap(),
    );
    assert!(matches!(out[0].1, JobOutcome::Done(_)));
    assert_eq!(out[1].1, JobOutcome::Crashed);
    assert!(
        matches!(out[2].1, JobOutcome::Done(_)),
        "pool restarted the worker"
    );
}

#[test]
fn a_hanging_worker_times_out() {
    let jobs = vec![job(0, "__hang__.mov"), job(1, "b.mov")];
    let start = Instant::now();
    let out = sorted(
        run_pool(
            &config(1, Duration::from_secs(2)),
            jobs,
            &AtomicBool::new(false),
            &|_, _| {},
        )
        .unwrap(),
    );
    assert_eq!(out[0].1, JobOutcome::TimedOut);
    assert!(matches!(out[1].1, JobOutcome::Done(_)));
    assert!(start.elapsed() < Duration::from_secs(20));
}

#[test]
fn cancel_stops_the_run() {
    let jobs: Vec<_> = (0..4).map(|i| job(i, "__hang__.mov")).collect();
    let cancel = AtomicBool::new(false);
    let start = Instant::now();
    let res = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            cancel.store(true, Ordering::SeqCst);
        });
        run_pool(
            &config(2, Duration::from_secs(60)),
            jobs,
            &cancel,
            &|_, _| {},
        )
    });
    assert_eq!(res, Err(PoolError::Cancelled));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_missing_worker_binary_is_a_run_error() {
    let mut c = config(1, Duration::from_secs(5));
    c.exe = "Z:/no/such/luna.exe".into();
    let res = run_pool(
        &c,
        vec![job(0, "a.mov")],
        &AtomicBool::new(false),
        &|_, _| {},
    );
    assert!(matches!(res, Err(PoolError::Spawn(_))));
}
