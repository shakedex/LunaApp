//! `luna-cli mcp` driven the way Claude Desktop drives it: newline-delimited
//! JSON-RPC over the child's stdin and stdout.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const MXF: [u8; 16] = [
    0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02, 0x04, 0x00,
];

fn tree(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for rel in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, [&MXF[..], &[0; 32]].concat()).unwrap();
    }
    dir
}

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Value>,
    next: u64,
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_luna-cli"))
            .arg("mcp")
            .env("LUNA_WORKER_TEST_HOOKS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let _ = tx.send(v);
                }
            }
        });
        let mut s = Server {
            stdin: child.stdin.take(),
            child,
            lines,
            next: 0,
        };
        s.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" }
            }),
        );
        s.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        s
    }

    fn send(&mut self, v: Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let v = self
                .lines
                .recv_timeout(Duration::from_secs(60))
                .expect("server answered");
            if v["id"] == id {
                return v;
            }
        }
    }

    /// The tool's result object (`content`, `structuredContent`, `isError`).
    fn call(&mut self, tool: &str, args: Value) -> Value {
        let v = self.request("tools/call", json!({ "name": tool, "arguments": args }));
        v["result"].clone()
    }
}

/// The JSON a tool returned, from its text content.
fn payload(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap_or("null");
    serde_json::from_str(text).unwrap_or(Value::String(text.to_owned()))
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

#[test]
fn lists_four_tools() {
    let mut server = Server::start();
    let v = server.request("tools/list", json!({}));
    let mut names: Vec<_> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "cancel_report",
            "inspect_folders",
            "report_status",
            "start_report"
        ]
    );
}

#[test]
fn start_and_poll_to_done() {
    let src = tree(&["A_CAM/A001/A001C001_251020_R1AB.mxf"]);
    let out = tempfile::tempdir().unwrap();
    let mut server = Server::start();

    let inspected = payload(&server.call("inspect_folders", json!({ "paths": [s(src.path())] })));
    assert_eq!(inspected["cameras"][0]["id"], "A");

    let started = server.call(
        "start_report",
        json!({ "sources": [s(src.path())], "output": s(out.path()), "production": "MCP" }),
    );
    let id = payload(&started)["report_id"].as_str().unwrap().to_owned();
    let start = Instant::now();
    let status = loop {
        let status = payload(&server.call("report_status", json!({ "report_id": id })));
        if status["state"] != "queued" && status["state"] != "running" {
            break status;
        }
        assert!(start.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(status["state"], "done", "{status}");
    assert!(Path::new(status["result"]["files"]["pdf"].as_str().unwrap()).exists());
}

#[test]
fn unknown_report_id_is_a_tool_error() {
    let mut server = Server::start();
    let r = server.call("report_status", json!({ "report_id": "nope" }));
    assert_eq!(r["isError"], true, "{r}");
    // Still serving.
    let v = server.request("tools/list", json!({}));
    assert!(v["result"]["tools"].is_array());
}

#[test]
fn empty_sources_is_a_tool_error() {
    let mut server = Server::start();
    let r = server.call("start_report", json!({ "sources": [] }));
    assert_eq!(r["isError"], true, "{r}");
}

#[test]
fn eof_cancels_running_report() {
    let src = tree(&["A001/A001C001__hang__.mxf"]);
    let out = tempfile::tempdir().unwrap();
    let mut server = Server::start();
    let started = server.call(
        "start_report",
        json!({ "sources": [s(src.path())], "output": s(out.path()) }),
    );
    let id = payload(&started)["report_id"].as_str().unwrap().to_owned();
    // Wait until it is really running, then hang up.
    let start = Instant::now();
    while payload(&server.call("report_status", json!({ "report_id": id })))["state"] != "running" {
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(server.stdin.take());
    let start = Instant::now();
    loop {
        if server.child.try_wait().unwrap().is_some() {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "server still running after stdin closed"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
}
