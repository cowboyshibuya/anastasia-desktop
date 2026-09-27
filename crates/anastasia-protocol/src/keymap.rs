//! The user-editable keyboard map persisted in the desktop settings file.
//!
//! Combos are stored in GPUI's own platform-neutral keystroke syntax, where
//! `secondary` is Command on macOS and Control elsewhere — so a settings file
//! is portable between platforms and no translation layer is needed at bind
//! time.
//!
//! Only shortcuts a user might reasonably want to move live here. Bindings that
//! back a native menu item (Quit, Close, Minimize, Hide), that are scoped to a
//! surface (find and replace in the file editor, the browser's navigation
//! keys), or that a platform reserves stay fixed in `keymap::bind`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutId {
    OpenSettings,
    ToggleSidebar,
    ToggleRightPanel,
    ToggleTerminal,
    NewSession,
    NewProject,
    ToggleCommandPalette,
    FocusComposer,
    ToggleModelPicker,
    ToggleUsagePanel,
    ToggleInteractionMode,
    NavigateBack,
    NavigateForward,
}

impl ShortcutId {
    pub const ALL: [Self; 13] = [
        Self::OpenSettings,
        Self::ToggleSidebar,
        Self::ToggleRightPanel,
        Self::ToggleTerminal,
        Self::NewSession,
        Self::NewProject,
        Self::ToggleCommandPalette,
        Self::FocusComposer,
        Self::ToggleModelPicker,
        Self::ToggleUsagePanel,
        Self::ToggleInteractionMode,
        Self::NavigateBack,
        Self::NavigateForward,
    ];

    /// Translation key for the row label in Settings → Shortcuts.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::OpenSettings => "shortcut.open_settings",
            Self::ToggleSidebar => "shortcut.toggle_sidebar",
            Self::ToggleRightPanel => "shortcut.toggle_right_panel",
            Self::ToggleTerminal => "shortcut.toggle_terminal",
            Self::NewSession => "shortcut.new_session",
            Self::NewProject => "shortcut.new_project",
            Self::ToggleCommandPalette => "shortcut.toggle_command_palette",
            Self::FocusComposer => "shortcut.focus_composer",
            Self::ToggleModelPicker => "shortcut.toggle_model_picker",
            Self::ToggleUsagePanel => "shortcut.toggle_usage_panel",
            Self::ToggleInteractionMode => "shortcut.toggle_interaction_mode",
            Self::NavigateBack => "shortcut.navigate_back",
            Self::NavigateForward => "shortcut.navigate_forward",
        }
    }

    pub fn label(self) -> String {
        crate::i18n::translate(self.label_key())
    }

    /// Stable key this shortcut is stored under. Deliberately hand-written
    /// rather than derived, so renaming the variant cannot silently orphan
    /// everyone's saved override.
    pub fn key(self) -> &'static str {
        match self {
            Self::OpenSettings => "openSettings",
            Self::ToggleSidebar => "toggleSidebar",
            Self::ToggleRightPanel => "toggleRightPanel",
            Self::ToggleTerminal => "toggleTerminal",
            Self::NewSession => "newSession",
            Self::NewProject => "newProject",
            Self::ToggleCommandPalette => "toggleCommandPalette",
            Self::FocusComposer => "focusComposer",
            Self::ToggleModelPicker => "toggleModelPicker",
            Self::ToggleUsagePanel => "toggleUsagePanel",
            Self::ToggleInteractionMode => "toggleInteractionMode",
            Self::NavigateBack => "navigateBack",
            Self::NavigateForward => "navigateForward",
        }
    }

    pub fn default_combo(self) -> &'static str {
        match self {
            Self::OpenSettings => "secondary-,",
            Self::ToggleSidebar => "secondary-b",
            Self::ToggleRightPanel => "secondary-i",
            Self::ToggleTerminal => "secondary-j",
            Self::NewSession => "secondary-n",
            Self::NewProject => "secondary-o",
            Self::ToggleCommandPalette => "secondary-k",
            Self::FocusComposer => "secondary-l",
            Self::ToggleModelPicker => "secondary-/",
            Self::ToggleUsagePanel => "secondary-u",
            // Shift-Tab is the convention every coding agent CLI uses for
            // flipping between planning and building.
            Self::ToggleInteractionMode => "shift-tab",
            Self::NavigateBack => "secondary-[",
            Self::NavigateForward => "secondary-]",
        }
    }
}

/// Persisted shortcut overrides.
///
/// Only entries that differ from their default are stored, so the file stays
/// small and a later change to a default still reaches users who never touched
/// that row.
///
/// Keyed by [`ShortcutId::key`] rather than by the enum, so an entry this build
/// does not recognize — a settings file written by a newer one — neither fails
/// the parse nor gets dropped on the next save.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct KeymapConfig {
    overrides: BTreeMap<String, String>,
}

impl KeymapConfig {
    /// The combo bound to `id`: the user's override, or the built-in default.
    /// An empty override means the user cleared the row, leaving it unbound.
    pub fn get(&self, id: ShortcutId) -> &str {
        self.overrides
            .get(id.key())
            .map(String::as_str)
            .unwrap_or_else(|| id.default_combo())
    }

    pub fn set(&mut self, id: ShortcutId, combo: String) {
        if combo == id.default_combo() {
            self.overrides.remove(id.key());
        } else {
            self.overrides.insert(id.key().to_owned(), combo);
        }
    }

    pub fn reset(&mut self, id: ShortcutId) {
        self.overrides.remove(id.key());
    }

    /// Restore every *known* shortcut to its default. An unrecognized entry is
    /// left alone: this build cannot show it, so it must not destroy it either.
    pub fn reset_all(&mut self) {
        for id in ShortcutId::ALL {
            self.overrides.remove(id.key());
        }
    }

    pub fn is_customized(&self, id: ShortcutId) -> bool {
        self.overrides.contains_key(id.key())
    }

    pub fn has_customizations(&self) -> bool {
        ShortcutId::ALL
            .into_iter()
            .any(|id| self.overrides.contains_key(id.key()))
    }
}

/// Shortcuts sharing a combo with another shortcut.
///
/// Cleared rows (empty combo) never conflict — any number of shortcuts may be
/// unbound at once.
pub fn conflicts(keymap: &KeymapConfig) -> Vec<ShortcutId> {
    ShortcutId::ALL
        .into_iter()
        .filter(|&id| {
            let combo = keymap.get(id);
            !combo.is_empty()
                && ShortcutId::ALL
                    .into_iter()
                    .any(|other| other != id && keymap.get(other) == combo)
        })
        .collect()
}

/// Build a stored combo from a recorded keystroke.
///
/// Returns `None` for a keystroke that cannot stand alone as a shortcut: a bare
/// modifier (the user is still mid-chord), or an unmodified key that would
/// swallow ordinary typing. Shift-Tab and the function keys are the exceptions —
/// they are not text input, so they bind without a primary modifier.
pub fn combo_from_keystroke(
    control: bool,
    alt: bool,
    shift: bool,
    platform: bool,
    key: &str,
) -> Option<String> {
    let key = key.trim().to_lowercase();
    if key.is_empty()
        || matches!(
            key.as_str(),
            "control" | "ctrl" | "alt" | "option" | "shift" | "cmd" | "command" | "platform" | "fn"
        )
    {
        return None;
    }

    let primary = control || platform;
    let standalone = key == "tab"
        || key == "escape"
        || (key.starts_with('f') && key[1..].parse::<u8>().is_ok_and(|n| (1..=20).contains(&n)));
    if !primary && !alt && !standalone {
        return None;
    }

    let mut parts: Vec<&str> = Vec::new();
    if primary {
        parts.push("secondary");
    }
    if alt {
        parts.push("alt");
    }
    if shift {
        parts.push("shift");
    }
    parts.push(&key);
    Some(parts.join("-"))
}

/// Render a stored combo the way the platform writes it, for the settings chip.
pub fn display_combo(combo: &str) -> String {
    if combo.is_empty() {
        return String::new();
    }
    let macos = cfg!(target_os = "macos");
    let mut out = String::new();
    let mut key = "";
    for part in combo.split('-') {
        match part {
            // A trailing empty segment is the "-" key itself ("secondary--").
            "" => key = "-",
            "secondary" => out.push_str(if macos { "⌘" } else { "Ctrl+" }),
            "alt" => out.push_str(if macos { "⌥" } else { "Alt+" }),
            "shift" => out.push_str(if macos { "⇧" } else { "Shift+" }),
            "ctrl" | "control" => out.push_str(if macos { "⌃" } else { "Ctrl+" }),
            other => key = other,
        }
    }
    out.push_str(&match key {
        "tab" => "⇥".to_string(),
        "escape" => "esc".to_string(),
        "enter" | "return" => "⏎".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        other if other.chars().count() == 1 => other.to_uppercase(),
        other => other.to_string(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_documented_ones_and_cost_nothing_to_store() {
        let keymap = KeymapConfig::default();
        assert_eq!(keymap.get(ShortcutId::OpenSettings), "secondary-,");
        assert_eq!(keymap.get(ShortcutId::ToggleSidebar), "secondary-b");
        assert_eq!(keymap.get(ShortcutId::ToggleRightPanel), "secondary-i");
        assert_eq!(keymap.get(ShortcutId::ToggleTerminal), "secondary-j");
        assert_eq!(keymap.get(ShortcutId::ToggleInteractionMode), "shift-tab");
        assert!(!keymap.has_customizations());
        // An untouched keymap serializes to an empty object, so a settings file
        // carries only what the user actually changed.
        assert_eq!(serde_json::to_string(&keymap).unwrap(), "{}");
    }

    #[test]
    fn no_two_defaults_collide() {
        assert!(conflicts(&KeymapConfig::default()).is_empty());
    }

    #[test]
    fn setting_back_to_the_default_stops_being_an_override() {
        let mut keymap = KeymapConfig::default();
        keymap.set(ShortcutId::ToggleSidebar, "secondary-shift-x".into());
        assert_eq!(keymap.get(ShortcutId::ToggleSidebar), "secondary-shift-x");
        assert!(keymap.is_customized(ShortcutId::ToggleSidebar));

        keymap.set(ShortcutId::ToggleSidebar, "secondary-b".into());
        assert!(!keymap.is_customized(ShortcutId::ToggleSidebar));

        keymap.set(ShortcutId::ToggleSidebar, "secondary-shift-x".into());
        keymap.reset(ShortcutId::ToggleSidebar);
        assert_eq!(keymap.get(ShortcutId::ToggleSidebar), "secondary-b");
    }

    #[test]
    fn conflicts_name_both_sides_and_ignore_cleared_rows() {
        let mut keymap = KeymapConfig::default();
        keymap.set(ShortcutId::ToggleRightPanel, "secondary-b".into());
        let conflicted = conflicts(&keymap);
        assert!(conflicted.contains(&ShortcutId::ToggleSidebar));
        assert!(conflicted.contains(&ShortcutId::ToggleRightPanel));

        // Two unbound rows are not a conflict.
        let mut cleared = KeymapConfig::default();
        cleared.set(ShortcutId::ToggleSidebar, String::new());
        cleared.set(ShortcutId::ToggleRightPanel, String::new());
        assert!(conflicts(&cleared).is_empty());
    }

    #[test]
    fn recording_rejects_keystrokes_that_cannot_stand_alone() {
        // Bare modifiers: the chord is not finished.
        assert_eq!(
            combo_from_keystroke(false, false, true, false, "shift"),
            None
        );
        assert_eq!(combo_from_keystroke(false, false, false, true, "cmd"), None);
        // An unmodified letter would swallow ordinary typing.
        assert_eq!(combo_from_keystroke(false, false, false, false, "b"), None);
        assert_eq!(combo_from_keystroke(false, false, true, false, "b"), None);
        // Tab and the function keys are not text input, so they stand alone.
        assert_eq!(
            combo_from_keystroke(false, false, true, false, "tab").as_deref(),
            Some("shift-tab")
        );
        assert_eq!(
            combo_from_keystroke(false, false, false, false, "f5").as_deref(),
            Some("f5")
        );
    }

    #[test]
    fn recording_folds_both_primary_modifiers_into_secondary() {
        // Command on macOS and Control elsewhere record to the same stored
        // combo, so a settings file moves between platforms intact.
        assert_eq!(
            combo_from_keystroke(false, false, false, true, "B").as_deref(),
            Some("secondary-b")
        );
        assert_eq!(
            combo_from_keystroke(true, false, false, false, "b").as_deref(),
            Some("secondary-b")
        );
        assert_eq!(
            combo_from_keystroke(false, true, true, true, "k").as_deref(),
            Some("secondary-alt-shift-k")
        );
    }

    #[test]
    fn unknown_shortcut_ids_do_not_fail_the_parse() {
        // A settings file from a newer build names a shortcut this one has
        // never heard of. It must load, dropping only that entry.
        let json = r#"{"toggleSidebar":"secondary-x","warpDrive":"secondary-9"}"#;
        let keymap: KeymapConfig = serde_json::from_str(json).expect("unknown ids load");
        assert_eq!(keymap.get(ShortcutId::ToggleSidebar), "secondary-x");

        // And it survives a round trip: downgrading must not silently discard
        // a shortcut a newer build owns.
        keymap_round_trip_keeps_unknown(&keymap);
    }

    fn keymap_round_trip_keeps_unknown(keymap: &KeymapConfig) {
        let written = serde_json::to_string(keymap).unwrap();
        assert!(written.contains("warpDrive"), "{written}");

        let mut restored: KeymapConfig = serde_json::from_str(&written).unwrap();
        restored.reset_all();
        let after_reset = serde_json::to_string(&restored).unwrap();
        assert!(after_reset.contains("warpDrive"), "{after_reset}");
        assert!(!after_reset.contains("toggleSidebar"), "{after_reset}");
    }

    #[test]
    fn display_spells_out_the_platform_glyphs() {
        if cfg!(target_os = "macos") {
            assert_eq!(display_combo("secondary-b"), "⌘B");
            assert_eq!(display_combo("shift-tab"), "⇧⇥");
            assert_eq!(display_combo("secondary-alt-shift-k"), "⌘⌥⇧K");
            assert_eq!(display_combo("secondary-,"), "⌘,");
        }
        assert_eq!(display_combo(""), "");
    }
}
