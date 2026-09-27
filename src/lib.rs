#![recursion_limit = "256"]

rust_i18n::i18n!("locales", fallback = "en");

macro_rules! tr {
    ($key:expr) => { rust_i18n::t!($key).into_owned() };
}

mod app;
mod assets;
mod harness;
mod input;
mod md;
mod theme;
mod ui;

use gpui::{App, Bounds, WindowBounds, WindowOptions, px, size};

pub fn run() {
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            cx.set_app_identity("com.anastasia.desktop", "Anastasia");
            assets::register_fonts(cx).expect("bundled fonts");
            theme::init(cx);
            input::init(cx);
            ui::menu::init(cx);
            let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(760.0), px(540.0))),
                    app_id: Some("com.anastasia.desktop".into()),
                    ..Default::default()
                },
                |window, cx| app::Desktop::new(window, cx),
            )
            .expect("open Anastasia window");
        });
}
