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
//! Panics in a downstream forge function are NOT caught here — they
//! propagate up to the socket listener thread and tear it down. This
//! matches the existing `notify_running_app` listener's behavior and
//! is acceptable because the forge functions exercised here are also
//! reachable via the GUI's Tauri commands and don't panic in
//! production. If that changes, wrap the `op(...)` call below in
//! `std::panic::catch_unwind` and turn the caught panic into a
//! `CliRpcResponse::err`.

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
/// state. Always returns a `CliRpcResponse` — even on error — so the
/// CLI can `match` on `ok` without juggling a transport-vs-business
/// distinction.
pub fn dispatch_cli_rpc<R: Runtime>(app: &AppHandle<R>, request: CliRpcRequest) -> CliRpcResponse {
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

/// Bundle the runtime-state references the forge dispatchers need.
/// Extracted so each `match` arm above doesn't repeat the
/// `app.state::<_>()` boilerplate four times.
struct RuntimeState {
    registry: Arc<RuntimeRegistry>,
    bindings: Arc<WorkspaceRuntimeBindings>,
}

/// Resolve `workspace_ref` to a canonical id, snapshot the runtime
/// state, and hand both to `op`. Any error along the way (workspace
/// not found, forge op blew up) becomes a `CliRpcResponse::err`.
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
