//! MCP server over stdio (`luna-cli mcp`): lets an AI agent inspect folders
//! and run reports. Reports run in the background (see [`crate::jobs`]); the
//! agent polls `report_status`. When the client hangs up, running reports are
//! cancelled and leave nothing behind.

use std::path::PathBuf;
use std::sync::Arc;

use luna_core::cli::{ReportArgs, Theme};
use luna_core::inspect::inspect;
use luna_core::run::WorkerSpec;
use luna_core::settings::Settings;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::headless::inspection_json;
use crate::jobs::Jobs;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InspectParams {
    /// Folders to scan, recursively (absolute paths).
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Field {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartParams {
    /// Folders to scan, recursively (absolute paths). One report covers all of them.
    pub sources: Vec<String>,
    /// Only these cameras: camera ids from inspect_folders, or bare letters such as "A".
    #[serde(default)]
    pub cameras: Vec<String>,
    pub production: Option<String>,
    pub company: Option<String>,
    pub author: Option<String>,
    /// Logo file: PNG, JPEG or SVG.
    pub logo: Option<String>,
    /// Header fields, printed after the user's saved fields.
    #[serde(default)]
    pub fields: Vec<Field>,
    /// Folder the report folder is written into (default: the user's setting).
    pub output: Option<String>,
    /// Report folder name (default: production name and run date).
    pub name: Option<String>,
    /// "light" or "dark".
    pub theme: Option<String>,
    /// Print clip warnings and unreadable files in the report (default: the user's setting).
    #[serde(default)]
    pub problems: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportIdParams {
    /// The id start_report returned.
    pub report_id: String,
}

#[derive(Clone)]
pub struct LunaServer {
    jobs: Arc<Jobs>,
    workers: WorkerSpec,
}

fn ok(v: Value) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![ContentBlock::text(
        v.to_string(),
    )]))
}

fn tool_error(message: impl Into<String>) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![ContentBlock::text(
        message.into(),
    )]))
}

#[tool_router]
impl LunaServer {
    pub fn new(workers: WorkerSpec) -> Self {
        Self {
            jobs: Arc::new(Jobs::new(workers.clone())),
            workers,
        }
    }

    pub fn jobs(&self) -> Arc<Jobs> {
        self.jobs.clone()
    }

    #[tool(
        description = "List the cameras, cards (reels) and clips in folders, from metadata only \
(no frames decoded). Returns camera ids, model, serial, and per card: reel, folder, clip count, \
formats, first/last recording date and size in bytes."
    )]
    async fn inspect_folders(
        &self,
        Parameters(p): Parameters<InspectParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if p.paths.is_empty() {
            return tool_error("Give at least one folder in `paths`.");
        }
        let roots: Vec<PathBuf> = p.paths.into_iter().map(PathBuf::from).collect();
        let workers = self.workers.clone();
        let jobs = self.jobs.clone();
        // Waits for any running report: they share the worker pool.
        let result = tokio::task::spawn_blocking(move || {
            jobs.exclusive(|stop| inspect(&roots, &workers, &|_| {}, stop))
        })
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        match result {
            Ok(i) => ok(inspection_json(&i)),
            Err(e) => tool_error(e.to_string()),
        }
    }

    #[tool(
        description = "Start one report (PDF, HTML and CSV) from one or more folders, optionally \
limited to some cameras. Returns a report_id at once; poll report_status for progress and the \
result."
    )]
    async fn start_report(
        &self,
        Parameters(p): Parameters<StartParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if p.sources.is_empty() {
            return tool_error("Give at least one folder in `sources`.");
        }
        let theme = match p.theme.as_deref().map(str::to_ascii_lowercase).as_deref() {
            None => None,
            Some("light") => Some(Theme::Light),
            Some("dark") => Some(Theme::Dark),
            Some(other) => return tool_error(format!("theme \"{other}\" isn't light or dark")),
        };
        let args = ReportArgs {
            sources: p.sources.into_iter().map(PathBuf::from).collect(),
            cameras: p.cameras,
            production: p.production,
            company: p.company,
            author: p.author,
            logo: p.logo.map(PathBuf::from),
            fields: p.fields.into_iter().map(|f| (f.label, f.value)).collect(),
            no_saved_fields: false,
            output: p.output.map(PathBuf::from),
            name: p.name,
            theme,
            problems: p.problems,
            json: true,
        };
        match args.into_request(&Settings::load()) {
            Ok(request) => ok(json!({ "report_id": self.jobs.start(request) })),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "The state of a report: queued, running, done, failed or cancelled, with \
progress counts, and once it has ended the result (report folder, file paths, clip counts, \
summary) or the error."
    )]
    async fn report_status(
        &self,
        Parameters(p): Parameters<ReportIdParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.jobs.status(&p.report_id) {
            Some(s) => ok(s.to_json()),
            None => tool_error(format!("No report with id \"{}\".", p.report_id)),
        }
    }

    #[tool(description = "Cancel a queued or running report. A cancelled report writes no files.")]
    async fn cancel_report(
        &self,
        Parameters(p): Parameters<ReportIdParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.jobs.cancel(&p.report_id) {
            Some(state) => ok(json!({ "state": state.as_str() })),
            None => tool_error(format!("No report with id \"{}\".", p.report_id)),
        }
    }
}

#[tool_handler(
    name = "luna",
    instructions = "Luna makes camera reports (PDF, HTML, CSV) from camera original \
folders. Call inspect_folders first to see the cameras, cards and clips in the folders. Then \
choose report groups from the user's request (one per camera, per shoot day, or one for the \
whole project) and call start_report once per group, passing the camera ids from \
inspect_folders in `cameras` to limit a report to those cameras. Reports run one at a time in \
the background: poll report_status until the state is done, failed or cancelled. The result \
lists the report files."
)]
impl ServerHandler for LunaServer {}

/// Serves until the client closes stdin, then cancels every report and waits
/// for them to stop.
pub fn serve(workers: WorkerSpec) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let server = LunaServer::new(workers);
    let jobs = server.jobs();
    let served = runtime.block_on(async move {
        let running = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| e.to_string())?;
        running.waiting().await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    });
    jobs.shutdown();
    // An inspect still running sees the stop flag; don't wait for it here.
    runtime.shutdown_background();
    served
}
