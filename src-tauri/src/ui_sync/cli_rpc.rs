//! Wire types for the desktop's CLI IPC.
//!
//! When the `helmor` CLI binary runs against a workspace bound to a
//! remote runtime, the forge ops (`gh`/`glab`) have to route through
//! that runtime — otherwise they hit the laptop's `gh`, which is
//! authenticated against the laptop's account and operates on the
//! laptop's git checkout (no branches, no PRs, nothing useful).
//!
//! The CLI process doesn't carry the `RuntimeRegistry` /
//! `WorkspaceRuntimeBindings` state the GUI has — those live in the
//! running Tauri app. So instead of duplicating dispatch in the CLI,
//! the CLI asks the desktop to do the forge op on its behalf over
//! the existing `ui_sync` Unix socket. The desktop runs the request
//! through the same code path the GUI uses for "Merge PR" / "Close
//! PR" buttons, then ships the serialized result back.
//!
//! When the desktop isn't running, [`CliRpcRequest`] is never sent
//! and the CLI falls back to the laptop's `gh` (with a one-line
//! warning matching pre-IPC behaviour). See `cli::github` for the
//! call sites.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Versioned envelope around a CLI RPC request, written one per line
/// onto the `ui_sync` socket. The version field's unique name
/// (`cliRpcVersion`) lets the socket listener distinguish RPC frames
/// from the existing `UiMutationEnvelope` by JSON-shape sniffing —
/// no top-level discriminator needed, no break for existing
/// `notify_running_app` callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliRpcEnvelope {
    pub cli_rpc_version: u8,
    pub request: CliRpcRequest,
}

impl CliRpcEnvelope {
    pub const VERSION: u8 = 1;

    pub fn new(request: CliRpcRequest) -> Self {
        Self {
            cli_rpc_version: Self::VERSION,
            request,
        }
    }
}

/// Forge operations the CLI can ask the desktop to dispatch. Each
/// variant carries the workspace identifier (id or directory name —
/// both shapes resolve via `service::resolve_workspace_ref`) the
/// operation should target. The desktop builds a `ForgeRunner`
/// scoped to the workspace's bound runtime and calls the same
/// `forge::github::*` function the GUI's Tauri command would.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CliRpcRequest {
    /// `helmor github pr show <workspace>` — read-only PR lookup.
    GithubPrShow { workspace_ref: String },
    /// `helmor github pr status <workspace>` — read-only PR action
    /// status (checks, reviews, mergeable state).
    GithubPrStatus { workspace_ref: String },
    /// `helmor github pr merge <workspace>` — destructive.
    GithubPrMerge { workspace_ref: String },
    /// `helmor github pr close <workspace>` — destructive.
    GithubPrClose { workspace_ref: String },
}

/// Result returned to the CLI for a single RPC call. `result` carries
/// the same JSON the GUI's Tauri command for this op would produce
/// (already serialized so the CLI doesn't have to import every
/// `ChangeRequestInfo`/`ForgeActionStatus` shape). `error` carries an
/// operator-actionable message when `ok` is `false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliRpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CliRpcResponse {
    pub fn ok(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn envelope_round_trips() {
        let envelope = CliRpcEnvelope::new(CliRpcRequest::GithubPrStatus {
            workspace_ref: "ws-1".into(),
        });
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: CliRpcEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, envelope);
    }

    #[test]
    fn envelope_uses_distinct_version_field() {
        // The version field's name lets the socket listener tell
        // CLI RPC frames apart from UiMutationEnvelope frames by
        // shape alone. Don't rename it without also teaching the
        // listener about the new shape.
        let envelope = CliRpcEnvelope::new(CliRpcRequest::GithubPrShow {
            workspace_ref: "ws".into(),
        });
        let json = serde_json::to_value(&envelope).unwrap();
        assert!(
            json.get("cliRpcVersion").is_some(),
            "envelope must serialize `cliRpcVersion`: {json}",
        );
        assert!(
            json.get("version").is_none(),
            "must not collide with UiMutationEnvelope's `version`: {json}",
        );
    }

    #[test]
    fn response_ok_includes_result() {
        let resp = CliRpcResponse::ok(json!({ "merged": true }));
        let parsed: CliRpcResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert!(parsed.ok);
        assert_eq!(parsed.result, Some(json!({ "merged": true })));
        assert!(parsed.error.is_none());
    }

    #[test]
    fn response_err_carries_message() {
        let resp = CliRpcResponse::err("workspace not found");
        assert!(!resp.ok);
        assert!(resp.result.is_none());
        assert_eq!(resp.error.as_deref(), Some("workspace not found"));
    }

    #[test]
    fn request_serializes_with_kind_discriminator() {
        let req = CliRpcRequest::GithubPrMerge {
            workspace_ref: "ws".into(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(
            json.get("kind").and_then(|v| v.as_str()),
            Some("githubPrMerge"),
        );
    }
}
