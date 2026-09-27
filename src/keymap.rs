//! Applying the persisted [`KeymapConfig`] to GPUI's key dispatcher.
//!
//! The map is re-applied whole every time it changes, so a shortcut edit takes
//! effect immediately without a restart. GPUI has no "remove one binding" API —
//! [`App::clear_key_bindings`] plus a full rebind is the supported shape — which
//! means every binding in the app has to be declared here, including the fixed
//! ones that are not user-editable, or they would be lost on the first edit.

use gpui::{App, KeyBinding, Keystroke};
use anastasia_protocol::keymap::{KeymapConfig, ShortcutId};

use crate::{
    BrowserAddressCancel, BrowserBack, BrowserDevtools, BrowserForward, BrowserHardReload,
    BrowserReload, BrowserStop, CancelTurn, CloseFind, CloseWindow, CopySelection, FindNext,
    FindPrevious, FocusBrowserAddress, FocusComposer, NavigateBack, NavigateForward, NewProject,
    NewSession, OpenFind, OpenFindReplace, OpenSettings, Quit, ReplaceAllMatches, SaveFile,
    ToggleCommandPalette, ToggleFindCaseSensitive, ToggleFindRegex, ToggleFindWholeWord,
    ToggleFpsCounter, ToggleInteractionMode, ToggleModelPicker, ToggleRightPanel, ToggleSidebar,
    ToggleTerminal, ToggleUsagePanel, WebviewCopy, WebviewCut, WebviewPaste, WebviewSelectAll,
};

/// The key context the customizable app shortcuts are bound in. `None` means
/// "anywhere", which is what an app-level shortcut wants.
const APP_CONTEXT: Option<&str> = None;

/// Install the app's entire key map, replacing everything currently bound.
/// Safe to call at any time; a shortcut edit calls it again.
///
/// Every module that owns bindings re-declares them here. `clear_key_bindings`
/// is indiscriminate — it drops the composer's editing keys, the menus, the
/// palette and the settings search along with the app shortcuts — so anything
/// not re-registered on this path silently stops working the moment the map is
/// applied. Each `init` below is a pure `bind_keys` call and is safe to re-run.
pub fn bind(cx: &mut App, keymap: &KeymapConfig) {
    cx.clear_key_bindings();
    crate::input::init(cx);
    crate::ui::menu::init(cx);
    crate::app::init_composer_autocomplete(cx);
    crate::app::init_settings_keys(cx);
    crate::app::init_command_palette(cx);
    crate::app::init_commit_dialog_keys(cx);
    crate::app::init_image_preview_keys(cx);
    crate::app::init_sidebar_keys(cx);
    crate::app::init_skills_keys(cx);
    cx.bind_keys(fixed_bindings());
    cx.bind_keys(customizable_bindings(keymap));
}

/// A binding for `id`, unless the user cleared the row or stored something GPUI
/// cannot parse. A bad combo falls back to the default rather than leaving the
/// action unreachable — the settings UI rejects unparseable input, so this only
/// fires for a hand-edited file.
fn binding_for(keymap: &KeymapConfig, id: ShortcutId) -> Option<String> {
    let combo = keymap.get(id);
    if combo.is_empty() {
        return None;
    }
    if Keystroke::parse(combo).is_ok() {
        return Some(combo.to_owned());
    }
    eprintln!(
        "Anastasia: unparseable shortcut {combo:?} for {}; using the default",
        id.key()
    );
    Some(id.default_combo().to_owned())
}

fn customizable_bindings(keymap: &KeymapConfig) -> Vec<KeyBinding> {
    let mut bindings = Vec::with_capacity(ShortcutId::ALL.len());
    // A macro rather than a closure: `KeyBinding::new` takes the action by
    // value and monomorphizes on its type, so the actions cannot be boxed
    // behind one `dyn Action` parameter.
    macro_rules! bind {
        ($id:expr, $action:expr, $context:expr) => {
            if let Some(combo) = binding_for(keymap, $id) {
                bindings.push(KeyBinding::new(&combo, $action, $context));
            }
        };
    }

    bind!(ShortcutId::OpenSettings, OpenSettings, APP_CONTEXT);
    bind!(ShortcutId::ToggleSidebar, ToggleSidebar, APP_CONTEXT);
    bind!(ShortcutId::ToggleRightPanel, ToggleRightPanel, APP_CONTEXT);
    bind!(ShortcutId::ToggleTerminal, ToggleTerminal, APP_CONTEXT);
    bind!(ShortcutId::NewSession, NewSession, APP_CONTEXT);
    bind!(ShortcutId::NewProject, NewProject, APP_CONTEXT);
    bind!(
        ShortcutId::ToggleCommandPalette,
        ToggleCommandPalette,
        APP_CONTEXT
    );
    bind!(ShortcutId::FocusComposer, FocusComposer, APP_CONTEXT);
    bind!(
        ShortcutId::ToggleModelPicker,
        ToggleModelPicker,
        APP_CONTEXT
    );
    bind!(ShortcutId::ToggleUsagePanel, ToggleUsagePanel, APP_CONTEXT);
    // Scoped to the app context so Shift-Tab stays available as a focus
    // traversal key inside settings, dialogs and the browser.
    bind!(
        ShortcutId::ToggleInteractionMode,
        ToggleInteractionMode,
        Some("Anastasia")
    );
    bind!(ShortcutId::NavigateBack, NavigateBack, Some("Anastasia"));
    bind!(
        ShortcutId::NavigateForward,
        NavigateForward,
        Some("Anastasia")
    );
    bindings
}

/// Bindings the keymap never touches: the ones backing native menu items, the
/// ones scoped to a surface that owns its own conventions, and the diagnostics
/// toggle.
fn fixed_bindings() -> Vec<KeyBinding> {
    vec![
        // `secondary` is Command on macOS and Control elsewhere.
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-w", CloseWindow, None),
        KeyBinding::new("secondary-s", SaveFile, None),
        KeyBinding::new("secondary-alt-shift-f", ToggleFpsCounter, None),
        KeyBinding::new("escape", CancelTurn, Some("Anastasia")),
        KeyBinding::new("secondary-c", CopySelection, Some("Anastasia")),
        // Find and replace in the right panel's file editor, on the
        // conventional VS Code bindings. The primary shortcut + G cycles
        // matches from the editor without moving focus to the bar.
        KeyBinding::new("secondary-f", OpenFind, Some("Anastasia")),
        KeyBinding::new("secondary-alt-f", OpenFindReplace, Some("Anastasia")),
        KeyBinding::new("secondary-g", FindNext, Some("Anastasia")),
        KeyBinding::new("secondary-shift-g", FindPrevious, Some("Anastasia")),
        // Scoped to the editor pane: escape closes the bar there and falls
        // through to CancelTurn anywhere else.
        KeyBinding::new("escape", CloseFind, Some("FileEditorPane")),
        KeyBinding::new(
            "secondary-alt-c",
            ToggleFindCaseSensitive,
            Some("FileEditorPane"),
        ),
        KeyBinding::new(
            "secondary-alt-w",
            ToggleFindWholeWord,
            Some("FileEditorPane"),
        ),
        KeyBinding::new("secondary-alt-r", ToggleFindRegex, Some("FileEditorPane")),
        KeyBinding::new("shift-enter", FindPrevious, Some("FindBar")),
        KeyBinding::new("secondary-alt-enter", ReplaceAllMatches, Some("FindBar")),
        // Browser surface. Deeper than "Anastasia", so while focus is on the
        // page or its address bar the browser reads the platform's
        // conventional navigation shortcuts; the same keys elsewhere keep
        // their app meanings. The clipboard trio is rebound because GPUI's
        // window view claims key equivalents before AppKit can walk the
        // responder chain into the webview.
        KeyBinding::new("secondary-l", FocusBrowserAddress, Some("Browser")),
        KeyBinding::new("secondary-r", BrowserReload, Some("Browser")),
        KeyBinding::new("secondary-shift-r", BrowserHardReload, Some("Browser")),
        KeyBinding::new("secondary-[", BrowserBack, Some("Browser")),
        KeyBinding::new("secondary-]", BrowserForward, Some("Browser")),
        KeyBinding::new("escape", BrowserStop, Some("Browser")),
        KeyBinding::new("secondary-alt-i", BrowserDevtools, Some("Browser")),
        KeyBinding::new("secondary-c", WebviewCopy, Some("Browser")),
        KeyBinding::new("secondary-x", WebviewCut, Some("Browser")),
        KeyBinding::new("secondary-v", WebviewPaste, Some("Browser")),
        KeyBinding::new("secondary-a", WebviewSelectAll, Some("Browser")),
        KeyBinding::new("escape", BrowserAddressCancel, Some("BrowserAddress")),
    ]
}
