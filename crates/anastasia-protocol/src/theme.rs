//! Process-neutral theme preference persisted in the desktop settings file.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    /// The default for a new installation. Anastasia is designed dark first —
    /// the near-black plane is the identity, not a variant of it — so a fresh
    /// install opens in it rather than inheriting whatever the OS is set to.
    /// System and Light remain fully supported choices.
    #[default]
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> String {
        match self {
            Self::System => crate::i18n::translate("settings.theme_system"),
            Self::Light => crate::i18n::translate("settings.theme_light"),
            Self::Dark => crate::i18n::translate("settings.theme_dark"),
        }
    }
}
