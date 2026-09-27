use gpui::{
    App, Hsla, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window, canvas, div, point,
    px, size,
};
use crate::theme::Theme;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DaemonGlyphState {
    #[default]
    Idle,
    Thinking,
    Reading,
    Editing,
    Executing,
    Waiting,
    Permission,
    Error,
    Complete,
}

#[allow(dead_code)]
#[derive(IntoElement)]
pub struct DaemonGlyph {
    state: DaemonGlyphState,
    size: Pixels,
    color_override: Option<Hsla>,
}

impl DaemonGlyph {
    pub fn new(state: DaemonGlyphState) -> Self {
        Self {
            state,
            size: px(12.0),
            color_override: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color_override = Some(color);
        self
    }
}

impl RenderOnce for DaemonGlyph {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let state = self.state;
        let glyph_size = self.size;
        let glyph_color = self.color_override.unwrap_or_else(|| match state {
            DaemonGlyphState::Idle => theme.text_ghost,
            DaemonGlyphState::Thinking
            | DaemonGlyphState::Reading
            | DaemonGlyphState::Editing
            | DaemonGlyphState::Executing => theme.accent,
            DaemonGlyphState::Waiting => theme.text_secondary,
            DaemonGlyphState::Permission => theme.warning,
            DaemonGlyphState::Error => theme.danger,
            DaemonGlyphState::Complete => theme.success,
        });

        // 8x8 bitmap grid scaled to glyph_size
        let grid_size = 8;
        let bitmap: [u8; 8] = match state {
            DaemonGlyphState::Idle => [
                0b00011000,
                0b00100100,
                0b01000010,
                0b10000001,
                0b10000001,
                0b01000010,
                0b00100100,
                0b00011000,
            ],
            DaemonGlyphState::Complete => [
                0b00011000,
                0b00111100,
                0b01111110,
                0b11111111,
                0b11111111,
                0b01111110,
                0b00111100,
                0b00011000,
            ],
            DaemonGlyphState::Thinking => [
                0b00011000,
                0b01010100,
                0b10101010,
                0b01010101,
                0b10101010,
                0b01010101,
                0b00101000,
                0b00011000,
            ],
            DaemonGlyphState::Reading => [
                0b00011000,
                0b00100100,
                0b11111111,
                0b10000001,
                0b10000001,
                0b11111111,
                0b00100100,
                0b00011000,
            ],
            DaemonGlyphState::Editing => [
                0b00111100,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b01100110,
                0b00111100,
            ],
            DaemonGlyphState::Executing => [
                0b11110000,
                0b11110000,
                0b11001100,
                0b11001100,
                0b00110011,
                0b00110011,
                0b00001111,
                0b00001111,
            ],
            DaemonGlyphState::Waiting => [
                0b11111111,
                0b10000001,
                0b10000001,
                0b10000001,
                0b10000001,
                0b10000001,
                0b10000001,
                0b11111111,
            ],
            DaemonGlyphState::Permission => [
                0b00111100,
                0b01000010,
                0b01000010,
                0b11111111,
                0b11011011,
                0b11011011,
                0b11111111,
                0b11111111,
            ],
            DaemonGlyphState::Error => [
                0b10000001,
                0b01000010,
                0b00100100,
                0b00011000,
                0b00011000,
                0b00100100,
                0b01000010,
                0b10000001,
            ],
        };

        div()
            .w(glyph_size)
            .h(glyph_size)
            .flex_none()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let cell_w = bounds.size.width / grid_size as f32;
                        let cell_h = bounds.size.height / grid_size as f32;
                        for (row_idx, row) in bitmap.iter().enumerate() {
                            for col_idx in 0..8 {
                                if (row >> (7 - col_idx)) & 1 == 1 {
                                    let x = bounds.origin.x + (col_idx as f32 * cell_w);
                                    let y = bounds.origin.y + (row_idx as f32 * cell_h);
                                    let pixel_rect = gpui::Bounds {
                                        origin: point(x, y),
                                        size: size(cell_w.ceil(), cell_h.ceil()),
                                    };
                                    window.paint_quad(gpui::fill(pixel_rect, glyph_color));
                                }
                            }
                        }
                    },
                )
                .size_full(),
            )
    }
}
