use gpui::{App, Global, Hsla, Window, WindowAppearance, hsla, rgb, transparent_black};

#[derive(Clone, Copy)]
pub enum ThemePreference { System, Light, Dark }

fn resolves_to_dark(preference: ThemePreference, system_appearance: WindowAppearance) -> bool {
    match preference {
        ThemePreference::System => matches!(
            system_appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
        ThemePreference::Light => false,
        ThemePreference::Dark => true,
    }
}

fn native_override(preference: ThemePreference) -> Option<bool> {
    match preference {
        ThemePreference::System => None,
        ThemePreference::Light => Some(false),
        ThemePreference::Dark => Some(true),
    }
}

/// Anastasia's visual language, take two: neutral graphite surfaces in the spirit
/// of Cursor — color is reserved for meaning. On macOS the sidebar's semantic
/// tint is installed as a native layer above Sidebar vibrancy; keeping this
/// GPUI surface clear avoids incorrectly accumulating the alpha of nested Metal
/// backgrounds. Selected, hovered, and pressed rows remain a 6% neutral layer.
#[derive(Clone, Copy)]
pub struct Theme {
    pub is_dark: bool,
    pub canvas: Hsla,
    pub sidebar: Hsla,
    pub sidebar_drag_background: Hsla,
    pub sidebar_item_background: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub composer: Hsla,
    pub inset: Hsla,
    /// Terminal screen surface: paper-white in light mode, near-black in dark.
    pub terminal: Hsla,
    pub overlay: Hsla,
    pub overlay_strong: Hsla,

    pub border: Hsla,
    pub border_strong: Hsla,
    pub sidebar_border: Hsla,

    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_ghost: Hsla,

    /// Brand coral. Logo, caret, live-activity pulses — nothing structural.
    pub accent: Hsla,
    pub resize_handle: Hsla,
    /// Meter fills in the usage panel. Quota-meter blue by convention;
    /// warning/danger take over as a lane fills.
    pub gauge: Hsla,

    /// Text-selection wash. Painted *under* the glyphs, so it stays
    /// translucent and deliberately reads as the familiar browser blue rather
    /// than as brand color.
    pub selection: Hsla,
    /// Inline `code` foreground and its rounded wash.
    pub code_text: Hsla,
    pub code_wash: Hsla,

    /// Light fill for primary buttons (send, allow), dark glyph on top.
    pub inverse: Hsla,
    pub on_inverse: Hsla,

    pub warning: Hsla,
    pub success: Hsla,
    pub favorite: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
}

impl Theme {
    pub fn current(cx: &App) -> Self {
        if cx.has_global::<ActiveAnastasiaTheme>() {
            cx.global::<ActiveAnastasiaTheme>().0
        } else {
            Self::dark()
        }
    }

    /// Dark is the default appearance, and the one the identity is drawn for:
    /// a near-black plane the way the halftone marks are set on black, with the
    /// chrome sitting a few points above it rather than a charcoal grey the
    /// content has to fight.
    ///
    /// Elevation reads bottom to top as inset < sidebar < surface < composer <
    /// raised, in luminance order — depth comes from that ladder, not from
    /// borders, which stay hairline.
    pub fn dark() -> Self {
        Self {
            is_dark: true,
            canvas: rgb(0x121213).into(),
            sidebar: if cfg!(target_os = "macos") {
                transparent_black()
            } else {
                rgb(0x09090A).into()
            },
            sidebar_drag_background: rgb(0x09090A).into(),
            sidebar_item_background: hsla(0.0, 0.0, 1.0, 0.05),
            surface: rgb(0x121213).into(),
            raised: rgb(0x212122).into(),
            composer: rgb(0x1A1A1B).into(),
            inset: rgb(0x09090A).into(),
            terminal: rgb(0x09090A).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.90, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.09),

            border: hsla(220.0 / 360.0, 0.08, 0.40, 0.22),
            border_strong: hsla(220.0 / 360.0, 0.08, 0.60, 0.35),
            sidebar_border: hsla(220.0 / 360.0, 0.08, 0.40, 0.22),

            text: rgb(0xF1F2F3).into(),
            text_secondary: rgb(0x9BA1A8).into(),
            text_tertiary: rgb(0x78808A).into(),
            text_ghost: rgb(0x667080).into(),

            // Beautiful Electric & Sky Blue gradient colors (#356FE6 & #81BEFF)
            accent: rgb(0x356FE6).into(),
            resize_handle: rgb(0x356FE6).into(),
            gauge: rgb(0x356FE6).into(),

            selection: hsla(220.0 / 360.0, 0.90, 0.60, 0.25),
            code_text: rgb(0x81BEFF).into(),
            code_wash: hsla(220.0 / 360.0, 0.90, 0.60, 0.08),

            inverse: rgb(0xF1F2F3).into(),
            on_inverse: rgb(0x121213).into(),

            warning: rgb(0xE6A450).into(),
            success: rgb(0x81BEFF).into(),
            favorite: rgb(0xE6A450).into(),
            danger: rgb(0xEC626A).into(),
            danger_soft: hsla(356.0 / 360.0, 0.80, 0.65, 0.12),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            canvas: rgb(0xF9F9FA).into(),
            sidebar: if cfg!(target_os = "macos") {
                transparent_black()
            } else {
                rgb(0xEEEEF0).into()
            },
            sidebar_drag_background: rgb(0xEEEEF0).into(),
            sidebar_item_background: hsla(0.0, 0.0, 0.0, 0.04),
            surface: rgb(0xF9F9FA).into(),
            raised: rgb(0xFFFFFF).into(),
            composer: rgb(0xFFFFFF).into(),
            inset: rgb(0xEEEEF0).into(),
            terminal: rgb(0xFFFFFF).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.12, 0.04),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.08),

            border: hsla(220.0 / 360.0, 0.10, 0.30, 0.16),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.30, 0.32),
            sidebar_border: hsla(220.0 / 360.0, 0.10, 0.30, 0.18),

            text: rgb(0x111318).into(),
            text_secondary: rgb(0x4A505A).into(),
            text_tertiary: rgb(0x606874).into(),
            text_ghost: rgb(0x767E8A).into(),

            accent: rgb(0x356FE6).into(),
            resize_handle: rgb(0x356FE6).into(),
            gauge: rgb(0x356FE6).into(),

            selection: hsla(220.0 / 360.0, 0.90, 0.55, 0.25),
            code_text: rgb(0x356FE6).into(),
            code_wash: hsla(220.0 / 360.0, 0.90, 0.55, 0.08),

            inverse: rgb(0x111318).into(),
            on_inverse: rgb(0xF9F9FA).into(),

            warning: rgb(0xB45309).into(),
            success: rgb(0x059669).into(),
            favorite: rgb(0xB45309).into(),
            danger: rgb(0xB91C1C).into(),
            danger_soft: hsla(0.0, 0.80, 0.40, 0.10),
        }
    }

    pub fn glass_overlay(&self) -> Hsla {
        if self.is_dark {
            hsla(0.0, 0.0, 0.08, 0.72)
        } else {
            hsla(0.0, 0.0, 0.98, 0.82)
        }
    }

    pub fn glass(&self) -> Hsla {
        if self.is_dark {
            hsla(0.0, 0.0, 0.05, 0.65)
        } else {
            hsla(0.0, 0.0, 0.97, 0.70)
        }
    }

    /// Semantic color for file mentions in composer and chat.
    pub fn mention_color(&self) -> Hsla {
        if self.is_dark {
            rgb(0x60A5FA).into() // bright blue
        } else {
            rgb(0x2563EB).into() // royal blue
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveAnastasiaTheme(Theme);

impl Global for ActiveAnastasiaTheme {}

/// Publish the resolved palette. [`Theme::current`] reads it back from the
/// global, which is how every view gets its colors.
fn set_active_theme(theme: Theme, cx: &mut App) {
    cx.set_global(ActiveAnastasiaTheme(theme));
}

/// Resolve and publish the startup palette, before any window exists.
pub fn init(cx: &mut App) {
    let system_appearance = cx.window_appearance();
    let theme = if resolves_to_dark(ThemePreference::System, system_appearance) {
        Theme::dark()
    } else {
        Theme::light()
    };
    set_active_theme(theme, cx);
}

pub fn apply_theme_preference(preference: ThemePreference, window: &mut Window, cx: &mut App) {
    let _ = native_override(preference);
    let is_dark = resolves_to_dark(preference, cx.window_appearance());
    set_active_theme(
        if is_dark {
            Theme::dark()
        } else {
            Theme::light()
        },
        cx,
    );
    window.refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2 relative luminance of an opaque theme color.
    fn luminance(color: Hsla) -> f32 {
        let rgba = gpui::Rgba::from(color);
        let channel = |c: f32| {
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgba.r) + 0.7152 * channel(rgba.g) + 0.0722 * channel(rgba.b)
    }

    fn contrast(foreground: Hsla, background: Hsla) -> f32 {
        let (a, b) = (luminance(foreground), luminance(background));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Text and controls have to stay legible on the surface they sit on.
    /// Body-weight text owes 4.5:1; a disabled or decorative control still owes
    /// 3:1, which is where `text_ghost` used to fail in both appearances.
    #[test]
    fn every_foreground_clears_its_contrast_floor() {
        for theme in [Theme::dark(), Theme::light()] {
            let name = if theme.is_dark { "dark" } else { "light" };
            let body = [
                ("text", theme.text),
                ("text_secondary", theme.text_secondary),
                ("text_tertiary", theme.text_tertiary),
                ("danger", theme.danger),
            ];
            for (label, color) in body {
                let ratio = contrast(color, theme.surface);
                assert!(ratio >= 4.5, "{name}.{label} is {ratio:.2}:1, under 4.5:1");
            }
            let non_body = [
                ("text_ghost", theme.text_ghost),
                ("accent", theme.accent),
                ("warning", theme.warning),
                ("success", theme.success),
            ];
            for (label, color) in non_body {
                let ratio = contrast(color, theme.surface);
                assert!(ratio >= 3.0, "{name}.{label} is {ratio:.2}:1, under 3.0:1");
            }
        }
    }

    /// Depth is carried by the elevation ladder rather than by borders, so the
    /// steps have to stay ordered — a raised surface that is not actually
    /// lighter than the plane it sits on reads as flat.
    #[test]
    fn the_elevation_ladder_stays_ordered() {
        let dark = Theme::dark();
        assert!(luminance(dark.inset) < luminance(dark.surface));
        assert!(luminance(dark.surface) < luminance(dark.composer));
        assert!(luminance(dark.composer) < luminance(dark.raised));

        let light = Theme::light();
        assert!(luminance(light.inset) < luminance(light.canvas));
        assert!(luminance(light.canvas) < luminance(light.composer));
    }

    /// One color, one meaning: the gauge and the resize handle used to paint a
    /// different blue from the accent, so two colors competed for "active".
    #[test]
    fn the_accent_is_the_only_accent() {
        for theme in [Theme::dark(), Theme::light()] {
            assert_eq!(theme.gauge, theme.accent);
            assert_eq!(theme.resize_handle, theme.accent);
        }
    }

}
