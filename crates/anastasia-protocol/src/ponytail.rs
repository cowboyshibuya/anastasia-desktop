//! Ponytail harness policy: the mode a session runs under, and how it got there.
//!
//! Ponytail (<https://github.com/DietrichGebert/ponytail>, MIT) makes an agent
//! prefer the simplest solution that actually solves the problem. Anastasia
//! applies it per session rather than by installing anything into the user's
//! global agent configuration.
//!
//! There is exactly one representation of "off": [`Option::None`]. The settings
//! UI has both an enabled switch and an intensity picker, but they collapse to a
//! single `Option<PonytailMode>` before anything else sees them, so no code
//! downstream can encounter a disabled-but-Ultra state.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ProviderKind;

/// The vendored Ponytail release.
///
/// Read from upstream's own manifest at build time, so the version cannot drift
/// from the files actually shipped and there is nothing to bump by hand on an
/// upgrade. Compiled in rather than read from disk, because the Settings page
/// renders this and `render` must never touch the filesystem.
pub fn vendored_version() -> &'static str {
    const MANIFEST: &str = include_str!("../../../resources/ponytail/.claude-plugin/plugin.json");
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        serde_json::from_str::<serde_json::Value>(MANIFEST)
            .ok()
            .and_then(|manifest| Some(manifest.get("version")?.as_str()?.to_owned()))
            .unwrap_or_default()
    })
}

/// How hard Ponytail pushes. Mirrors upstream's intensity levels, minus `off`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PonytailMode {
    Lite,
    #[default]
    Full,
    Ultra,
}

impl PonytailMode {
    pub const ALL: [Self; 3] = [Self::Lite, Self::Full, Self::Ultra];

    /// The lowercase name upstream uses in `PONYTAIL_DEFAULT_MODE` and in the
    /// mode-keyed rows of its ruleset. Parsing and rendering must agree with
    /// upstream exactly, so both directions live here.
    pub fn id(self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Full => "full",
            Self::Ultra => "ultra",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "lite" => Some(Self::Lite),
            "full" => Some(Self::Full),
            "ultra" => Some(Self::Ultra),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Lite => tr!("ponytail.mode_lite"),
            Self::Full => tr!("ponytail.mode_full"),
            Self::Ultra => tr!("ponytail.mode_ultra"),
        }
    }
}

/// Which mechanism actually delivered Ponytail to the runtime.
///
/// Ordered strongest first, matching the ladder Anastasia climbs per provider:
/// a native plugin or extension owns the agent's own lifecycle, instruction
/// injection only reaches the model's context, and unsupported means the
/// provider's launch path was left exactly as it would be with Ponytail off.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PonytailIntegration {
    Native,
    Instructions,
    #[default]
    Unsupported,
}

impl PonytailIntegration {
    /// The mechanism Anastasia uses for a provider when Ponytail is on.
    ///
    /// A property of the runtime rather than of the machine, so the Settings
    /// page can show the ladder without launching anything. One caveat it
    /// cannot see: Claude drops to `Instructions` when Node.js is missing, which
    /// only the session's own status reports.
    pub fn for_provider(provider: ProviderKind) -> Self {
        match provider {
            // Session-scoped plugin and extension hosts: real lifecycle hooks.
            ProviderKind::Claude | ProviderKind::Pi => Self::Native,
            // An instruction channel, but no plugin Anastasia may install
            // without writing to the user's global agent configuration.
            ProviderKind::Codex | ProviderKind::OpenCode | ProviderKind::Grok => Self::Instructions,
            ProviderKind::Amp | ProviderKind::Cursor | ProviderKind::DeepSeek => Self::Unsupported,
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Native => tr!("ponytail.integration_native"),
            Self::Instructions => tr!("ponytail.integration_instructions"),
            Self::Unsupported => tr!("ponytail.integration_unsupported"),
        }
    }
}

/// What Ponytail actually did for one session, recorded at launch.
///
/// This is the whole diagnostic surface (enabled, requested and effective mode,
/// version, runtime, mechanism, health, error). It is captured when the driver
/// starts and never recomputed, so it cannot drift from what the agent was
/// really given. `message` carries operator-facing diagnostics only — never
/// prompt text and never anything read out of the user's repository.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PonytailStatus {
    pub provider: ProviderKind,
    /// The mode the user asked for. Present even when the integration failed,
    /// so the UI can say "Full, but not applied" rather than silently showing off.
    pub mode: PonytailMode,
    pub integration: PonytailIntegration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PonytailStatus {
    /// Ponytail is switched on and reached the agent through `integration`.
    pub fn active(
        provider: ProviderKind,
        mode: PonytailMode,
        integration: PonytailIntegration,
        version: Option<String>,
    ) -> Self {
        Self {
            provider,
            mode,
            integration,
            version,
            healthy: true,
            message: None,
        }
    }

    /// Ponytail was requested but could not be applied. The session still runs;
    /// the status says so instead of claiming an integration that never happened.
    pub fn failed(provider: ProviderKind, mode: PonytailMode, message: impl Into<String>) -> Self {
        Self {
            provider,
            mode,
            integration: PonytailIntegration::Unsupported,
            version: None,
            healthy: false,
            message: Some(message.into()),
        }
    }

    /// This provider has no channel Anastasia can apply Ponytail through. Not an
    /// error: nothing was attempted, so nothing failed.
    pub fn unsupported(provider: ProviderKind, mode: PonytailMode) -> Self {
        Self {
            provider,
            mode,
            integration: PonytailIntegration::Unsupported,
            version: None,
            healthy: true,
            message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vendored_version_is_readable() {
        // Guards the vendored manifest itself: a bad copy would otherwise show
        // up as a blank version in Settings rather than a failing build.
        let version = vendored_version();
        assert!(!version.is_empty(), "vendored plugin.json has no version");
        assert!(version.starts_with(char::is_numeric), "{version:?}");
    }

    #[test]
    fn mode_ids_round_trip_through_upstream_spelling() {
        for mode in PonytailMode::ALL {
            assert_eq!(PonytailMode::from_id(mode.id()), Some(mode));
        }
    }

    #[test]
    fn mode_parsing_tolerates_case_and_padding_but_rejects_off() {
        assert_eq!(PonytailMode::from_id(" ULTRA "), Some(PonytailMode::Ultra));
        // "off" is Option::None at the call site, never a PonytailMode, so
        // parsing it here must fail rather than invent a fourth level.
        assert_eq!(PonytailMode::from_id("off"), None);
        assert_eq!(PonytailMode::from_id("review"), None);
        assert_eq!(PonytailMode::from_id(""), None);
    }

    #[test]
    fn every_provider_has_a_declared_integration() {
        // The Settings page renders this for all providers, so a new one must
        // be classified rather than silently defaulting.
        let unsupported = ProviderKind::ALL
            .into_iter()
            .filter(|provider| {
                PonytailIntegration::for_provider(*provider) == PonytailIntegration::Unsupported
            })
            .collect::<Vec<_>>();
        assert_eq!(
            unsupported,
            vec![
                ProviderKind::Amp,
                ProviderKind::Cursor,
                ProviderKind::DeepSeek
            ]
        );
    }

    #[test]
    fn failed_status_never_claims_an_integration() {
        let status = PonytailStatus::failed(ProviderKind::Claude, PonytailMode::Full, "no node");
        assert!(!status.healthy);
        assert_eq!(status.integration, PonytailIntegration::Unsupported);
        // The requested mode survives so the UI can report what was asked for.
        assert_eq!(status.mode, PonytailMode::Full);
    }
}
