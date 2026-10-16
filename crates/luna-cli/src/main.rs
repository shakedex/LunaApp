//! `luna-cli`: Luna without a window, for scripts and AI agents.
//!
//! - `luna-cli report <folders…> [options]` writes a report and exits.
//! - `luna-cli inspect <folders…>` lists cameras, cards and clips.
//! - `luna-cli mcp` serves the same over MCP (stdio).
//!
//! It is also its own worker process (`--worker`), like `Luna.exe`.

mod headless;

use std::process::ExitCode;

use clap::Parser;
use luna_core::cli::{Cli, Command};

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--worker") {
        luna_worker::worker_main();
        return ExitCode::SUCCESS;
    }
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(if e.use_stderr() {
                headless::BAD_ARGUMENTS
            } else {
                0
            });
        }
    };
    ExitCode::from(match cli.command {
        Command::Report(args) => headless::report(args),
        Command::Inspect(args) => headless::inspect_folders(args),
        Command::Mcp => {
            eprintln!("MCP isn't available in this build");
            headless::FAILED
        }
    })
}
