//! `helmor github pr` — workspace-scoped PR operations. Auth lives in
//! the bundled `gh` CLI auth state; Helmor binds the right account
//! per-repo automatically.
//!
//! # Routing
//!
//! The CLI is a separate process from the desktop, so it doesn't
//! carry the `RuntimeRegistry` / `WorkspaceRuntimeBindings` state the
//! GUI uses to route forge ops through bound runtimes. Two cases:
//!
//!   1. **Desktop is running.** The CLI dispatches the op over the
//!      `ui_sync` Unix socket (see `cli::ipc_client`); the desktop
//!      runs it against the workspace's bound runtime — same path
//!      `merge_workspace_change_request` follows from the GUI — and
//!      ships back the typed result. Silent good case.
//!
//!   2. **Desktop is NOT running.** The CLI falls back to its own
//!      laptop `gh`. If the workspace is bound to a non-`local`
//!      runtime, we emit a one-line stderr warning so the operator
//!      knows the laptop binary did the work (it's authenticated
//!      against the laptop's account, not the container's). Local-
//!      bound workspaces stay silent.

use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::forge::command::ForgeRunner;
use crate::forge::ChangeRequestInfo;
use crate::github_pr;
use crate::service;
use crate::ui_sync::{CliRpcRequest, CliRpcResponse, UiMutationEvent};

use super::args::{Cli, GithubAction, GithubPrAction};
use super::ipc_client::dispatch_via_ipc;
use super::{notify_ui_event, output};

/// Local-only `ForgeRunner` factory + a one-line stderr warning when
/// the workspace is bound to a non-`local` runtime. Used as the
/// fallback path when the desktop IPC isn't available; the IPC
/// fast-path skips this entirely so the warning never fires for the
/// happy case.
fn forge_runner_for_workspace_cli(workspace_id: &str) -> ForgeRunner {
    let bound = crate::models::workspaces::load_workspace_runtime_name(workspace_id)
        .ok()
        .flatten()
        .or_else(|| match crate::data_dir::data_dir() {
            Ok(dir) => {
                crate::remote::WorkspaceRuntimeBindings::load_from_disk(&dir).lookup(workspace_id)
            }
            Err(_) => None,
        });
    if let Some(name) = bound {
        if name != "local" {
            eprintln!(
                "warning: workspace is bound to remote runtime `{name}` and the Helmor \
                 desktop isn't running — falling back to the laptop's `gh`. Open the \
                 desktop app to route forge ops through the bound runtime."
            );
        }
    }
    ForgeRunner::local()
}

pub fn dispatch(action: &GithubAction, cli: &Cli) -> Result<()> {
    match action {
        GithubAction::Pr { action } => pr_dispatch(action, cli),
    }
}

fn pr_dispatch(action: &GithubPrAction, cli: &Cli) -> Result<()> {
    match action {
        GithubPrAction::Show { workspace_ref } => pr_show(workspace_ref, cli),
        GithubPrAction::Status { workspace_ref } => pr_status(workspace_ref, cli),
        GithubPrAction::Merge { workspace_ref } => pr_merge(workspace_ref, cli),
        GithubPrAction::Close { workspace_ref } => pr_close(workspace_ref, cli),
    }
}

fn pr_show(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    if let Some(response) = dispatch_via_ipc(CliRpcRequest::GithubPrShow {
        workspace_ref: workspace_ref.to_string(),
    })? {
        return print_ipc_pr_show(cli, response);
    }
    let pr = github_pr::lookup_workspace_pr(&id, forge_runner_for_workspace_cli(&id))?;
    print_pr_show(cli, &pr)
}

fn pr_status(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    if let Some(response) = dispatch_via_ipc(CliRpcRequest::GithubPrStatus {
        workspace_ref: workspace_ref.to_string(),
    })? {
        return print_ipc_pr_status(cli, response);
    }
    let status =
        github_pr::lookup_workspace_pr_action_status(&id, forge_runner_for_workspace_cli(&id))?;
    output::print(cli, &status, |s| format!("{s:?}"))
}

fn pr_merge(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    if let Some(response) = dispatch_via_ipc(CliRpcRequest::GithubPrMerge {
        workspace_ref: workspace_ref.to_string(),
    })? {
        notify_ui_event(UiMutationEvent::WorkspaceChangeRequestChanged {
            workspace_id: id.clone(),
        });
        return print_ipc_pr_merge(cli, response);
    }
    let pr = github_pr::merge_workspace_pr(&id, forge_runner_for_workspace_cli(&id))?;
    notify_ui_event(UiMutationEvent::WorkspaceChangeRequestChanged {
        workspace_id: id.clone(),
    });
    print_pr_merge(cli, &pr)
}

fn pr_close(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    if let Some(response) = dispatch_via_ipc(CliRpcRequest::GithubPrClose {
        workspace_ref: workspace_ref.to_string(),
    })? {
        notify_ui_event(UiMutationEvent::WorkspaceChangeRequestChanged {
            workspace_id: id.clone(),
        });
        return print_ipc_pr_close(cli, response);
    }
    let pr = github_pr::close_workspace_pr(&id, forge_runner_for_workspace_cli(&id))?;
    notify_ui_event(UiMutationEvent::WorkspaceChangeRequestChanged {
        workspace_id: id.clone(),
    });
    print_pr_close(cli, &pr)
}

// ── shared output formatters ──────────────────────────────────────

fn print_pr_show(cli: &Cli, pr: &Option<ChangeRequestInfo>) -> Result<()> {
    output::print(cli, pr, |value| match value {
        Some(pr) => format!(
            "#{} {}\nURL:    {}\nState:  {}{}",
            pr.number,
            pr.title,
            pr.url,
            pr.state,
            if pr.is_merged { " (merged)" } else { "" },
        ),
        None => "No PR linked to this workspace.".to_string(),
    })
}

fn print_pr_merge(cli: &Cli, pr: &Option<ChangeRequestInfo>) -> Result<()> {
    output::print(cli, pr, |value| match value {
        Some(pr) => format!("Merged PR #{}: {}", pr.number, pr.url),
        None => "No PR to merge.".to_string(),
    })
}

fn print_pr_close(cli: &Cli, pr: &Option<ChangeRequestInfo>) -> Result<()> {
    output::print(cli, pr, |value| match value {
        Some(pr) => format!("Closed PR #{}: {}", pr.number, pr.url),
        None => "No PR to close.".to_string(),
    })
}

// ── IPC response handlers — same printers, value deserialized ─────

fn print_ipc_pr_show(cli: &Cli, response: CliRpcResponse) -> Result<()> {
    let pr: Option<ChangeRequestInfo> = decode_ipc_result(response)?;
    print_pr_show(cli, &pr)
}

fn print_ipc_pr_status(cli: &Cli, response: CliRpcResponse) -> Result<()> {
    let status: crate::forge::ForgeActionStatus = decode_ipc_result(response)?;
    output::print(cli, &status, |s| format!("{s:?}"))
}

fn print_ipc_pr_merge(cli: &Cli, response: CliRpcResponse) -> Result<()> {
    let pr: Option<ChangeRequestInfo> = decode_ipc_result(response)?;
    print_pr_merge(cli, &pr)
}

fn print_ipc_pr_close(cli: &Cli, response: CliRpcResponse) -> Result<()> {
    let pr: Option<ChangeRequestInfo> = decode_ipc_result(response)?;
    print_pr_close(cli, &pr)
}

/// Convert a `CliRpcResponse` into either the typed payload or an
/// error that propagates through the CLI's standard error surface.
/// `T: Serialize` is needed for the eventual `output::print` call;
/// `T: DeserializeOwned` for parsing the JSON the desktop sent back.
fn decode_ipc_result<T>(response: CliRpcResponse) -> Result<T>
where
    T: DeserializeOwned + Serialize,
{
    if !response.ok {
        let message = response
            .error
            .unwrap_or_else(|| "desktop reported an unknown error".to_string());
        bail!(message);
    }
    let value = response
        .result
        .ok_or_else(|| anyhow!("desktop response was ok but carried no result payload"))?;
    serde_json::from_value(value).map_err(|err| anyhow!(err))
}
