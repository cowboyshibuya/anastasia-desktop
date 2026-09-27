//! Anastasia's tooltip surface.
//!
//! GPUI already owns tooltip *behaviour* — hover timing, placement, dismissal —
//! through `InteractiveElement::tooltip`, which asks only for a view to render.
//! This is that view, and nothing more.

use gpui::{
    AnyView, App, AppContext, FontWeight, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, prelude::*, px,
};

use crate::theme::Theme;

/// A single-line hint, optionally paired with a keyboard shortcut badge.
pub struct Tooltip {
    label: SharedString,
    shortcut: Option<SharedString>,
}

impl Tooltip {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
        }
    }

    /// Attach a keyboard shortcut hint badge to the tooltip.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        let sc = shortcut.into();
        if !sc.is_empty() {
            self.shortcut = Some(sc);
        }
        self
    }

    /// Build the view GPUI's `.tooltip(..)` expects.
    pub fn build(self, _window: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| self).into()
    }

    /// Shorthand for the overwhelmingly common case:
    /// `.tooltip(Tooltip::text("Copy message"))`.
    pub fn text(
        label: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let label = label.into();
        move |window, cx| Tooltip::new(label.clone()).build(window, cx)
    }

    /// Shorthand for a tooltip with a keyboard shortcut hint:
    /// `.tooltip(Tooltip::with_shortcut("Toggle Sidebar", "⌘B"))`.
    pub fn with_shortcut(
        label: impl Into<SharedString>,
        shortcut: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let label = label.into();
        let shortcut = shortcut.into();
        move |window, cx| {
            Tooltip::new(label.clone())
                .shortcut(shortcut.clone())
                .build(window, cx)
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        // The outer wrapper is transparent and only offsets the card from the
        // cursor; the shadow needs a parent that does not clip it.
        div().pt(px(4.0)).pl(px(2.0)).child(
            div()
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme.border_strong)
                .bg(theme.raised)
                .shadow_md()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(11.5))
                        .line_height(px(15.0))
                        .text_color(theme.text)
                        .child(self.label.clone()),
                )
                .when_some(self.shortcut.clone(), |card, shortcut| {
                    card.child(
                        div()
                            .px(px(5.0))
                            .py(px(1.0))
                            .min_w(px(18.0))
                            .rounded(px(4.0))
                            .bg(theme.overlay_strong)
                            .border_1()
                            .border_color(theme.border)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(10.5))
                            .line_height(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .font_family(crate::md::render::MONO_FAMILY)
                            .text_color(theme.text_secondary)
                            .child(shortcut),
                    )
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_creation_and_shortcut() {
        let tt = Tooltip::new("Toggle Sidebar");
        assert_eq!(tt.label, "Toggle Sidebar");
        assert_eq!(tt.shortcut, None);

        let tt_with_sc = Tooltip::new("Toggle Sidebar").shortcut("⌘B");
        assert_eq!(tt_with_sc.label, "Toggle Sidebar");
        assert_eq!(tt_with_sc.shortcut.as_deref(), Some("⌘B"));

        let tt_empty_sc = Tooltip::new("Toggle Sidebar").shortcut("");
        assert_eq!(tt_empty_sc.shortcut, None);
    }
}
