use gpui::{Hsla, Svg, px, svg, prelude::*};

pub mod menu;

pub fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg().path(path).w(px(size)).h(px(size)).flex_none().text_color(color)
}
