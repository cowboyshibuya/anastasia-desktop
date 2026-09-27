use std::f32::consts::{PI, TAU};
use std::time::Duration;

use gpui::{
    App, BorderStyle, Bounds, Hsla, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window,
    canvas, div, point, px, quad, size,
};
use uuid::Uuid;

use super::motion;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrbVariant {
    Lattice,
    Lens,
    Ring,
    Helix,
    Morph,
}

impl OrbVariant {
    pub fn for_id(id: Uuid) -> Self {
        match id.as_u128() % 5 {
            0 => Self::Lattice,
            1 => Self::Lens,
            2 => Self::Ring,
            3 => Self::Helix,
            _ => Self::Morph,
        }
    }
}

#[derive(IntoElement)]
pub struct Orb {
    variant: OrbVariant,
    size: Pixels,
}

impl Orb {
    pub fn for_id(id: Uuid) -> Self {
        Self {
            variant: OrbVariant::for_id(id),
            size: px(14.0),
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Orb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = Theme::current(cx).accent;
        let variant = self.variant;
        let edge = self.size;
        motion::pulse(Duration::from_millis(1_800), move |phase| {
            div()
                .size(edge)
                .flex_none()
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            paint_orb(window, bounds, color, variant, phase)
                        },
                    )
                    .size_full(),
                )
                .into_any_element()
        })
        .every(2)
    }
}

fn paint_orb(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    color: Hsla,
    variant: OrbVariant,
    phase: f32,
) {
    let edge = f32::from(bounds.size.width.min(bounds.size.height));
    let center = edge / 2.0;
    let dots = match variant {
        OrbVariant::Lattice => lattice_dots(phase),
        OrbVariant::Lens => lens_dots(phase),
        OrbVariant::Ring => ring_dots(phase),
        OrbVariant::Helix => helix_dots(phase),
        OrbVariant::Morph => morph_dots(phase),
    };
    for (x, y, opacity, scale) in dots {
        let dot = (edge * 0.13 * scale).max(1.0);
        let dot_bounds = Bounds::new(
            point(
                bounds.origin.x + px(center + x * edge * 0.36 - dot / 2.0),
                bounds.origin.y + px(center + y * edge * 0.36 - dot / 2.0),
            ),
            size(px(dot), px(dot)),
        );
        window.paint_quad(quad(
            dot_bounds,
            px(dot / 2.0),
            color.opacity(opacity.clamp(0.12, 1.0)),
            px(0.0),
            gpui::transparent_black(),
            BorderStyle::default(),
        ));
    }
}

type Dot = (f32, f32, f32, f32);

fn lattice_dots(phase: f32) -> Vec<Dot> {
    (-1..=1)
        .flat_map(|y| {
            (-1..=1).map(move |x| {
                let delay = ((x + y + 2) as f32 / 4.0 + phase) % 1.0;
                let wave = (delay * TAU).sin() * 0.5 + 0.5;
                (x as f32, y as f32, 0.2 + 0.8 * wave, 0.75 + 0.35 * wave)
            })
        })
        .collect()
}

fn lens_dots(phase: f32) -> Vec<Dot> {
    (0..8)
        .map(|index| {
            let side = if index < 4 { -1.0 } else { 1.0 };
            let angle = (index % 4) as f32 / 4.0 * TAU + phase * TAU * side;
            let x = side * 0.42 + angle.cos() * 0.55;
            let y = angle.sin() * 0.9;
            (x, y, 0.25 + 0.75 * (angle.sin() * 0.5 + 0.5), 0.9)
        })
        .collect()
}

fn ring_dots(phase: f32) -> Vec<Dot> {
    (0..8)
        .map(|index| {
            let angle = index as f32 / 8.0 * TAU - PI / 2.0;
            let chase = ((phase - index as f32 / 8.0).rem_euclid(1.0) * TAU).cos() * 0.5 + 0.5;
            (
                angle.cos(),
                angle.sin(),
                0.18 + 0.82 * chase,
                0.85 + 0.25 * chase,
            )
        })
        .collect()
}

fn helix_dots(phase: f32) -> Vec<Dot> {
    [-0.65_f32, 0.0, 0.65]
        .into_iter()
        .enumerate()
        .flat_map(|(ring, y)| {
            (0..5).map(move |index| {
                let angle = index as f32 / 5.0 * TAU + phase * TAU + ring as f32 * 0.35;
                let depth = angle.sin() * 0.5 + 0.5;
                (angle.cos(), y, 0.16 + 0.84 * depth, 0.65 + 0.35 * depth)
            })
        })
        .collect()
}

fn morph_dots(phase: f32) -> Vec<Dot> {
    let amount = (phase * TAU).sin() * 0.5 + 0.5;
    (0..8)
        .map(|index| {
            let angle = index as f32 / 8.0 * TAU - PI / 2.0;
            let circle = (angle.cos(), angle.sin());
            let square_scale = 1.0 / circle.0.abs().max(circle.1.abs()).max(0.01);
            let square = (circle.0 * square_scale, circle.1 * square_scale);
            (
                circle.0 + (square.0 - circle.0) * amount,
                circle.1 + (square.1 - circle.1) * amount,
                0.35 + 0.65 * ((phase + index as f32 / 8.0) * TAU).sin().abs(),
                0.85,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orb_variant_is_stable_and_covers_the_curated_set() {
        let variants = (0..5)
            .map(|value| OrbVariant::for_id(Uuid::from_u128(value)))
            .collect::<Vec<_>>();
        assert_eq!(
            variants,
            vec![
                OrbVariant::Lattice,
                OrbVariant::Lens,
                OrbVariant::Ring,
                OrbVariant::Helix,
                OrbVariant::Morph,
            ]
        );
        let id = Uuid::new_v4();
        assert_eq!(OrbVariant::for_id(id), OrbVariant::for_id(id));
    }
}
