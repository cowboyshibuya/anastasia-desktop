//! The composer's small paint palette, retained from the prototype editor.

use gpui::{Hsla, rgb};
use super::highlight::TokenClass;
use crate::theme::Theme;

pub const SANS_FAMILY: &str = "Geist";

pub struct Palette { dark: bool, ghost: Hsla, tertiary: Hsla, added: Hsla, removed: Hsla }

impl Palette {
    pub fn from_theme(theme: &Theme) -> Self {
        Self { dark: theme.is_dark, ghost: theme.text_ghost,
            tertiary: theme.text_tertiary, added: theme.success, removed: theme.danger }
    }

    pub fn token(&self, class: TokenClass) -> Hsla {
        let hue = |dark, light| rgb(if self.dark { dark } else { light }).into();
        match class {
            TokenClass::Keyword => hue(0xC98BC0, 0x9A4B92),
            TokenClass::Literal | TokenClass::Number => hue(0xD9A05B, 0x9A6019),
            TokenClass::String => hue(0x94C08A, 0x3F7A36),
            TokenClass::Comment => self.ghost,
            TokenClass::Type | TokenClass::Function => hue(0x8FB8D9, 0x2F6690),
            TokenClass::Meta => self.tertiary,
            TokenClass::Added => self.added,
            TokenClass::Removed => self.removed,
        }
    }
}
