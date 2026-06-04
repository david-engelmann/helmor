//! Server-side handler for [`CliRpcRequest`] frames received on the
//! `ui_sync` socket. Resolves the workspace's bound runtime via the
//! same `RuntimeRegistry` + `WorkspaceRuntimeBindings` the GUI uses,
//! calls the matching `forge::github::*` function, and serializes
//! the result into a [`CliRpcResponse`].
//!
//! Errors that come back as a Rust `Result::Err` (workspace not
//! found, forge op returned an `anyhow::Error`) are converted to
//! `CliRpcResponse::err(...)` so the CLI always gets a typed reply.
//!
//! Panics in a downstream forge function are caught here with
//! `std::panic::catch_unwind` and turned into `CliRpcResponse::err`
//! so a single bad request can't tear down the socket listener
//! thread. The CLI always gets a typed reply — even when the
//! underlying op panics — and the desktop stays responsive on the
//! socket for subsequent requests.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use super::cli_rpc::{CliRpcRequest, CliRpcResponse};
use crate::commands::forge_commands::forge_runner_for_workspace;
use crate::github_pr;
use crate::remote::registry::RuntimeRegistry;
use crate::remote::workspace_bindings::WorkspaceRuntimeBindings;
use crate::service;

/// Dispatch a single `CliRpcRequest` against the desktop's live
/// state. Always returns a `CliRpcResponse` — even on error or panic
/// — so the CLI can `match` on `ok` without juggling a transport-vs-
/// business distinction, and the socket listener thread never gets
/// torn down by an exception in a downstream forge function.
pub fn dispatch_cli_rpc<R: Runtime>(app: &AppHandle<R>, request: CliRpcRequest) -> CliRpcResponse {
    let workspace_ref = workspace_ref_for_logging(&request).to_string();
    run_catching_panic(&workspace_ref, || dispatch_inner(app, request))
}

fn dispatch_inner<R: Runtime>(app: &AppHandle<R>, request: CliRpcRequest) -> CliRpcResponse {
    match request {
        CliRpcRequest::GithubPrShow { workspace_ref } => {
            run_with_workspace(app, &workspace_ref, |id, runtime_state| {
                let runner = forge_runner_for_workspace(
                    id,
                    &runtime_state.registry,
                    &runtime_state.bindings,
                );
                let pr = github_pr::lookup_workspace_pr(id, runner)?;
                Ok(serde_json::to_value(pr)?)
            })
        }
        CliRpcRequest::GithubPrStatus { workspace_ref } => {
            run_with_workspace(app, &workspace_ref, |id, runtime_state| {
                let runner = forge_runner_for_workspace(
                    id,
                    &runtime_state.registry,
                    &runtime_state.bindings,
                );
                let status = github_pr::lookup_workspace_pr_action_status(id, runner)?;
                Ok(serde_json::to_value(status)?)
            })
        }
        CliRpcRequest::GithubPrMerge { workspace_ref } => {
            run_with_workspace(app, &workspace_ref, |id, runtime_state| {
                let runner = forge_runner_for_workspace(
                    id,
                    &runtime_state.registry,
                    &runtime_state.bindings,
                );
                let pr = github_pr::merge_workspace_pr(id, runner)?;
                Ok(serde_json::to_value(pr)?)
            })
        }
        CliRpcRequest::GithubPrClose { workspace_ref } => {
            run_with_workspace(app, &workspace_ref, |id, runtime_state| {
                let runner = forge_runner_for_workspace(
                    id,
                    &runtime_state.registry,
                    &runtime_state.bindings,
                );
                let pr = github_pr::close_workspace_pr(id, runner)?;
                Ok(serde_json::to_value(pr)?)
            })
        }
    }
}

fn workspace_ref_for_logging(request: &CliRpcRequest) -> &str {
    match request {
        CliRpcRequest::GithubPrShow { workspace_ref }
        | CliRpcRequest::GithubPrStatus { workspace_ref }
        | CliRpcRequest::GithubPrMerge { workspace_ref }
        | CliRpcRequest::GithubPrClose { workspace_ref } => workspace_ref.as_str(),
    }
}

/// Bundle the runtime-state references the forge dispatchers need.
/// Extracted so each `match` arm above doesn't repeat the
/// `app.state::<_>()` boilerplate four times.
struct RuntimeState {
    registry: Arc<RuntimeRegistry>,
    bindings: Arc<WorkspaceRuntimeBindings>,
}

/// Resolve `workspace_ref` to a canonical id, snapshot the runtime
/// state, and hand both to `op`. Any `Result::Err` along the way
/// (workspace not found, forge op returned `Err`) becomes a
/// `CliRpcResponse::err`. Panics are caught at the outer
/// `dispatch_cli_rpc` boundary, not here.
fn run_with_workspace<R, F>(app: &AppHandle<R>, workspace_ref: &str, op: F) -> CliRpcResponse
where
    R: Runtime,
    F: FnOnce(&str, &RuntimeState) -> anyhow::Result<Value>,
{
    let id = match service::resolve_workspace_ref(workspace_ref) {
        Ok(id) => id,
        Err(err) => return CliRpcResponse::err(format!("{err:#}")),
    };
    let registry = app.state::<Arc<RuntimeRegistry>>();
    let bindings = app.state::<Arc<WorkspaceRuntimeBindings>>();
    let state = RuntimeState {
        registry: Arc::clone(&registry),
        bindings: Arc::clone(&bindings),
    };
    match op(&id, &state) {
        Ok(value) => CliRpcResponse::ok(value),
        Err(err) => CliRpcResponse::err(format!("{err:#}")),
    }
}

/// Run `dispatch` under `catch_unwind`. The closure returns a
/// `CliRpcResponse` directly — any panic anywhere inside it
/// (workspace resolve, state access, forge call) is caught and
/// turned into a typed error reply so the socket listener thread
/// stays alive for subsequent requests.
///
/// Factored out so the catch-unwind shape stays testable with a
/// synthetic closure that doesn't need a Tauri `AppHandle`.
fn run_catching_panic<F>(workspace_ref: &str, dispatch: F) -> CliRpcResponse
where
    F: FnOnce() -> CliRpcResponse,
{
    match std::panic::catch_unwind(AssertUnwindSafe(dispatch)) {
        Ok(response) => response,
        Err(payload) => {
            let detail = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else {
                "<non-string panic payload>".to_string()
            };
            tracing::error!(
                workspace_ref = workspace_ref,
                panic = %detail,
                "cli_rpc_dispatch: forge op panicked; replying with error and keeping listener alive"
            );
            CliRpcResponse::err(format!("forge op panicked: {detail}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_with_string_payload_becomes_response_err() {
        let response = run_catching_panic("ws-abc", || panic!("boom: simulated forge crash"));
        assert!(!response.ok);
        let msg = response.error.expect("error message on panic");
        assert!(msg.contains("forge op panicked"), "got: {msg}");
        assert!(msg.contains("boom: simulated forge crash"), "got: {msg}");
        assert!(response.result.is_none());
    }

    #[test]
    fn panic_with_static_str_payload_becomes_response_err() {
        let response = run_catching_panic("ws-def", || {
            std::panic::panic_any("static-str panic payload")
        });
        assert!(!response.ok);
        let msg = response.error.expect("error message on panic");
        assert!(msg.contains("static-str panic payload"), "got: {msg}");
    }

    #[test]
    fn panic_with_non_string_payload_falls_back_to_placeholder() {
        let response = run_catching_panic("ws-ghi", || std::panic::panic_any(42_i32));
        assert!(!response.ok);
        let msg = response.error.expect("error message on panic");
        assert!(msg.contains("<non-string panic payload>"), "got: {msg}");
    }

    #[test]
    fn ok_response_passes_through_unchanged() {
        let response = run_catching_panic("ws-jkl", || {
            CliRpcResponse::ok(serde_json::json!({"hello": "world"}))
        });
        assert!(response.ok);
        assert_eq!(response.result, Some(serde_json::json!({"hello": "world"})));
        assert!(response.error.is_none());
    }

    #[test]
    fn err_response_passes_through_without_panic_prefix() {
        let response = run_catching_panic("ws-mno", || CliRpcResponse::err("plain dispatch error"));
        assert!(!response.ok);
        let msg = response.error.expect("error message on Err");
        assert_eq!(msg, "plain dispatch error");
        assert!(
            !msg.contains("forge op panicked"),
            "non-panic errors must not be tagged as panics: {msg}"
        );
    }
}
