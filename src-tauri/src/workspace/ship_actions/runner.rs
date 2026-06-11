use anyhow::{Context, Result};

use crate::forge::command::ForgeRunner;
use crate::models::{repos, sessions, settings};
use crate::service::{self, SendMessageParams};

use super::prompts::{build_agent_action_prompt, PromptContext};
use super::{
    DispatchedShipAction, OwnedSessionOverrides, WorkspaceShipActionKind, WorkspaceShipActionResult,
};

/// CLI/script-side `ForgeRunner` factory for ship actions. Same
/// behaviour + reasoning as `cli::github::forge_runner_for_workspace_cli`
/// — reads the bindings store from disk to detect a remote pin,
/// emits a one-line stderr notice when found, and always returns
/// a local runner because the CLI binary has no in-process runtime
/// registry to dispatch through. The GUI uses
/// `commands::forge_commands::merge_workspace_change_request`
/// instead (which resolves the runner from Tauri state), so the
/// notice only fires on real CLI use.
fn forge_runner_for_ship_action(workspace_id: &str) -> ForgeRunner {
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
                "warning: workspace is bound to remote runtime `{name}` — `helmor ship` \
                 runs forge actions against the laptop's `gh`/`glab` and does not route \
                 through the daemon. Use the GUI for binding-aware Ship on remote-bound \
                 workspaces."
            );
        }
    }
    ForgeRunner::local()
}

pub fn run_workspace_ship_action(
    workspace_ref: &str,
    action: WorkspaceShipActionKind,
) -> Result<WorkspaceShipActionResult> {
    let workspace_id = service::resolve_workspace_ref(workspace_ref)?;

    match action {
        WorkspaceShipActionKind::MergePr => {
            // Ship actions go through this code path from both the
            // GUI's Ship dropdown AND the `helmor ship` CLI. The CLI
            // has no long-lived runtime registry / SSH connection
            // pool to dispatch a remote `gh` through, so we degrade
            // to local with a stderr notice when a workspace is
            // bound to a non-`local` runtime — exactly the same
            // shape as `cli::github::forge_runner_for_workspace_cli`.
            // GUI invocations go through the binding-aware
            // `commands::forge_commands::merge_workspace_change_request`
            // Tauri command (which already resolves the runner from
            // the in-process registry), not this function, so the
            // notice only fires on actual CLI usage.
            let info = crate::forge::merge_workspace_change_request(
                &workspace_id,
                forge_runner_for_ship_action(&workspace_id),
            )?;
            Ok(WorkspaceShipActionResult::Direct {
                action,
                workspace_id,
                result: serde_json::to_value(info)?,
            })
        }
        WorkspaceShipActionKind::PullLatest => {
            let result = crate::workspaces::sync_workspace_with_target_branch(&workspace_id)?;
            Ok(WorkspaceShipActionResult::Direct {
                action,
                workspace_id,
                result: serde_json::to_value(result)?,
            })
        }
        WorkspaceShipActionKind::CommitAndPush
        | WorkspaceShipActionKind::CreatePr
        | WorkspaceShipActionKind::FixErrors
        | WorkspaceShipActionKind::ResolveConflicts => {
            let detail = service::get_workspace(&workspace_id)?;
            let repo_preferences = repos::load_repo_preferences(&detail.repo_id)?;
            let target_branch = detail
                .intended_target_branch
                .as_deref()
                .or(detail.default_branch.as_deref());
            let prompt = build_agent_action_prompt(
                action,
                PromptContext {
                    repo_preferences: &repo_preferences,
                    target_branch,
                    remote: detail.remote.as_deref(),
                    forge_provider: detail.forge_provider.as_deref(),
                },
            )?;
            let overrides = action_session_overrides(action)?;
            let action_kind = action
                .action_kind()
                .context("missing action kind for agent-dispatched ship action")?;
            let session = sessions::create_session(
                &workspace_id,
                Some(action_kind),
                None,
                overrides.as_create_session_overrides(),
            )?;

            let response = service::send_message(
                SendMessageParams {
                    workspace_ref: workspace_id.clone(),
                    session_id: Some(session.session_id.clone()),
                    prompt,
                    model: None,
                    permission_mode: Some("auto".to_string()),
                    linked_directories: Vec::new(),
                },
                &mut |_event| {},
            )?;

            Ok(WorkspaceShipActionResult::Dispatched {
                action,
                workspace_id,
                dispatch: DispatchedShipAction {
                    session_id: response.session_id,
                    provider: response.provider,
                    model: response.model,
                },
            })
        }
    }
}

fn action_session_overrides(action: WorkspaceShipActionKind) -> Result<OwnedSessionOverrides> {
    if !action.uses_action_model_override() {
        return Ok(OwnedSessionOverrides::default());
    }

    Ok(OwnedSessionOverrides {
        model: load_setting_trimmed("app.pr_model_id")?
            .or(load_setting_trimmed("app.default_model_id")?),
        effort_level: load_setting_trimmed("app.pr_effort")?
            .or(load_setting_trimmed("app.default_effort")?),
        fast_mode: load_setting_bool("app.pr_fast_mode")?
            .or(load_setting_bool("app.default_fast_mode")?),
    })
}

fn load_setting_trimmed(key: &str) -> Result<Option<String>> {
    Ok(settings::load_setting_value(key)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn load_setting_bool(key: &str) -> Result<Option<bool>> {
    Ok(
        settings::load_setting_value(key)?.and_then(|value| match value.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }),
    )
}
