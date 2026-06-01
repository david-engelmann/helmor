//! Bounded subprocess execution for forge CLI integrations.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::bundled;

#[derive(Debug, Clone)]
pub(crate) struct CommandOutput {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) success: bool,
    pub(crate) status: Option<i32>,
}

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn run_command<I, S>(program: &str, args: I) -> std::io::Result<CommandOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_command_with_timeout(program, args, DEFAULT_COMMAND_TIMEOUT)
}

/// Prefer the bundled binary over whatever is on PATH.
fn resolve_program(program: &str) -> PathBuf {
    bundled::bundled_path_for(program).unwrap_or_else(|| PathBuf::from(program))
}

pub(crate) fn run_command_with_timeout<I, S>(
    program: &str,
    args: I,
    timeout: Duration,
) -> std::io::Result<CommandOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_command_full::<_, _, &str, &str>(program, args, timeout, &[])
}

/// `run_command` with caller-supplied environment overrides. Each `(name,
/// value)` pair is applied on top of the inherited environment AFTER the
/// default `NO_COLOR=1` etc., so callers can set per-spawn `GH_TOKEN`
/// without leaking it back into the parent process.
pub(crate) fn run_command_with_env<I, S, K, V>(
    program: &str,
    args: I,
    env: &[(K, V)],
) -> std::io::Result<CommandOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    run_command_full(program, args, DEFAULT_COMMAND_TIMEOUT, env)
}

fn run_command_full<I, S, K, V>(
    program: &str,
    args: I,
    timeout: Duration,
    env: &[(K, V)],
) -> std::io::Result<CommandOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let resolved = resolve_program(program);
    let mut command = Command::new(&resolved);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Force monochrome output so JSON parsing isn't broken by ANSI
        // colour codes when the user's environment sets CLICOLOR_FORCE=1
        // or similar.
        .env("NO_COLOR", "1")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("FORCE_COLOR");
    for (name, value) in env {
        command.env(name, value);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let child = command.spawn()?;
    let child_pid = child.id();
    let (tx, rx) = mpsc::channel();
    let waiter = thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let output = match rx.recv_timeout(timeout) {
        Ok(result) => {
            let _ = waiter.join();
            result?
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            kill_process(child_pid);
            let _ = waiter.join();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("`{program}` timed out after {timeout:?}"),
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = waiter.join();
            return Err(std::io::Error::other(format!(
                "`{program}` waiter thread exited unexpectedly"
            )));
        }
    };

    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
        status: output.status.code(),
    })
}

#[cfg(unix)]
fn kill_process(child_pid: u32) {
    unsafe {
        libc::kill(-(child_pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process(child_pid: u32) {
    let pid = child_pid.to_string();
    let _ = Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .status();
}

/// Public escape hatch for the remote-runner seam. Runs the requested
/// forge CLI on the LOCAL filesystem (i.e., this desktop process) and
/// hands back its captured output in a wire-compatible shape so the
/// `RemoteRuntime::forge_exec` trait method can dispatch through one
/// uniform surface. The remote impl (`RemoteSshRuntime`) wires
/// `forge.exec` through the JSON-RPC client instead; both paths
/// agree on the return shape so callers don't care which runtime
/// resolved the work.
///
/// `timeout_ms = None` keeps the existing `DEFAULT_COMMAND_TIMEOUT`
/// behaviour — most internal callers don't override it.
pub fn forge_run_local<I, S, K, V>(
    program: &str,
    args: I,
    env: &[(K, V)],
    timeout_ms: Option<u64>,
) -> std::io::Result<ForgeLocalResult>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let timeout = timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT);
    let output = run_command_full(program, args, timeout, env)?;
    Ok(ForgeLocalResult {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.status,
    })
}

/// Wire-compatible mirror of `super::remote::methods::ForgeExecResult`.
/// Kept in this module (the local impl's home) so callers depending
/// only on `crate::forge` don't have to reach into `remote::methods`.
#[derive(Debug, Clone)]
pub struct ForgeLocalResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Runtime-aware wrapper around the local `gh`/`glab` shell-outs. A
/// workspace-scoped forge op resolves the workspace's bound runtime
/// once at the call's entry point and threads this through the call
/// chain — every downstream `gh api ...` invocation flows through
/// `ForgeRunner::run_cli_with_login` (or `::run_cli`) and lands on
/// the bound runtime instead of the laptop.
///
/// `None` keeps the legacy local-only behaviour for non-workspace
/// surfaces (account listing, inbox enumeration, CLI auth status —
/// laptop-level operations that depend on the desktop's own auth).
#[derive(Clone, Default)]
pub(crate) struct ForgeRunner {
    runtime: Option<std::sync::Arc<dyn crate::remote::runtime::RemoteRuntime>>,
}

impl std::fmt::Debug for ForgeRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForgeRunner")
            .field(
                "runtime",
                &self.runtime.as_ref().map(|_| "<dyn RemoteRuntime>"),
            )
            .finish()
    }
}

impl ForgeRunner {
    /// Stay-local runner. Used by the existing forge surfaces (and
    /// by tests / callers without a workspace context) — every
    /// `gh`/`glab` call resolves through the laptop's vendored
    /// binary.
    pub(crate) fn local() -> Self {
        Self::default()
    }

    /// Bind to a workspace's resolved runtime. When the runtime is
    /// remote, every dispatched `gh`/`glab` call routes through
    /// `forge.exec` so the container's authenticated CLI does the
    /// work; the local-runtime case stays byte-for-byte equivalent
    /// to `local()`.
    pub(crate) fn with_runtime(
        runtime: std::sync::Arc<dyn crate::remote::runtime::RemoteRuntime>,
    ) -> Self {
        Self {
            runtime: Some(runtime),
        }
    }

    /// `true` when this runner routes through the local
    /// `forge::command` path (no bound runtime, or bound runtime is
    /// `LocalRuntime`). Used by callers that need to decide whether
    /// passing a host-side `GH_TOKEN` makes sense — the container's
    /// `gh` has its OWN auth state, so we drop the laptop's token
    /// when routing remote.
    pub(crate) fn is_local(&self) -> bool {
        match &self.runtime {
            None => true,
            Some(rt) => matches!(
                rt.runtime_health().map(|h| h.kind),
                Ok(crate::remote::runtime::RuntimeKind::Local),
            ),
        }
    }

    /// Dispatch a plain `gh`/`glab` invocation through the bound
    /// runtime (remote-routed when set, local-fallback otherwise).
    /// Each arg is sent verbatim; environment is empty.
    pub(crate) fn run<I, S>(&self, program: &str, args: I) -> std::io::Result<CommandOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_env::<_, _, &str, &str>(program, args, &[])
    }

    /// `Self::run` with caller-supplied environment overrides
    /// (e.g. `GH_TOKEN` for the local path). On a remote runtime
    /// the env is forwarded to the daemon — useful for picking a
    /// specific authenticated identity on the *host*; the remote
    /// daemon's `gh` typically ignores `GH_TOKEN` and uses its own
    /// `gh auth` state, but we forward the env anyway so callers
    /// don't have to branch on routing.
    pub(crate) fn run_with_env<I, S, K, V>(
        &self,
        program: &str,
        args: I,
        env: &[(K, V)],
    ) -> std::io::Result<CommandOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let runtime = match &self.runtime {
            Some(rt) => rt,
            None => return run_command_with_env(program, args, env),
        };
        let args: Vec<String> = args
            .into_iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect();
        let env: Vec<crate::remote::methods::ForgeExecEnv> = env
            .iter()
            .map(|(k, v)| crate::remote::methods::ForgeExecEnv {
                name: k.as_ref().to_string_lossy().into_owned(),
                value: v.as_ref().to_string_lossy().into_owned(),
            })
            .collect();
        match runtime.forge_exec(crate::remote::methods::ForgeExecParams {
            program: program.to_string(),
            args,
            env,
            timeout_ms: None,
        }) {
            Ok(result) => Ok(CommandOutput {
                stdout: result.stdout,
                stderr: result.stderr,
                success: matches!(result.exit_code, Some(0)),
                status: result.exit_code,
            }),
            // The daemon-side handler returns its anyhow chain as a
            // JSON-RPC error; surface it through io::Error so callers
            // (which already have `.with_context` chains for the local
            // path) keep their existing error-handling shape.
            Err(err) => Err(std::io::Error::other(format!("forge.exec failed: {err:#}"))),
        }
    }
}

pub(crate) fn command_detail(output: &CommandOutput) -> String {
    let stderr = output.stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    let stdout = output.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }
    match output.status {
        Some(code) => format!("command exited with status {code}"),
        None => "command exited unsuccessfully".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn run_command_with_timeout_kills_stalled_command() {
        let started_at = std::time::Instant::now();
        let error =
            run_command_with_timeout("/bin/sh", ["-c", "sleep 2"], Duration::from_millis(100))
                .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started_at.elapsed() < Duration::from_secs(1));
    }
}
