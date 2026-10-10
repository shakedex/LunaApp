//! Writes the test fixture report as PDF, HTML and CSV for a visual check:
//! `cargo run -p luna-report --example sample -- <output dir>`

#[path = "../tests/common/mod.rs"]
mod common;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir)?;
    let report = common::sample_report();
    std::fs::write(dir.join("sample.pdf"), luna_report::write_pdf(&report)?)?;
    std::fs::write(dir.join("sample.html"), luna_report::write_html(&report)?)?;
    std::fs::write(dir.join("sample.csv"), luna_report::write_csv(&report))?;
    println!(
        "wrote sample.pdf, sample.html, sample.csv to {}",
        dir.display()
    );
    Ok(())
}
