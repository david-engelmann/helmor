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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_dir::lock_test_env;
    use crate::ui_sync::CliRpcResponse;
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    /// Stand up a one-shot mock listener at the desktop socket path
    /// and dispatch a real `CliRpcRequest` through it. Proves the
    /// envelope round-trips intact over the wire — request shape +
    /// response shape stay in sync between client and server.
    ///
    /// This is the integration test the gap audit flagged: the
    /// per-shape unit tests on `cli_rpc.rs` cover JSON round-trip,
    /// but only this exercise actually runs bytes across a Unix
    /// socket the way the production CLI would.
    #[test]
    fn dispatch_via_ipc_round_trips_envelope_over_unix_socket() {
        let _guard = lock_test_env();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HELMOR_DATA_DIR", dir.path());
        crate::data_dir::ensure_directory_structure().unwrap();

        let socket = crate::ui_sync::socket_path().unwrap();
        // The listener mock: accept one connection, read one line,
        // parse as a CliRpcEnvelope, write back a canned response.
        let listener = UnixListener::bind(&socket).unwrap();
        let captured_request: std::sync::Arc<std::sync::Mutex<Option<CliRpcRequest>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let captured_clone = std::sync::Arc::clone(&captured_request);

        let listener_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            {
                let mut reader = BufReader::new(&mut stream);
                reader.read_line(&mut line).unwrap();
            }
            let envelope: crate::ui_sync::CliRpcEnvelope = serde_json::from_str(&line).unwrap();
            captured_clone.lock().unwrap().replace(envelope.request);

            let response = CliRpcResponse::ok(json!({
                "number": 42,
                "url": "https://example.com/pr/42",
                "state": "open",
                "title": "test",
                "isMerged": false,
            }));
            let body = serde_json::to_string(&response).unwrap();
            stream.write_all(body.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
            stream.flush().unwrap();
        });

        let request = CliRpcRequest::GithubPrStatus {
            workspace_ref: "ws-1".into(),
        };
        let response = dispatch_via_ipc(request.clone())
            .expect("dispatch_via_ipc transport-level success")
            .expect("desktop responded (Ok(Some))");
        listener_thread.join().unwrap();

        // Server side: the listener parsed the same request shape
        // back out of the envelope, byte-for-byte.
        assert_eq!(captured_request.lock().unwrap().as_ref(), Some(&request));
        // Client side: the response landed with the canned payload
        // intact through the wire path.
        assert!(response.ok);
        assert!(response.error.is_none());
        let result = response.result.expect("response carries result");
        assert_eq!(result.get("number").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(
            result.get("url").and_then(|v| v.as_str()),
            Some("https://example.com/pr/42"),
        );
    }

    #[test]
    fn dispatch_via_ipc_returns_none_when_socket_is_missing() {
        let _guard = lock_test_env();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HELMOR_DATA_DIR", dir.path());
        // Don't create the socket — the production "desktop not
        // running" case. Must short-circuit to `Ok(None)` so the
        // CLI falls back to the laptop's `gh` cleanly.
        let result = dispatch_via_ipc(CliRpcRequest::GithubPrShow {
            workspace_ref: "ws-1".into(),
        })
        .unwrap();
        assert!(result.is_none(), "no socket → Ok(None), got {result:?}");
    }

    #[test]
    fn dispatch_via_ipc_propagates_server_error_response() {
        let _guard = lock_test_env();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HELMOR_DATA_DIR", dir.path());
        crate::data_dir::ensure_directory_structure().unwrap();

        let socket = crate::ui_sync::socket_path().unwrap();
        let listener = UnixListener::bind(&socket).unwrap();
        let listener_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            {
                let mut reader = BufReader::new(&mut stream);
                reader.read_line(&mut line).unwrap();
            }
            let response = CliRpcResponse::err("workspace not found: ws-doesnt-exist");
            let body = serde_json::to_string(&response).unwrap();
            stream.write_all(body.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
            stream.flush().unwrap();
        });

        let response = dispatch_via_ipc(CliRpcRequest::GithubPrMerge {
            workspace_ref: "ws-doesnt-exist".into(),
        })
        .unwrap()
        .unwrap();
        listener_thread.join().unwrap();

        // Wire-level error response must survive deserialization with
        // its message intact — the CLI's `decode_ipc_result` reads
        // this exact field to bail with an operator-actionable message.
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("workspace not found: ws-doesnt-exist"),
        );
    }
}
