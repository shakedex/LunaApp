//! `report` and `inspect` without a window: progress on stderr, the result on
//! stdout, and an exit code a script can branch on.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use luna_core::cli::{InspectArgs, ReportArgs};
use luna_core::inspect::{Inspection, inspect};
use luna_core::run::{Progress, RunError, RunOutcome, WorkerSpec, run};
use luna_core::settings::Settings;
use serde_json::{Value, json};

/// Exit codes (spec §5.3).
pub const WRITTEN: u8 = 0;
pub const WRITTEN_WITH_PROBLEMS: u8 = 1;
pub const BAD_ARGUMENTS: u8 = 2;
pub const SOURCE_UNREADABLE: u8 = 3;
pub const FAILED: u8 = 4;
pub const CANCELLED: u8 = 5;

pub fn exit_code(result: &Result<RunOutcome, RunError>) -> u8 {
    match result {
        Ok(o) if o.clips_with_problems == 0 => WRITTEN,
        Ok(_) => WRITTEN_WITH_PROBLEMS,
        Err(RunError::SourceUnreadable(_)) => SOURCE_UNREADABLE,
        Err(RunError::Cancelled) => CANCELLED,
        Err(_) => FAILED,
    }
}

pub fn result_json(result: &Result<RunOutcome, RunError>) -> Value {
    match result {
        Ok(o) => json!({
            "ok": true,
            "report_dir": o.files.folder,
            "files": { "pdf": o.files.pdf, "html": o.files.html, "csv": o.files.csv },
            "clips": o.clips,
            "clips_with_problems": o.clips_with_problems,
            "summary": o.summary,
            "not_included": o.not_included,
            "unreadable": o.unreadable,
            "copies": o.copies,
            "copy_errors": o.copy_errors,
        }),
        Err(e) => error_json(&e.to_string(), exit_code(result)),
    }
}

pub fn error_json(error: &str, code: u8) -> Value {
    json!({ "ok": false, "error": error, "code": code })
}

/// Ctrl+C (and SIGTERM) stop the run; it then writes nothing.
fn cancel_flag() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let f = flag.clone();
    let _ = ctrlc::set_handler(move || f.store(true, Ordering::SeqCst));
    flag
}

fn print_progress(p: Progress) {
    match p {
        Progress::FilesFound(n) => eprintln!("Found {n} files"),
        Progress::Probed { done, total } => eprintln!("Probed {done}/{total}"),
        Progress::ClipsTotal(n) => eprintln!("{n} clips"),
        Progress::ClipDone {
            done,
            total,
            problems,
        } => eprintln!("Clip {done}/{total} ({problems} need attention)"),
    }
}

fn workers() -> Result<WorkerSpec, String> {
    WorkerSpec::current_exe().map_err(|e| format!("Luna couldn't find its own executable: {e}"))
}

pub fn report(args: ReportArgs) -> u8 {
    let json = args.json;
    let fail = |error: String, code: u8| {
        if json {
            println!("{}", error_json(&error, code));
        } else {
            eprintln!("{error}");
        }
        code
    };
    let request = match args.into_request(&Settings::load()) {
        Ok(r) => r,
        Err(e) => return fail(e, BAD_ARGUMENTS),
    };
    let workers = match workers() {
        Ok(w) => w,
        Err(e) => return fail(e, FAILED),
    };
    let cancel = cancel_flag();
    let result = run(&request, &workers, &print_progress, &cancel);
    let code = exit_code(&result);
    match (&result, json) {
        (_, true) => println!("{}", result_json(&result)),
        (Ok(o), false) => {
            println!("{}", o.files.pdf.display());
            println!("{}", o.summary);
        }
        (Err(e), false) => eprintln!("{e}"),
    }
    code
}

pub fn inspect_folders(args: InspectArgs) -> u8 {
    let workers = match workers() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            return FAILED;
        }
    };
    let cancel = cancel_flag();
    let result = inspect(&args.sources, &workers, &print_progress, &cancel);
    let code = match &result {
        Ok(_) => WRITTEN,
        Err(RunError::SourceUnreadable(_)) => SOURCE_UNREADABLE,
        Err(RunError::Cancelled) => CANCELLED,
        Err(_) => FAILED,
    };
    match (result, args.json) {
        (Ok(i), true) => println!("{}", inspection_json(&i)),
        (Ok(i), false) => print_inspection(&i),
        (Err(e), true) => println!("{}", error_json(&e.to_string(), code)),
        (Err(e), false) => eprintln!("{e}"),
    }
    code
}

pub fn inspection_json(i: &Inspection) -> Value {
    serde_json::to_value(i).unwrap_or(Value::Null)
}

fn print_inspection(i: &Inspection) {
    for cam in &i.cameras {
        for card in &cam.cards {
            println!(
                "{}  {}  {} clips  {}",
                cam.id, card.reel, card.clips, card.folder
            );
        }
    }
}
