//! How the app announces that a session wants attention.
//!
//! Three independent switches rather than one "notifications" toggle: a chime
//! and a banner interrupt differently, and whether an announcement is wanted
//! while the app is already in front is a separate question again.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NotificationSettings {
    /// Play a chime on the two edges worth interrupting for.
    pub sound_enabled: bool,
    /// Post a desktop banner on turn completion.
    pub banners_enabled: bool,
    /// Only announce while Anastasia is not the focused app. On by default:
    /// while you are watching the transcript, the transcript itself is the
    /// notification.
    pub background_only: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            banners_enabled: true,
            background_only: true,
        }
    }
}

impl NotificationSettings {
    /// Whether an announcement should be made at all, given whether the app
    /// currently holds focus.
    pub fn should_announce(&self, app_focused: bool) -> bool {
        !self.background_only || !app_focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_announce_only_in_the_background() {
        let settings = NotificationSettings::default();
        assert!(settings.sound_enabled);
        assert!(settings.banners_enabled);
        assert!(settings.should_announce(false));
        assert!(!settings.should_announce(true));
    }

    #[test]
    fn clearing_background_only_announces_either_way() {
        let settings = NotificationSettings {
            background_only: false,
            ..Default::default()
        };
        assert!(settings.should_announce(true));
        assert!(settings.should_announce(false));
    }

    #[test]
    fn a_settings_file_predating_these_keys_still_loads() {
        let settings: NotificationSettings = serde_json::from_str("{}").expect("empty object");
        assert_eq!(settings, NotificationSettings::default());

        // And a partial one keeps the defaults for whatever it omits.
        let partial: NotificationSettings =
            serde_json::from_str(r#"{"soundEnabled":false}"#).expect("partial object");
        assert!(!partial.sound_enabled);
        assert!(partial.banners_enabled);
        assert!(partial.background_only);
    }
}
