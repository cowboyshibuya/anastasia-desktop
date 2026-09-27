//! Shared pulse clock for the repeating loaders, and the timing catalog the
//! manually-driven tweens read.
//!
//! Ported from Zeron's motion kit (<https://github.com/zeronsh/comet>, MIT).
//! A repeating `with_animation` element requests a redraw every display frame
//! for as long as it is mounted — one working row pinned the whole window at
//! 120 Hz on a ProMotion panel. Loaders instead read their phase from one
//! shared clock: it ticks at ~30 fps, notifies only views that painted a
//! loader recently, and parks itself once the last lease lapses, so a window
//! with no loader mounted schedules nothing at all. Every loader shares one
//! epoch, keeping multi-instance loaders phase-locked.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnyElement, App, EntityId, Global, IntoElement, RenderOnce, Svg, Transformation,
    Window, percentage,
};

// ---------------------------------------------------------------------------
// Timing catalog — cubic beziers and motion specs
// ---------------------------------------------------------------------------

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function (endpoints fixed at
/// (0,0) and (1,1)). Evaluation solves x(t) = input by Newton iteration with a
/// bisection fallback — the standard UnitBezier approach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn coefficients(a: f32, b: f32) -> (f32, f32, f32) {
        let c = 3.0 * a;
        let bb = 3.0 * (b - a) - c;
        let aa = 1.0 - c - bb;
        (aa, bb, c)
    }

    fn sample_x(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        ((a * t + b) * t + c) * t
    }

    fn sample_y(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.y1, self.y2);
        ((a * t + b) * t + c) * t
    }

    fn sample_x_derivative(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        (3.0 * a * t + 2.0 * b) * t + c
    }

    /// Curve parameter `t` for a given progress `x` (both 0..1).
    fn solve_t_for_x(&self, x: f32) -> f32 {
        // Newton–Raphson.
        let mut t = x;
        for _ in 0..8 {
            let err = self.sample_x(t) - x;
            if err.abs() < 1e-6 {
                return t;
            }
            let d = self.sample_x_derivative(t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        // Bisection fallback (x(t) is monotonic for valid CSS beziers).
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..32 {
            let mid = (lo + hi) / 2.0;
            if self.sample_x(mid) < x {
                lo = mid
            } else {
                hi = mid
            }
        }
        (lo + hi) / 2.0
    }

    /// Eased output for input progress `x ∈ [0,1]` (clamped).
    pub fn eval(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        // f32 rounding can push sample_y a hair past 1.0; gpui's animation
        // element asserts `delta ∈ [0,1]` and aborts, so clamp the output hard.
        self.sample_y(self.solve_t_for_x(x)).clamp(0.0, 1.0)
    }
}

/// The signature entrance curve — CSS `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_OUT_EXPO: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// CSS `ease-out` — width/height transitions.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
/// CSS `ease` — quick fades, menu/dialog pops.
pub const EASE: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);

/// One catalog entry: duration + optional delay + curve. The delay is folded
/// into the timeline: the spec runs for `delay + duration` and
/// [`progress`](MotionSpec::progress) holds 0 until the delay has elapsed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpec {
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub curve: CubicBezier,
}

impl MotionSpec {
    pub const fn new(duration_ms: u64, curve: CubicBezier) -> Self {
        Self {
            duration_ms,
            delay_ms: 0,
            curve,
        }
    }

    pub const fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Wall-clock span of the whole timeline (delay + duration).
    pub fn total(&self) -> Duration {
        Duration::from_millis(self.delay_ms + self.duration_ms)
    }

    /// A oneshot gpui [`Animation`] for this spec, delay folded in and the
    /// wall-clock span scaled by [`speed_scale`].
    pub fn animation(&self) -> Animation {
        let spec = *self;
        Animation::new(spec.total().mul_f32(speed_scale()))
            .with_easing(move |delta| spec.progress(delta))
    }

    /// Eased progress (0..1) for a raw timeline delta (0..1 across
    /// [`total`](Self::total)). Pure — unit-testable without a window.
    pub fn progress(&self, raw_delta: f32) -> f32 {
        let total = (self.delay_ms + self.duration_ms) as f32;
        if total <= 0.0 || self.duration_ms == 0 {
            return 1.0;
        }
        let t =
            (raw_delta.clamp(0.0, 1.0) * total - self.delay_ms as f32) / self.duration_ms as f32;
        self.curve.eval(t.clamp(0.0, 1.0))
    }
}

/// Sidebar / pane width transitions: 200ms ease-out.
pub const RESIZE: MotionSpec = MotionSpec::new(200, EASE_OUT);
/// Boot splash exit: 0.5s fade + lift after a 0.15s hold.
pub const SPLASH_OUT: MotionSpec = MotionSpec::new(500, EASE).with_delay(150);
/// Boot splash entrance: the staggered cell wave.
pub const SPLASH_IN: MotionSpec = MotionSpec::new(2400, EASE_OUT_EXPO);

pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// A oneshot width tween, evaluated MANUALLY from render — never through a
/// `with_animation` wrapper.
///
/// gpui keys an animation element's start time by its full global element-id
/// path, so a wrapper that mounts or remounts (a route swap, or an ancestor
/// animation keyed by a fresh epoch) silently replays the tween from t=0.
/// Manual evaluation keeps the element tree's shape constant: a finished or
/// stale tween is exactly the steady state, no matter how the tree around it
/// remounts.
///
/// Reversal is free — the caller builds a new tween from the *current*
/// displayed value, so an interrupted glide continues from where it is rather
/// than jumping back to its old origin.
///
/// The tween stores only where it came from; render supplies the destination
/// each frame. That way a tween can never land on a stale target — the pane's
/// settled width depends on the viewport, which can change mid-glide.
#[derive(Debug, Clone, Copy)]
pub struct WidthTween {
    from: f32,
    started: Instant,
}

impl WidthTween {
    pub fn new(from: f32) -> Self {
        Self {
            from,
            started: Instant::now(),
        }
    }

    /// The width to paint this frame, or `None` once the tween has run out —
    /// at which point the caller paints the settled target instead.
    pub fn value(&self, to: f32) -> Option<f32> {
        self.value_at(self.started.elapsed(), to)
    }

    /// Pure core, so the endpoints and easing are testable without a clock.
    fn value_at(&self, elapsed: Duration, to: f32) -> Option<f32> {
        let total = RESIZE.total().mul_f32(speed_scale());
        let raw = elapsed.as_secs_f32() / total.as_secs_f32();
        (raw < 1.0).then(|| lerp(self.from, to, RESIZE.progress(raw)))
    }
}

/// Wall-clock multiplier for every manually driven tween. A measurement knob:
/// `ANASTASIA_MOTION_SCALE=8` slows motion enough to inspect frame by frame.
pub fn speed_scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("ANASTASIA_MOTION_SCALE")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|scale| scale.is_finite())
            .map(|scale| scale.clamp(0.01, 100.0))
            .unwrap_or(1.0)
    })
}

// ---------------------------------------------------------------------------
// Pulse clock
// ---------------------------------------------------------------------------

/// Repeat-tick interval (~30 fps): visually equivalent for these chunky
/// pulses and spins at a quarter of a ProMotion display's redraws.
const PULSE_TICK: Duration = Duration::from_millis(33);

/// How long a view stays on the tick list after it last painted a loader. One
/// lease outlives a few missed frames; an unmounted loader stops renewing and
/// its view drops off, letting the clock park.
const PULSE_LEASE: Duration = Duration::from_millis(300);

/// The rotating `loader-circle` spinners' period.
const SPINNER_PERIOD: Duration = Duration::from_millis(900);

struct Lease {
    until: Instant,
    /// Notify this view every `stride`-th tick. A view's whole subtree
    /// rebuilds per notify, so a loader on an expensive surface can trade
    /// animation granularity for a cheaper cadence.
    stride: u32,
}

struct PulseClock {
    epoch: Instant,
    leases: HashMap<EntityId, Lease>,
    ticks: u64,
    running: bool,
}

impl Global for PulseClock {}

impl Default for PulseClock {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            leases: HashMap::new(),
            ticks: 0,
            running: false,
        }
    }
}

/// Keep `view` re-rendering at [`PULSE_TICK`] until the lease lapses. A caller
/// that stops leasing stops being notified, and the clock parks once no
/// leases remain — quiescence needs no unsubscribe step.
pub fn pulse_lease(view: EntityId, cx: &mut App) {
    pulse_lease_with_stride(view, 1, cx);
}

/// [`pulse_lease`] at every second tick (~15 fps), for animations whose view
/// is expensive to rebuild and whose motion survives the coarser step — a
/// notify re-renders the view's whole subtree, so cadence is priced per
/// tick, not per animation.
pub fn pulse_lease_slow(view: EntityId, cx: &mut App) {
    pulse_lease_with_stride(view, 2, cx);
}

fn pulse_lease_with_stride(view: EntityId, stride: u32, cx: &mut App) {
    let clock = cx.default_global::<PulseClock>();
    let until = Instant::now() + PULSE_LEASE;
    // A view hosting both a full-rate and a strided loader keeps full rate.
    clock
        .leases
        .entry(view)
        .and_modify(|lease| {
            lease.until = until;
            lease.stride = lease.stride.min(stride);
        })
        .or_insert(Lease { until, stride });
    if clock.running {
        return;
    }
    clock.running = true;
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(PULSE_TICK).await;
            let parked = cx.update(|cx| {
                let clock = cx.default_global::<PulseClock>();
                let now = Instant::now();
                clock.ticks += 1;
                let ticks = clock.ticks;
                clock.leases.retain(|_, lease| lease.until > now);
                if clock.leases.is_empty() {
                    clock.running = false;
                    return true;
                }
                let due = clock
                    .leases
                    .iter_mut()
                    .filter(|(_, lease)| ticks % lease.stride.max(1) as u64 == 0)
                    .map(|(view, lease)| {
                        // Strides re-establish on the render this notify
                        // triggers; without the reset, one full-rate lease
                        // would drag its view's cadence down permanently.
                        lease.stride = u32::MAX;
                        *view
                    })
                    .collect::<Vec<_>>();
                for view in due {
                    cx.notify(view);
                }
                false
            });
            if parked {
                break;
            }
        }
    })
    .detach();
}

/// Phase `[0,1)` of a repeating cycle of `period`, plus a lease keeping `view`
/// re-rendering while its loader stays mounted. Under reduce-motion this is a
/// constant 0 — the cycle's first frame, matching what a repeating
/// `with_animation` held — and nothing is scheduled.
fn pulse_phase(period: Duration, stride: u32, view: EntityId, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return 0.0;
    }
    let clock = cx.default_global::<PulseClock>();
    let phase = (clock.epoch.elapsed().as_secs_f32() / period.as_secs_f32()).fract();
    pulse_lease_with_stride(view, stride, cx);
    phase
}

/// Read a half-cadence phase for paint work embedded in another element, such
/// as the final-glyph streaming caret. It shares the loader clock and lease.
pub fn pulse_phase_slow(period: Duration, view: EntityId, cx: &mut App) -> f32 {
    pulse_phase(period, 2, view, cx)
}

/// A loader element styled from the shared clock's phase. Resolving the phase
/// is deferred to render, where the owning view is known, so call sites need
/// neither a `Window` nor an `EntityId` in scope.
pub fn pulse(period: Duration, render: impl FnOnce(f32) -> AnyElement + 'static) -> Pulse {
    Pulse {
        period,
        stride: 1,
        render: Box::new(render),
    }
}

/// A rotating loader icon riding the shared clock.
pub fn spin(icon: Svg) -> AnyElement {
    spin_with_stride(icon, 1)
}

/// A rotating loader at every second tick (~15 fps — the classic
/// discrete-step spinner cadence). For loaders on expensive surfaces: the
/// sidebar rebuilds its whole subtree per notify, and a session row's working
/// spinner is not worth pricing that at full rate.
pub fn spin_slow(icon: Svg) -> AnyElement {
    spin_with_stride(icon, 2)
}

fn spin_with_stride(icon: Svg, stride: u32) -> AnyElement {
    let mut pulse = pulse(SPINNER_PERIOD, move |phase| {
        icon.with_transformation(Transformation::rotate(percentage(phase)))
            .into_any_element()
    });
    pulse.stride = stride;
    pulse.into_any_element()
}

#[derive(IntoElement)]
pub struct Pulse {
    period: Duration,
    stride: u32,
    render: Box<dyn FnOnce(f32) -> AnyElement>,
}

impl Pulse {
    /// Tick every `stride`-th pulse instead of every one. A view's whole
    /// subtree rebuilds per notify — the pane ticks at the fastest of its
    /// lessees — so a loader mounted for a whole turn on an expensive
    /// surface should ride the coarser cadence.
    pub fn every(mut self, stride: u32) -> Self {
        self.stride = stride.max(1);
        self
    }
}

impl RenderOnce for Pulse {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let phase = pulse_phase(self.period, self.stride, window.current_view(), cx);
        (self.render)(phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_bezier_holds_its_endpoints_and_stays_in_range() {
        for curve in [EASE_OUT, EASE, EASE_OUT_EXPO] {
            assert_eq!(curve.eval(0.0), 0.0);
            assert_eq!(curve.eval(1.0), 1.0);
            // Out-of-range input is clamped, never extrapolated: gpui asserts
            // an animation delta is within [0,1] and aborts otherwise.
            assert_eq!(curve.eval(-1.0), 0.0);
            assert_eq!(curve.eval(2.0), 1.0);
            for step in 0..=20 {
                let value = curve.eval(step as f32 / 20.0);
                assert!(
                    (0.0..=1.0).contains(&value),
                    "{curve:?} left range: {value}"
                );
            }
        }
        // ease-out leads its linear counterpart: most of the distance is
        // covered early. This is what makes a collapsing pane feel like it
        // starts moving the instant it is clicked.
        assert!(EASE_OUT.eval(0.25) > 0.25);
    }

    #[test]
    fn motion_spec_delay_holds_progress_at_zero() {
        // SPLASH_OUT is 500ms of motion after a 150ms hold, so 650ms total.
        assert_eq!(SPLASH_OUT.total(), Duration::from_millis(650));
        assert_eq!(SPLASH_OUT.progress(0.0), 0.0);
        // 150/650 == the end of the delay: still nothing has moved.
        assert_eq!(SPLASH_OUT.progress(150.0 / 650.0), 0.0);
        assert_eq!(SPLASH_OUT.progress(1.0), 1.0);
    }

    #[test]
    fn width_tween_runs_from_its_origin_to_the_supplied_target() {
        let tween = WidthTween::new(260.0);

        // Start: the origin, not the destination.
        assert_eq!(tween.value_at(Duration::ZERO, 0.0), Some(260.0));

        // Mid-flight: strictly between, and past the halfway point already,
        // because the curve is ease-out.
        let midpoint = tween
            .value_at(Duration::from_millis(100), 0.0)
            .expect("still running at 100ms of 200ms");
        assert!(midpoint > 0.0 && midpoint < 260.0, "{midpoint}");
        assert!(
            midpoint < 130.0,
            "ease-out should be past halfway: {midpoint}"
        );

        // Finished: `None`, so the caller paints the settled target itself and
        // the element tree returns to its steady state.
        assert_eq!(tween.value_at(RESIZE.total(), 0.0), None);
        assert_eq!(tween.value_at(Duration::from_secs(5), 0.0), None);
    }

    #[test]
    fn width_tween_target_is_read_per_frame_not_captured() {
        // A resize mid-glide changes where the pane is heading; the tween has
        // to land on the new target rather than the one it started with.
        let tween = WidthTween::new(0.0);
        let toward_260 = tween.value_at(Duration::from_millis(100), 260.0).unwrap();
        let toward_400 = tween.value_at(Duration::from_millis(100), 400.0).unwrap();
        assert!(toward_400 > toward_260);
    }
}
