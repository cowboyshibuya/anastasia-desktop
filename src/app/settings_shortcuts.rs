//! Settings → Shortcuts: view and rebind the app's keyboard shortcuts.
//!
//! Recording works by focusing the row's chip and capturing the next
//! keystroke. A combo that would swallow ordinary typing, or that another
//! shortcut already owns, is rejected with a reason rather than silently
//! accepted and left broken.

use gpui::KeyDownEvent;
use anastasia_protocol::keymap::{self, ShortcutId};

use super::*;

/// How long a rejection message stays on the row before it fades out.
const REJECTION_LINGER: Duration = Duration::from_secs(4);

/// Why a recorded keystroke was refused, in the row's own words.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum ShortcutRejection {
    /// The keystroke cannot stand alone — no modifier, and not one of the keys
    /// that are never text input.
    NeedsModifier,
    /// Another shortcut already owns this combo.
    Taken(ShortcutId),
}

impl ShortcutRejection {
    fn message(&self) -> String {
        match self {
            Self::NeedsModifier => tr!("shortcuts.needs_modifier"),
            Self::Taken(other) => tr!("shortcuts.already_taken", name = other.label()),
        }
    }
}

impl Waku {
    /// Put a row into recording mode, or take it back out.
    pub(super) fn toggle_shortcut_recording(&mut self, id: ShortcutId, cx: &mut Context<Self>) {
        self.recording_shortcut = if self.recording_shortcut == Some(id) {
            None
        } else {
            Some(id)
        };
        self.shortcut_rejection = None;
        cx.notify();
    }

    /// Consume a keystroke aimed at the recording row.
    ///
    /// Returns whether the event was claimed — the caller stops propagation on
    /// true, so a shortcut being recorded cannot also fire its own action.
    pub(super) fn record_shortcut_keystroke(
        &mut self,
        id: ShortcutId,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();

        // Escape leaves recording without changing anything.
        if key == "escape" && !modifiers.modified() {
            self.recording_shortcut = None;
            self.shortcut_rejection = None;
            cx.notify();
            return true;
        }

        // Backspace and delete clear the binding, leaving the action reachable
        // only from the menus.
        if matches!(key, "backspace" | "delete") && !modifiers.modified() {
            self.apply_shortcut(id, String::new(), cx);
            return true;
        }

        let Some(combo) = keymap::combo_from_keystroke(
            modifiers.control,
            modifiers.alt,
            modifiers.shift,
            modifiers.platform,
            key,
        ) else {
            // A bare modifier means the chord is still being formed; say
            // nothing and keep waiting. Anything else is a real rejection.
            if !matches!(
                key,
                "control" | "alt" | "shift" | "cmd" | "platform" | "function" | "fn"
            ) {
                self.reject_shortcut(ShortcutRejection::NeedsModifier, cx);
            }
            return true;
        };

        if let Some(other) = ShortcutId::ALL
            .into_iter()
            .find(|&other| other != id && self.state.keymap.get(other) == combo)
        {
            self.reject_shortcut(ShortcutRejection::Taken(other), cx);
            return true;
        }

        self.apply_shortcut(id, combo, cx);
        true
    }

    fn reject_shortcut(&mut self, rejection: ShortcutRejection, cx: &mut Context<Self>) {
        self.shortcut_rejection = Some(rejection);
        // Clear the message on its own, so a rejection the user has moved on
        // from does not sit there looking like a live error.
        self.shortcut_rejection_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REJECTION_LINGER).await;
            let _ = this.update(cx, |this, cx| {
                this.shortcut_rejection = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn apply_shortcut(&mut self, id: ShortcutId, combo: String, cx: &mut Context<Self>) {
        self.state.keymap.set(id, combo);
        self.recording_shortcut = None;
        self.shortcut_rejection = None;
        self.rebind_keymap(cx);
        self.save();
        cx.notify();
    }

    pub(super) fn reset_shortcut(&mut self, id: ShortcutId, cx: &mut Context<Self>) {
        self.state.keymap.reset(id);
        self.recording_shortcut = None;
        self.shortcut_rejection = None;
        self.rebind_keymap(cx);
        self.save();
        cx.notify();
    }

    pub(super) fn reset_all_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.state.keymap.reset_all();
        self.recording_shortcut = None;
        self.shortcut_rejection = None;
        self.rebind_keymap(cx);
        self.save();
        cx.notify();
    }

    /// Re-apply the whole keymap so an edit takes effect without a restart.
    fn rebind_keymap(&self, cx: &mut Context<Self>) {
        crate::keymap::bind(cx, &self.state.keymap);
    }

    pub(super) fn render_shortcuts_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let customized = self.state.keymap.has_customizations();

        let rows = ShortcutId::ALL.into_iter().enumerate().map(|(index, id)| {
            let recording = self.recording_shortcut == Some(id);
            let combo = self.state.keymap.get(id).to_owned();
            let rejection = recording.then(|| self.shortcut_rejection.clone()).flatten();

            let chip_label = if recording {
                tr!("shortcuts.press_keys")
            } else if combo.is_empty() {
                tr!("shortcuts.unbound")
            } else {
                keymap::display_combo(&combo)
            };

            let chip = div()
                .id(SharedString::from(format!("shortcut-chip-{}", id.key())))
                .tab_index(0)
                .focus_visible(|style| style.border_color(theme.accent))
                .min_w(px(96.0))
                .h(px(26.0))
                .px(px(10.0))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(7.0))
                .cursor_default()
                .border_1()
                .border_color(if recording {
                    theme.accent
                } else {
                    theme.border_strong
                })
                .bg(if recording {
                    theme.inset
                } else {
                    theme.surface
                })
                .text_size(px(12.0))
                // The chip reads as a key legend, so it wants the mono face
                // even though the label beside it is the interface face.
                .font_family(crate::md::render::MONO_FAMILY)
                .text_color(if recording {
                    theme.accent
                } else if combo.is_empty() {
                    theme.text_tertiary
                } else {
                    theme.text
                })
                .child(chip_label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_shortcut_recording(id, cx);
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if this.recording_shortcut == Some(id) {
                        if this.record_shortcut_keystroke(id, event, cx) {
                            cx.stop_propagation();
                        }
                        return;
                    }
                    // Not recording yet: enter and space start it, the way any
                    // other button on this page activates.
                    if !event.keystroke.modifiers.modified()
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        this.toggle_shortcut_recording(id, cx);
                        cx.stop_propagation();
                    }
                }));

            let reset = self.state.keymap.is_customized(id).then(|| {
                div()
                    .id(SharedString::from(format!("shortcut-reset-{}", id.key())))
                    .tab_index(0)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .h(px(26.0))
                    .px(px(8.0))
                    .flex()
                    .flex_none()
                    .items_center()
                    .rounded(px(7.0))
                    .cursor_default()
                    .text_size(px(12.0))
                    .text_color(theme.text_secondary)
                    .hover(|style| style.bg(theme.overlay).text_color(theme.text))
                    .child(tr!("shortcuts.reset"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.reset_shortcut(id, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            this.reset_shortcut(id, cx);
                            cx.stop_propagation();
                        }
                    }))
            });

            let label = div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(id.label()),
                )
                .children(rejection.map(|rejection| {
                    div()
                        .mt(px(5.0))
                        .text_size(px(12.5))
                        .line_height(px(18.0))
                        // Paired with the message text, never color alone.
                        .text_color(theme.danger)
                        .child(rejection.message())
                }));

            let row = div()
                .w_full()
                .min_h(px(52.0))
                .px(px(20.0))
                .py(px(10.0))
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(label)
                .children(reset)
                .child(chip);

            // A hairline between rows, inset like every other settings card.
            if index == 0 {
                row.into_any_element()
            } else {
                div()
                    .child(div().mx(px(20.0)).h(px(1.0)).bg(theme.border))
                    .child(row)
                    .into_any_element()
            }
        });

        div()
            .child(
                div()
                    .mt(px(15.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.5))
                            .line_height(px(18.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("shortcuts.description")),
                    )
                    .when(customized, |header| {
                        header.child(
                            div()
                                .id("shortcuts-reset-all")
                                .tab_index(0)
                                .focus_visible(|style| style.border_color(theme.accent))
                                .h(px(26.0))
                                .px(px(10.0))
                                .flex()
                                .flex_none()
                                .items_center()
                                .rounded(px(7.0))
                                .cursor_default()
                                .border_1()
                                .border_color(theme.border_strong)
                                .text_size(px(12.0))
                                .text_color(theme.text_secondary)
                                .hover(|style| style.bg(theme.overlay).text_color(theme.text))
                                .child(tr!("shortcuts.reset_all"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.reset_all_shortcuts(cx);
                                }))
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    if !event.keystroke.modifiers.modified()
                                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                                    {
                                        this.reset_all_shortcuts(cx);
                                        cx.stop_propagation();
                                    }
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .mt(px(12.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .rounded(px(13.0))
                    .overflow_hidden()
                    .bg(theme.raised)
                    .children(rows),
            )
            .into_any_element()
    }
}
