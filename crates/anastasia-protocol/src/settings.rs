use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::alabasta::AlabastaConnection;
use crate::computer_use::ComputerAppGrant;
use crate::model::ProviderKind;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(default)]
pub struct DaemonSettings {
    pub computer_use_enabled: bool,
    pub computer_use_allowed_apps: Vec<ComputerAppGrant>,
    pub disabled_providers: Vec<ProviderKind>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub provider_binary_overrides: HashMap<ProviderKind, String>,
    /// The connected Alabasta workspace, or `None` when signed out. Carries no
    /// secret: the refresh token lives in the login keychain, keyed by the
    /// connection's account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alabasta: Option<AlabastaConnection>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            computer_use_enabled: false,
            computer_use_allowed_apps: Vec::new(),
            disabled_providers: Vec::new(),
            provider_binary_overrides: HashMap::new(),
            alabasta: None,
            extra: BTreeMap::new(),
        }
    }
}

impl DaemonSettings {
    pub fn default_path() -> PathBuf {
        crate::identity::desktop_data_dir().join("settings.json")
    }

    pub fn discard_legacy_app_keys(&mut self) {
        for key in ["analytics_enabled", "favorite_models", "theme", "language"] {
            self.extra.remove(key);
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct NewThreadComposerBackground {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NewThreadBackgroundEffect {
    #[default]
    None,
    Scanlines,
    Ascii,
    Halftone,
    Dither,
}

impl NewThreadBackgroundEffect {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Scanlines,
        Self::Ascii,
        Self::Halftone,
        Self::Dither,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Scanlines => "Scanlines",
            Self::Ascii => "ASCII",
            Self::Halftone => "Halftone",
            Self::Dither => "Dither",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::None => "Shows original artwork.",
            Self::Scanlines => "Adds horizontal display scanlines.",
            Self::Ascii => "Renders artwork with colored ASCII characters.",
            Self::Halftone => "Recreates artwork with color print halftone dots.",
            Self::Dither => "Rebuilds artwork with a dithered palette.",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SidebarSort {
    #[default]
    LastUpdated,
    Created,
}

impl SidebarSort {
    pub const ALL: [Self; 2] = [Self::LastUpdated, Self::Created];

    pub const fn label(self) -> &'static str {
        match self {
            Self::LastUpdated => "Last updated",
            Self::Created => "Date created",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SidebarGrouping {
    #[default]
    ByDate,
    ByProject,
    Flat,
}

impl SidebarGrouping {
    pub const ALL: [Self; 3] = [Self::ByDate, Self::ByProject, Self::Flat];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ByDate => "By date",
            Self::ByProject => "By project",
            Self::Flat => "In one list",
        }
    }
}
