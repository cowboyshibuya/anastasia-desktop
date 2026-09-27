//! Shared locale selection and translation access.
//!
//! Anastasia ships English only for now. The preference is kept as an enum
//! rather than collapsed away so an old settings file naming a language that is
//! no longer bundled still loads — it simply resolves to English — and so
//! adding a locale back is a matter of shipping its catalog.

use serde::{Deserialize, Serialize};

/// The language preference Anastasia persists. `System` resolves to one of the
/// locales Anastasia deliberately ships today.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppLanguage {
    #[default]
    System,
    English,
    /// No longer bundled; retained so an existing settings file still parses.
    #[serde(alias = "simplified-chinese")]
    SimplifiedChinese,
    /// No longer bundled; retained so an existing settings file still parses.
    #[serde(alias = "japanese")]
    Japanese,
}

impl AppLanguage {
    /// What the selector offers. Only the locales actually bundled appear;
    /// with a single language there is nothing to choose between, so the
    /// Appearance page hides the control entirely.
    pub const ALL: [Self; 1] = [Self::English];

    pub fn locale(self) -> &'static str {
        match self.resolved() {
            Self::System => unreachable!("system language always resolves to a shipped locale"),
            // Every retained variant resolves to the only catalog shipped.
            _ => "en",
        }
    }

    /// Explicit language names are autonyms so the selector remains
    /// understandable even when the current locale is unfamiliar.
    pub fn label(self) -> String {
        match self {
            Self::System => translate("language.system"),
            _ => "English".to_owned(),
        }
    }

    /// The language actually used. A preference naming an unbundled locale
    /// resolves to English rather than failing.
    pub fn resolved(self) -> Self {
        Self::English
    }
}

pub fn set_language(language: AppLanguage) {
    rust_i18n::set_locale(language.locale());
}

pub fn translate(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preference_resolves_to_the_one_bundled_catalog() {
        for language in [
            AppLanguage::System,
            AppLanguage::English,
            AppLanguage::SimplifiedChinese,
            AppLanguage::Japanese,
        ] {
            assert_eq!(language.locale(), "en");
        }
    }

    #[test]
    fn a_settings_file_naming_a_dropped_language_still_loads() {
        // Written by a build that shipped more locales; it must not fail the
        // parse, and must come up in English.
        let language: AppLanguage =
            serde_json::from_str("\"simplified-chinese\"").expect("known variant still parses");
        assert_eq!(language.locale(), "en");
    }
}
