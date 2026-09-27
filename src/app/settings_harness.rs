//! Settings → Harness: the policies Anastasia compiles into every agent it runs.
//!
//! Ponytail is the first, and for now the only, policy here. The page is
//! deliberately shaped as a list of policy packs rather than a Ponytail screen,
//! because that is what the section grows into.

use super::*;

use anastasia_client::ponytail::{PonytailIntegration, PonytailMode};

/// Where the vendored copy came from, shown beside the version.
const PONYTAIL_HOMEPAGE: &str = "https://github.com/DietrichGebert/ponytail";

impl Waku {
    fn set_ponytail_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.state.ponytail_enabled == enabled {
            return;
        }
        self.state.ponytail_enabled = enabled;
        self.save();
        cx.notify();
    }

    fn set_ponytail_mode(&mut self, mode: PonytailMode, cx: &mut Context<Self>) {
        if self.state.ponytail == mode {
            return;
        }
        self.state.ponytail = mode;
        self.save();
        cx.notify();
    }

    pub(super) fn render_harness_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let enabled = self.state.ponytail_enabled;
        let mode = self.state.ponytail;

        let switch = crate::ui::settings_switch(
            "ponytail-enabled",
            enabled,
            theme,
            cx,
            move |this, _, cx| this.set_ponytail_enabled(!enabled, cx),
        );

        let intensity_handle = self.menu_handle("ponytail-intensity", cx);
        let weak = cx.entity().downgrade();
        let intensity = dropdown_menu(
            MenuChip::new("ponytail-intensity")
                .label(mode.label())
                .outlined()
                .selected(intensity_handle.is_open())
                .w(px(116.0))
                .justify_between(),
            "ponytail-intensity-menu",
            &intensity_handle,
            MenuAlign::BelowRight,
            move |_| {
                PonytailMode::ALL
                    .into_iter()
                    .map(|option| {
                        let weak = weak.clone();
                        MenuItem::new(option.label(), move |_, cx| {
                            let _ = weak.update(cx, |this, cx| this.set_ponytail_mode(option, cx));
                        })
                        .selected(option == mode)
                    })
                    .collect()
            },
        );

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(15.0))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .rounded(px(13.0))
                    .overflow_hidden()
                    .bg(theme.raised)
                    .child(setting_row(
                        theme,
                        tr!("settings.ponytail"),
                        tr!("settings.ponytail_description"),
                        switch.into_any_element(),
                    ))
                    .child(hairline(theme))
                    .child(setting_row(
                        theme,
                        tr!("settings.ponytail_intensity"),
                        tr!("settings.ponytail_intensity_description"),
                        intensity.into_any_element(),
                    )),
            )
            .child(self.render_ponytail_runtimes(enabled, theme))
            .child(render_ponytail_attribution(theme))
            .into_any_element()
    }

    /// Which mechanism each installed runtime gets, so a user can see where the
    /// policy is enforced by the agent's own lifecycle and where it is only
    /// instructions. Disabled providers are left out — they cannot run a session.
    fn render_ponytail_runtimes(&self, enabled: bool, theme: Theme) -> AnyElement {
        let runtimes = ProviderKind::ALL.into_iter().filter(|kind| {
            !self.state.disabled_providers.contains(kind)
                && self
                    .provider_probe(*kind)
                    .is_some_and(|probe| probe.installed)
        });

        let mut rows = div().w_full().flex().flex_col();
        let mut any = false;
        for kind in runtimes {
            let integration = PonytailIntegration::for_provider(kind);
            let applies = enabled && integration != PonytailIntegration::Unsupported;
            if any {
                rows = rows.child(hairline(theme));
            }
            any = true;
            rows = rows.child(
                div()
                    .w_full()
                    .min_h(px(44.0))
                    .px(px(20.0))
                    .py(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(24.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.0))
                            .text_color(theme.text)
                            .child(kind.display_name()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            // Never color alone: the mechanism is spelled out.
                            .text_color(if applies {
                                theme.text_secondary
                            } else {
                                theme.text_tertiary
                            })
                            .child(if enabled {
                                integration.label()
                            } else {
                                tr!("ponytail.integration_off")
                            }),
                    ),
            );
        }

        if !any {
            return div().into_any_element();
        }

        div()
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(4.0))
                    .pb(px(8.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_secondary)
                    .child(tr!("settings.ponytail_runtimes")),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .rounded(px(13.0))
                    .overflow_hidden()
                    .bg(theme.raised)
                    .child(rows),
            )
            .into_any_element()
    }
}

/// A settings row: title and description on the left, one control on the right.
fn setting_row(theme: Theme, title: String, description: String, control: AnyElement) -> Div {
    div()
        .w_full()
        .min_h(px(60.0))
        .px(px(20.0))
        .py(px(12.0))
        .flex()
        .items_center()
        .gap(px(24.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title),
                )
                .child(
                    div()
                        .mt(px(5.0))
                        .text_size(px(12.5))
                        .line_height(px(18.0))
                        .text_color(theme.text_secondary)
                        .child(description),
                ),
        )
        .child(control)
}

fn hairline(theme: Theme) -> Div {
    div().mx(px(20.0)).h(px(1.0)).bg(theme.border)
}

/// Upstream credit and the MIT notice the vendored copy carries.
fn render_ponytail_attribution(theme: Theme) -> AnyElement {
    let version = anastasia_client::ponytail::vendored_version();
    div()
        .px(px(4.0))
        .flex()
        .flex_col()
        .gap(px(3.0))
        .text_size(px(11.5))
        .line_height(px(17.0))
        .text_color(theme.text_tertiary)
        .child(tr!("settings.ponytail_attribution", version = version,))
        .child(PONYTAIL_HOMEPAGE)
        .into_any_element()
}
