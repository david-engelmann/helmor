//! `helmor github pr` — workspace-scoped PR operations. Auth lives in the
//! bundled `gh` CLI auth state; Helmor binds the right account
//! per-repo automatically.
//!
//! Routing: the CLI runs as a one-shot binary on the laptop, with no
//! long-lived runtime registry / SSH connection pool. So even for a
//! workspace bound to a remote runtime the `gh` call lands on the
//! laptop's `gh` (the same way `lookup_workspace_pr` already worked
//! before the binding-aware refactor in #33). When that happens we
//! print a one-line notice to stderr so the operator can see they're
//! not getting the same routing the GUI would give. A future
//! "CLI talks to the running daemon via the proxy socket" pass can
//! lift the limitation; this notice keeps the surprise out of the
//! mean time.

use anyhow::Result;

use crate::forge::command::ForgeRunner;
use crate::github_pr;
use crate::service;
use crate::ui_sync::UiMutationEvent;

use super::args::{Cli, GithubAction, GithubPrAction};
use super::{notify_ui_event, output};

/// CLI-side `ForgeRunner` factory. Reads the bindings store from
/// disk to detect whether the workspace is pinned to a non-`local`
/// runtime, emits a stderr notice when so, and returns a local
/// runner regardless. Falls back to local silently on any lookup
/// failure (corrupt JSON, missing data dir, etc.).
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
                "warning: workspace is bound to remote runtime `{name}` — the helmor CLI \
                 runs forge ops against the laptop's `gh` and does not route through the \
                 daemon. Use the GUI for binding-aware PR status / merge / close on \
                 remote-bound workspaces."
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
    let pr = github_pr::lookup_workspace_pr(&id, forge_runner_for_workspace_cli(&id))?;
    output::print(cli, &pr, |value| match value {
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

fn pr_status(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    let status =
        github_pr::lookup_workspace_pr_action_status(&id, forge_runner_for_workspace_cli(&id))?;
    output::print(cli, &status, |s| format!("{s:?}"))
}

fn pr_merge(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    let pr = github_pr::merge_workspace_pr(&id, forge_runner_for_workspace_cli(&id))?;
    notify_ui_event(UiMutationEvent::WorkspaceChangeRequestChanged {
        workspace_id: id.clone(),
    });
    output::print(cli, &pr, |value| match value {
        Some(pr) => format!("Merged PR #{}: {}", pr.number, pr.url),
        None => "No PR to merge.".to_string(),
    })
}

fn pr_close(workspace_ref: &str, cli: &Cli) -> Result<()> {
    let id = service::resolve_workspace_ref(workspace_ref)?;
    let pr = github_pr::close_workspace_pr(&id, forge_runner_for_workspace_cli(&id))?;
    notify_ui_event(UiMutationEvent::WorkspaceChangeRequestChanged {
        workspace_id: id.clone(),
    });
    output::print(cli, &pr, |value| match value {
        Some(pr) => format!("Closed PR #{}: {}", pr.number, pr.url),
        None => "No PR to close.".to_string(),
    })
}
