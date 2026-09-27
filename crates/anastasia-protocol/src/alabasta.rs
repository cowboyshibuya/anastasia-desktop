//! Alabasta workspace binding: what a session is working on, and what context
//! Anastasia actually managed to compile for it.
//!
//! Alabasta is the context and coordination plane; Anastasia is the local
//! execution plane. Anastasia requests, caches and renders context — it never
//! decides which organizational decisions are authoritative. That stays in
//! Alabasta's Context Runtime, so the MCP transport, the web UI and Anastasia
//! cannot drift apart in what they consider in scope.
//!
//! Every type here is optional on the entity that owns it. `None` means "not
//! connected", and a session with `None` launches its agent byte-identically to
//! one built before this integration existed.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ProviderKind;

/// A connected Alabasta account and the workspace it is acting in.
///
/// The refresh token is **not** here — it lives in the keychain, keyed by
/// [`Self::account_key`]. This struct is written to the daemon's settings file
/// in plain text, so it must never carry a secret.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct AlabastaConnection {
    /// The Convex **site** origin, e.g. `https://<deployment>.convex.site`.
    /// Note this is the HTTP-actions origin, not the `.convex.cloud` API one.
    pub site_url: String,
    /// Where the browser signs in — the Next.js app origin.
    pub app_url: String,
    /// Shown in Settings so a user can tell which account is connected.
    pub account_label: String,
    pub workspace_id: String,
    pub workspace_slug: String,
    pub workspace_name: String,
}

impl AlabastaConnection {
    /// Keychain account for this connection's refresh token. Keyed by site and
    /// user so two deployments, or two accounts, do not overwrite each other.
    pub fn account_key(&self) -> String {
        format!("{}|{}", self.site_url, self.account_label)
    }

    pub fn is_configured(&self) -> bool {
        !self.site_url.is_empty() && !self.workspace_id.is_empty()
    }
}

/// What a session needs in order to compile its Alabasta context at launch.
///
/// Carries the connection rather than a token: the daemon reads the refresh
/// token from the keychain itself, so a credential never rides the wire and
/// never reaches an agent process.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlabastaLaunchRequest {
    pub connection: AlabastaConnection,
    pub task_id: String,
    pub task_identifier: String,
    pub task_title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
}

/// The Alabasta product a local project is bound to.
///
/// Tasks are numbered per product (`ALB-482`, minted from the product's own
/// counter), so binding at the product level makes a project's task list exactly
/// the right set. Projects are optional on tasks and would hide product-level
/// work, which is why the binding is not at project level.
///
/// The workspace is deliberately absent: it comes from the connection, so a
/// binding cannot disagree with the account that is actually signed in.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlabastaBinding {
    pub product_id: String,
    /// The identifier prefix, e.g. `ALB`.
    pub product_identifier: String,
    pub product_name: String,
}

/// How Alabasta context reached a session's agent.
///
/// Ordered strongest first. `Push` means the compiled package was in the model's
/// context before its first token; `Bridge` means the agent can pull more
/// through Anastasia's brokered channel. Most supported providers get both —
/// this records the weakest link, since that is what a user needs to know.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AlabastaIntegration {
    /// Context injected at launch and a brokered pull channel registered.
    PushAndBridge,
    /// Context injected at launch; this runtime exposes no MCP seam.
    PushOnly,
    #[default]
    Unsupported,
}

impl AlabastaIntegration {
    /// What a provider is capable of when Alabasta is connected.
    ///
    /// A property of the runtime, not of the machine or the account, so the
    /// Settings page can render the table without launching anything.
    pub fn for_provider(provider: ProviderKind) -> Self {
        match provider {
            // Every one of these has a process-scoped MCP seam Anastasia already
            // drives for its own QuickJS server.
            ProviderKind::Claude
            | ProviderKind::Codex
            | ProviderKind::OpenCode
            | ProviderKind::Pi
            | ProviderKind::Grok => Self::PushAndBridge,
            // These have an instruction channel but no MCP registration path, so
            // they can be told things but cannot ask for more.
            ProviderKind::Amp | ProviderKind::Cursor | ProviderKind::DeepSeek => Self::PushOnly,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::PushAndBridge => tr!("alabasta.integration_push_and_bridge"),
            Self::PushOnly => tr!("alabasta.integration_push_only"),
            Self::Unsupported => tr!("alabasta.integration_unsupported"),
        }
    }
}

/// How ready Alabasta considered a task's context to be.
///
/// Mirrors `contextQuality.readiness` from the Context Runtime rather than
/// re-deriving it: Alabasta owns that judgement. Unknown covers a package from a
/// runtime version that reports something Anastasia does not recognise, which
/// must not be silently rendered as "ready".
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ContextReadiness {
    Ready,
    NeedsContext,
    NotReady,
    Blocked,
    #[default]
    Unknown,
}

impl ContextReadiness {
    pub fn from_id(id: &str) -> Self {
        match id.trim().to_ascii_lowercase().as_str() {
            "ready" => Self::Ready,
            "needs_context" => Self::NeedsContext,
            "not_ready" => Self::NotReady,
            "blocked" => Self::Blocked,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Ready => tr!("alabasta.readiness_ready"),
            Self::NeedsContext => tr!("alabasta.readiness_needs_context"),
            Self::NotReady => tr!("alabasta.readiness_not_ready"),
            Self::Blocked => tr!("alabasta.readiness_blocked"),
            Self::Unknown => tr!("alabasta.readiness_unknown"),
        }
    }
}

/// One compiled context source, kept so the UI can answer "why does the agent
/// know this?" without refetching.
///
/// `authority` is Alabasta's own provenance rank (1 = security policy, 12 =
/// inferred); lower binds harder. Anastasia displays it and never reorders by
/// its own judgement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContextSource {
    /// The `alabasta://` resource URI, when the item is drillable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    pub title: String,
    /// Runtime-defined kind (`decision`, `rule`, `document`, …). Deliberately a
    /// string: Alabasta may add kinds without Anastasia needing a release.
    pub kind: String,
    pub authority: String,
    pub authority_rank: u32,
}

/// What the Alabasta integration did for one session, captured at launch.
///
/// Captured once and persisted, never recomputed, so the badge keeps reporting
/// the context the agent was actually given even after the binding changes or
/// the app restarts. `message` is operator-facing diagnostics only — never
/// prompt text, never repository contents, never workspace content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlabastaStatus {
    pub provider: ProviderKind,
    pub integration: AlabastaIntegration,
    pub readiness: ContextReadiness,
    /// Sources compiled into the launch prompt, most binding first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ContextSource>,
    /// Approximate tokens the package reported, for the context meter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_tokens: Option<u32>,
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl AlabastaStatus {
    /// Context compiled and delivered.
    pub fn active(
        provider: ProviderKind,
        integration: AlabastaIntegration,
        readiness: ContextReadiness,
        sources: Vec<ContextSource>,
        approximate_tokens: Option<u32>,
    ) -> Self {
        Self {
            provider,
            integration,
            readiness,
            sources,
            approximate_tokens,
            healthy: true,
            message: None,
        }
    }

    /// Context was requested but could not be compiled or delivered. The session
    /// still runs; the status says so rather than implying the agent is informed.
    pub fn failed(provider: ProviderKind, message: impl Into<String>) -> Self {
        Self {
            provider,
            integration: AlabastaIntegration::Unsupported,
            readiness: ContextReadiness::Unknown,
            sources: Vec::new(),
            approximate_tokens: None,
            healthy: false,
            message: Some(message.into()),
        }
    }
}

/// The Alabasta task a session is executing, and what context it received.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlabastaSessionBinding {
    pub task_id: String,
    /// The human identifier, e.g. `ALB-482`.
    pub task_identifier: String,
    pub task_title: String,
    pub status: AlabastaStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_declares_an_integration_level() {
        // Settings renders this for all providers, so a newly added runtime must
        // be classified deliberately rather than defaulting to Unsupported.
        let push_only = ProviderKind::ALL
            .into_iter()
            .filter(|provider| {
                AlabastaIntegration::for_provider(*provider) == AlabastaIntegration::PushOnly
            })
            .collect::<Vec<_>>();
        assert_eq!(
            push_only,
            vec![
                ProviderKind::Amp,
                ProviderKind::Cursor,
                ProviderKind::DeepSeek
            ]
        );
        // Nothing is Unsupported once connected: every runtime can at least be
        // told what the task is.
        assert!(
            ProviderKind::ALL.into_iter().all(|provider| {
                AlabastaIntegration::for_provider(provider) != AlabastaIntegration::Unsupported
            }),
            "a provider cannot receive context at all"
        );
    }

    #[test]
    fn readiness_parsing_never_upgrades_an_unknown_value() {
        assert_eq!(ContextReadiness::from_id("ready"), ContextReadiness::Ready);
        assert_eq!(
            ContextReadiness::from_id("needs_context"),
            ContextReadiness::NeedsContext
        );
        // A future runtime value must not be rendered as ready.
        assert_eq!(
            ContextReadiness::from_id("partially_ready"),
            ContextReadiness::Unknown
        );
        assert_eq!(ContextReadiness::from_id(""), ContextReadiness::Unknown);
    }

    #[test]
    fn a_failed_status_never_claims_context() {
        let status = AlabastaStatus::failed(ProviderKind::Claude, "token expired");
        assert!(!status.healthy);
        assert!(status.sources.is_empty());
        assert_eq!(status.readiness, ContextReadiness::Unknown);
        assert_eq!(status.integration, AlabastaIntegration::Unsupported);
    }
}
