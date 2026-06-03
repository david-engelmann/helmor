//! Client side of the desktop's CLI IPC.
//!
//! The `helmor` CLI is a separate process from the desktop; it doesn't
//! carry the `RuntimeRegistry` / `WorkspaceRuntimeBindings` state the
//! GUI uses to route forge ops through bound runtimes. When the desktop
//! IS running, the CLI dispatches the op over the existing `ui_sync`
//! Unix socket, the desktop runs it against the right runtime, and
//! ships back the typed result.
//!
//! When the desktop ISN'T running, [`dispatch_via_ipc`] returns
//! `Ok(None)` and the caller falls back to the local-only path (with
//! the pre-existing one-line stderr warning that the laptop's `gh`
//! is doing the work).

use std::io::{BufRead, BufReader, Write};

use anyhow::{Context, Result};

use crate::ui_sync::{self, CliRpcEnvelope, CliRpcRequest, CliRpcResponse};

/// Try to dispatch a forge op through the running desktop's IPC
/// socket. Returns:
///   - `Ok(Some(response))` when the desktop responded (the caller
///     should use the response's `ok` / `error` to decide what to
///     print, and short-circuit the local fallback).
///   - `Ok(None)` when the desktop isn't running (no socket file, or
///     connect refused). Caller falls back to the local path.
///   - `Err(_)` for unexpected transport failures (socket present
///     but read/write failed mid-request). Caller likely wants to
///     surface the error rather than silently fall back.
pub fn dispatch_via_ipc(request: CliRpcRequest) -> Result<Option<CliRpcResponse>> {
    #[cfg(unix)]
    {
        let socket_path = ui_sync::socket_path()?;
        if !socket_path.exists() {
            return Ok(None);
        }
        // `notify_running_app` treats a connect failure as "desktop
        // not running" (returns false) rather than an error. Mirror
        // that here: a stale socket file shouldn't bubble up.
        let mut stream = match std::os::unix::net::UnixStream::connect(&socket_path) {
            Ok(stream) => stream,
            Err(_) => return Ok(None),
        };

        let envelope = CliRpcEnvelope::new(request);
        let payload =
            serde_json::to_string(&envelope).context("Failed to serialize CLI RPC envelope")?;
        stream
            .write_all(payload.as_bytes())
            .context("Failed to write CLI RPC payload")?;
        stream
            .write_all(b"\n")
            .context("Failed to terminate CLI RPC payload")?;
        stream.flush().context("Failed to flush CLI RPC payload")?;

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .context("Failed to read CLI RPC response")?;

        let response: CliRpcResponse = serde_json::from_str(response_line.trim())
            .with_context(|| format!("Failed to parse CLI RPC response: {response_line}"))?;
        Ok(Some(response))
    }

    #[cfg(not(unix))]
    {
        let _ = request;
        Ok(None)
    }
}
