use gpui::prelude::*;
use gpui::*;
use anastasia_client::alabasta::{
    AlabastaConnection, AlabastaIntegration,
};
use anastasia_protocol::model::ProviderKind;

use super::{Theme, Waku};
use crate::ui::icon;

impl Waku {
    pub(super) fn render_integrations_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = Theme::current(cx);
        let connection = self.state.alabasta.clone();

        div()
            .mt(px(15.0))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(self.render_alabasta_connection_card(connection.as_ref(), theme, cx))
            .child(self.render_alabasta_project_bindings_card(connection.is_some(), theme, cx))
            .child(self.render_alabasta_provider_matrix_card(theme))
            .into_any_element()
    }

    fn render_alabasta_connection_card(
        &self,
        connection: Option<&AlabastaConnection>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_connected = connection.is_some_and(|c| c.is_configured());

        let mut card = div()
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(13.0))
            .overflow_hidden()
            .bg(theme.raised);

        // Header row
        let header_row = div()
            .w_full()
            .min_h(px(64.0))
            .px(px(20.0))
            .py(px(14.0))
            .flex()
            .items_center()
            .gap(px(16.0))
            .child(
                div()
                    .w(px(36.0))
                    .h(px(36.0))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.overlay)
                    .child(icon("icons/alabasta.svg", 20.0, theme.accent)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(tr!("settings.alabasta")),
                    )
                    .child(
                        div()
                            .mt(px(3.0))
                            .text_size(px(12.5))
                            .line_height(px(17.0))
                            .text_color(theme.text_secondary)
                            .child(tr!("settings.alabasta_description")),
                    ),
            )
            .child(if is_connected {
                div()
                    .id("alabasta-disconnect-btn")
                    .tab_index(0)
                    .h(px(28.0))
                    .px(px(12.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(theme.border_strong)
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .focus_visible(|style| style.border_color(theme.accent))
                    .hover(|element| element.bg(theme.overlay))
                    .child(tr!("settings.alabasta_disconnect"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.disconnect_alabasta(cx);
                    }))
            } else {
                div()
                    .id("alabasta-connect-btn")
                    .tab_index(0)
                    .h(px(28.0))
                    .px(px(14.0))
                    .rounded(px(6.0))
                    .bg(theme.accent)
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.on_inverse)
                    .focus_visible(|style| style.border_color(theme.text))
                    .hover(|element| element.opacity(0.9))
                    .child(tr!("settings.alabasta_connect"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.begin_alabasta_connect(cx);
                    }))
            });

        card = card.child(header_row);

        if let Some(conn) = connection {
            if is_connected {
                let workspace_display = if !conn.workspace_name.is_empty() {
                    conn.workspace_name.clone()
                } else if !conn.workspace_slug.is_empty() {
                    conn.workspace_slug.clone()
                } else {
                    conn.workspace_id.clone()
                };

                card = card
                    .child(hairline(theme))
                    .child(detail_row(
                        theme,
                        tr!("settings.alabasta_account"),
                        if conn.account_label.is_empty() {
                            "Signed in".into()
                        } else {
                            conn.account_label.clone()
                        },
                    ))
                    .child(hairline(theme))
                    .child(detail_row(
                        theme,
                        tr!("settings.alabasta_workspace"),
                        workspace_display,
                    ))
                    .child(hairline(theme))
                    .child(detail_row(
                        theme,
                        tr!("settings.alabasta_site_url"),
                        conn.site_url.clone(),
                    ));
            }
        }

        card.into_any_element()
    }

    fn render_alabasta_project_bindings_card(
        &self,
        alabasta_connected: bool,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let projects = self.state.projects.clone();

        let mut card = div()
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(13.0))
            .overflow_hidden()
            .bg(theme.raised);

        let header = div()
            .w_full()
            .px(px(20.0))
            .pt(px(14.0))
            .pb(px(10.0))
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tr!("settings.alabasta_product_bindings")),
            )
            .child(
                div()
                    .mt(px(3.0))
                    .text_size(px(12.0))
                    .text_color(theme.text_secondary)
                    .child(tr!("settings.alabasta_product_bindings_description")),
            );

        card = card.child(header);

        if projects.is_empty() {
            card = card.child(hairline(theme)).child(
                div()
                    .w_full()
                    .px(px(20.0))
                    .py(px(14.0))
                    .text_size(px(12.5))
                    .text_color(theme.text_tertiary)
                    .child(tr!("settings.alabasta_no_projects")),
            );
        } else {
            for project in projects {
                let project_id = project.id;
                let binding = self.state.alabasta_bindings.get(&project_id).cloned();
                let bound = binding.is_some();
                let product_name = binding
                    .as_ref()
                    .map(|b| {
                        if !b.product_name.is_empty() {
                            b.product_name.clone()
                        } else {
                            b.product_identifier.clone()
                        }
                    })
                    .unwrap_or_else(|| tr!("settings.alabasta_unbind"));

                let row = div()
                    .w_full()
                    .min_h(px(46.0))
                    .px(px(20.0))
                    .py(px(10.0))
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(project.name),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(theme.text_tertiary)
                                    .child(project.path.display().to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(if bound {
                                        theme.text_secondary
                                    } else {
                                        theme.text_tertiary
                                    })
                                    .child(if bound {
                                        tr!("settings.alabasta_bound_to", product = product_name)
                                    } else {
                                        product_name
                                    }),
                            )
                            .when(bound && alabasta_connected, |parent| {
                                parent.child(
                                    div()
                                        .id(SharedString::from(format!("unbind-{}", project_id)))
                                        .tab_index(0)
                                        .h(px(24.0))
                                        .px(px(8.0))
                                        .rounded(px(5.0))
                                        .border_1()
                                        .border_color(theme.border)
                                        .flex()
                                        .items_center()
                                        .cursor_default()
                                        .text_size(px(11.0))
                                        .text_color(theme.text_secondary)
                                        .focus_visible(|style| style.border_color(theme.accent))
                                        .hover(|el| el.bg(theme.overlay))
                                        .child(tr!("settings.alabasta_unbind"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.state.alabasta_bindings.remove(&project_id);
                                            this.save();
                                            cx.notify();
                                        })),
                                )
                            }),
                    );

                card = card.child(hairline(theme)).child(row);
            }
        }

        card.into_any_element()
    }

    fn render_alabasta_provider_matrix_card(&self, theme: Theme) -> AnyElement {
        let providers = ProviderKind::ALL;

        let mut card = div()
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(13.0))
            .overflow_hidden()
            .bg(theme.raised);

        let header = div()
            .w_full()
            .px(px(20.0))
            .pt(px(14.0))
            .pb(px(10.0))
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(tr!("settings.alabasta_health")),
            );

        card = card.child(header);

        for kind in providers {
            let integration = AlabastaIntegration::for_provider(kind);
            let row = div()
                .w_full()
                .min_h(px(40.0))
                .px(px(20.0))
                .py(px(8.0))
                .flex()
                .items_center()
                .gap(px(16.0))
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
                        .text_color(match integration {
                            AlabastaIntegration::PushAndBridge => theme.text_secondary,
                            AlabastaIntegration::PushOnly => theme.text_secondary,
                            AlabastaIntegration::Unsupported => theme.text_tertiary,
                        })
                        .child(integration.label()),
                );

            card = card.child(hairline(theme)).child(row);
        }

        card.into_any_element()
    }

    fn begin_alabasta_connect(&mut self, cx: &mut Context<Self>) {
        let site_url = std::env::var("ALABASTA_SITE_URL")
            .unwrap_or_else(|_| "https://expert-mule-962.eu-west-1.convex.site".into());
        let app_url = std::env::var("ALABASTA_APP_URL")
            .unwrap_or_else(|_| "http://localhost:3001".into());

        self.show_toast(tr!("settings.alabasta_connecting"));
        cx.notify();

        let bg = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let auth_result = bg
                .spawn(async move {
                    let flow = crate::gui_services::alabasta::auth::begin(&site_url, &app_url)?;
                    let authorize_url = flow.url.clone();
                    Ok::<_, anyhow::Error>((flow, authorize_url, site_url, app_url))
                })
                .await;

            let (flow, authorize_url, site_url, app_url) = match auth_result {
                Ok(tuple) => tuple,
                Err(error) => {
                    let _ = this.update(cx, |view, cx| {
                        view.show_toast(format!("Alabasta connect error: {error:#}"));
                        cx.notify();
                    });
                    return;
                }
            };

            let _ = this.update(cx, |_, cx| {
                cx.open_url(&authorize_url);
            });

            let token_result = bg
                .spawn(async move {
                    let tokens = flow.complete(&site_url)?;
                    let connection = AlabastaConnection {
                        site_url: site_url.clone(),
                        app_url: app_url.clone(),
                        workspace_id: "default".into(),
                        workspace_slug: "default".into(),
                        workspace_name: "Default Workspace".into(),
                        account_label: "Signed in".into(),
                    };
                    crate::gui_services::alabasta::auth::store_refresh_token(
                        &connection.account_key(),
                        &tokens.refresh_token,
                    )?;
                    Ok::<_, anyhow::Error>(connection)
                })
                .await;

            let _ = this.update(cx, |view, cx| {
                match token_result {
                    Ok(connection) => {
                        view.state.alabasta = Some(connection);
                        view.save();
                        view.show_toast(tr!("settings.alabasta_connected"));
                    }
                    Err(error) => {
                        view.show_toast(format!("Alabasta authorization failed: {error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn disconnect_alabasta(&mut self, cx: &mut Context<Self>) {
        if let Some(conn) = self.state.alabasta.take() {
            crate::gui_services::alabasta::auth::delete_refresh_token(&conn.account_key());
            self.save();
            self.show_toast(tr!("settings.alabasta_disconnected"));
            cx.notify();
        }
    }
}

fn detail_row(theme: Theme, label: String, value: String) -> Div {
    div()
        .w_full()
        .min_h(px(42.0))
        .px(px(20.0))
        .py(px(10.0))
        .flex()
        .items_center()
        .gap(px(16.0))
        .child(
            div()
                .text_size(px(12.5))
                .text_color(theme.text_secondary)
                .child(label),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(value),
        )
}

fn hairline(theme: Theme) -> Div {
    div().mx(px(20.0)).h(px(1.0)).bg(theme.border)
}
