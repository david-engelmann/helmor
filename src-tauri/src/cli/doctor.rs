//! `helmor doctor` — one-shot diagnostic snapshot for local + remote state.
//!
//! Read-only by design. The worst it can do is hold an SSH session
//! open for a few seconds while running probes. Used by on-call to
//! answer "what's broken with my remote" without ssh-ing in manually
//! and checking five different things.
//!
//! Scope (v1):
//! - Local: data directory exists + size, desktop running (ui-sync socket present).
//! - Per remote (SSH): reachability with elapsed ms, daemon binary
//!   exists, last 5 lines of `daemon.log` tagged by ERROR/WARN signal.
//!
//! Non-SSH transports (`Command` variant) get a notice but no probe —
//! the operator manages those out-of-band.
//!
//! Output:
//! - Default: ANSI-tagged human-readable rows with a ✓ / ⚠ / ✗ glyph.
//! - `--json`: machine-readable [`DoctorReport`].
//! - Exit 0 = all green or warnings only; exit 1 = at least one ✗.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use anyhow::Result;
use serde::Serialize;

use crate::cli::args::Cli;
use crate::remote::connection::RuntimeConnectionConfig;
use crate::remote::persistence;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub sections: Vec<DoctorSection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorSection {
    pub title: String,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
}

impl DoctorCheck {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: DoctorStatus::Ok,
            detail: detail.into(),
        }
    }
    fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: DoctorStatus::Warn,
            detail: detail.into(),
        }
    }
    fn error(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: DoctorStatus::Error,
            detail: detail.into(),
        }
    }
}

pub fn run(cli: &Cli) -> Result<()> {
    let report = build_report();

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human(&report);
    }

    if report.has_errors() {
        std::process::exit(1);
    }
    Ok(())
}

fn build_report() -> DoctorReport {
    let mut sections = vec![DoctorSection {
        title: "Local".into(),
        checks: local_checks(),
    }];

    if let Ok(data_dir) = crate::data_dir::data_dir() {
        let runtimes = persistence::load(&data_dir);
        for entry in &runtimes.entries {
            let (title, checks) = match &entry.config {
                RuntimeConnectionConfig::Local { .. } => continue,
                RuntimeConnectionConfig::Ssh { host, .. } => (
                    format!("Remote: {} (ssh {host})", entry.name),
                    remote_ssh_checks(host),
                ),
                RuntimeConnectionConfig::Command { argv } => (
                    format!(
                        "Remote: {} (command {})",
                        entry.name,
                        argv.first().cloned().unwrap_or_default()
                    ),
                    vec![DoctorCheck::warn(
                        "transport",
                        "non-SSH transport — doctor only probes SSH today; check the daemon manually on the remote",
                    )],
                ),
            };
            sections.push(DoctorSection { title, checks });
        }
    }

    DoctorReport { sections }
}

impl DoctorReport {
    fn has_errors(&self) -> bool {
        self.sections
            .iter()
            .any(|s| s.checks.iter().any(|c| c.status == DoctorStatus::Error))
    }
    fn has_warns(&self) -> bool {
        self.sections
            .iter()
            .any(|s| s.checks.iter().any(|c| c.status == DoctorStatus::Warn))
    }
}

fn local_checks() -> Vec<DoctorCheck> {
    let mut checks = vec![];

    match crate::data_dir::data_dir() {
        Ok(dir) => {
            let size_mb = dir_size_mb(&dir);
            checks.push(DoctorCheck::ok(
                "data directory",
                format!("{} ({size_mb:.1} MB)", dir.display()),
            ));

            let sock = dir.join("run").join("ui-sync.sock");
            if sock.exists() {
                checks.push(DoctorCheck::ok(
                    "desktop running",
                    format!("ui-sync socket present at {}", sock.display()),
                ));
            } else {
                checks.push(DoctorCheck::warn(
                    "desktop running",
                    format!(
                        "no ui-sync socket at {} — desktop probably not running",
                        sock.display()
                    ),
                ));
            }
        }
        Err(err) => {
            checks.push(DoctorCheck::error("data directory", format!("{err}")));
        }
    }

    checks
}

fn remote_ssh_checks(host: &str) -> Vec<DoctorCheck> {
    let mut checks = vec![];

    let t0 = Instant::now();
    let ssh_result = ssh_probe(host, &["echo", "ok"]);
    let elapsed_ms = t0.elapsed().as_millis();

    match ssh_result {
        Ok(output) if output.status.success() => {
            checks.push(DoctorCheck::ok(
                "ssh reachable",
                format!("connected in {elapsed_ms}ms"),
            ));
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let first_two = stderr
                .trim()
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join(" / ");
            checks.push(DoctorCheck::error(
                "ssh reachable",
                format!("ssh failed (exit {}): {first_two}", output.status),
            ));
            return checks; // skip downstream probes when SSH is dead
        }
        Err(err) => {
            checks.push(DoctorCheck::error(
                "ssh reachable",
                format!("ssh spawn failed: {err}"),
            ));
            return checks;
        }
    }

    match ssh_probe(host, &["ls", "-la", "$HOME/.helmor/server/helmor-server"]) {
        Ok(output) if output.status.success() => {
            let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
            checks.push(DoctorCheck::ok("daemon binary", line));
        }
        Ok(_) => {
            checks.push(DoctorCheck::error(
                "daemon binary",
                "$HOME/.helmor/server/helmor-server not found — the desktop's install gate should drop it there on first connect",
            ));
        }
        Err(err) => {
            checks.push(DoctorCheck::error(
                "daemon binary",
                format!("probe failed: {err}"),
            ));
        }
    }

    match ssh_probe(host, &["tail", "-5", "$HOME/.helmor/server/daemon.log"]) {
        Ok(output) if output.status.success() => {
            let log = String::from_utf8_lossy(&output.stdout).to_string();
            let trimmed = log.trim();
            let has_err = trimmed.contains("ERROR");
            let has_warn = trimmed.contains("WARN");
            let detail = if trimmed.is_empty() {
                "log file empty".to_string()
            } else {
                format!("last 5 lines:\n{}", indent(trimmed, "    "))
            };
            let check = if has_err {
                DoctorCheck::error("daemon log tail", detail)
            } else if has_warn {
                DoctorCheck::warn("daemon log tail", detail)
            } else {
                DoctorCheck::ok("daemon log tail", detail)
            };
            checks.push(check);
        }
        Ok(_) => {
            checks.push(DoctorCheck::warn(
                "daemon log tail",
                "$HOME/.helmor/server/daemon.log not found or unreadable",
            ));
        }
        Err(err) => {
            checks.push(DoctorCheck::warn(
                "daemon log tail",
                format!("probe failed: {err}"),
            ));
        }
    }

    checks
}

fn ssh_probe(host: &str, argv: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        "-o",
        "ServerAliveInterval=10",
        host,
    ])
    .args(argv);
    cmd.output()
}

fn print_human(report: &DoctorReport) {
    println!("helmor doctor\n");
    for section in &report.sections {
        println!("{}", section.title);
        for check in &section.checks {
            let symbol = match check.status {
                DoctorStatus::Ok => "✓",
                DoctorStatus::Warn => "⚠",
                DoctorStatus::Error => "✗",
            };
            // First line on the same row as the check name; any
            // additional lines (e.g. log tail) get an extra leading
            // indent so they visually nest under the check.
            let mut lines = check.detail.split('\n');
            let first = lines.next().unwrap_or("");
            println!("  {symbol} {}: {first}", check.name);
            for rest in lines {
                println!("    {rest}");
            }
        }
        println!();
    }
    if report.has_errors() {
        println!("Summary: errors present — see ✗ rows above.");
    } else if report.has_warns() {
        println!("Summary: warnings only — see ⚠ rows above.");
    } else {
        println!("Summary: all checks green.");
    }
}

fn dir_size_mb(path: &PathBuf) -> f64 {
    fn walk(p: &PathBuf) -> u64 {
        let mut total = 0_u64;
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = entry.metadata() else { continue };
                if meta.is_file() {
                    total = total.saturating_add(meta.len());
                } else if meta.is_dir() {
                    total = total.saturating_add(walk(&path));
                }
            }
        }
        total
    }
    walk(path) as f64 / (1024.0 * 1024.0)
}

fn indent(s: &str, prefix: &str) -> String {
    s.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_has_errors_when_any_check_is_error() {
        let r = DoctorReport {
            sections: vec![DoctorSection {
                title: "t".into(),
                checks: vec![
                    DoctorCheck::ok("a", "fine"),
                    DoctorCheck::error("b", "boom"),
                ],
            }],
        };
        assert!(r.has_errors());
        assert!(!r.has_warns());
    }

    #[test]
    fn report_has_warns_when_only_warns_present() {
        let r = DoctorReport {
            sections: vec![DoctorSection {
                title: "t".into(),
                checks: vec![DoctorCheck::warn("a", "soft"), DoctorCheck::ok("b", "fine")],
            }],
        };
        assert!(!r.has_errors());
        assert!(r.has_warns());
    }

    #[test]
    fn report_is_clean_when_all_ok() {
        let r = DoctorReport {
            sections: vec![DoctorSection {
                title: "t".into(),
                checks: vec![DoctorCheck::ok("a", "fine"), DoctorCheck::ok("b", "fine")],
            }],
        };
        assert!(!r.has_errors());
        assert!(!r.has_warns());
    }

    #[test]
    fn indent_prefixes_every_line() {
        assert_eq!(indent("a\nb\nc", "  "), "  a\n  b\n  c");
        assert_eq!(indent("", "  "), "");
        assert_eq!(indent("single", ">>"), ">>single");
    }

    #[test]
    fn check_constructors_set_status() {
        assert_eq!(DoctorCheck::ok("n", "d").status, DoctorStatus::Ok);
        assert_eq!(DoctorCheck::warn("n", "d").status, DoctorStatus::Warn);
        assert_eq!(DoctorCheck::error("n", "d").status, DoctorStatus::Error);
    }
}
