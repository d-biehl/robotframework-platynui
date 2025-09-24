use std::f64::consts::PI;
use std::time::{Duration, Instant};

use platynui_core::platform::{
    PlatformError, PointOrigin, PointerButton, PointerDevice, PointerMotionMode, PointerOverrides,
    PointerProfile, PointerSettings, ScrollDelta,
};
use platynui_core::types::{Point, Rect};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PointerError {
    #[error("no PointerDevice registered")]
    MissingDevice,
    #[error("pointer action failed: {0}")]
    Platform(#[from] PlatformError),
    #[error(
        "pointer could not reach target {expected:?} (actual {actual:?}, threshold {threshold})"
    )]
    EnsureMove { expected: Point, actual: Point, threshold: f64 },
}

pub(crate) struct PointerEngine<'a> {
    device: &'a dyn PointerDevice,
    settings: PointerSettings,
    profile: PointerProfile,
    origin: PointOrigin,
    desktop_bounds: Rect,
    sleep: &'a dyn Fn(Duration),
}

impl<'a> PointerEngine<'a> {
    pub fn new(
        device: &'a dyn PointerDevice,
        desktop_bounds: Rect,
        settings: PointerSettings,
        base_profile: PointerProfile,
        overrides: PointerOverrides,
        sleep: &'a dyn Fn(Duration),
    ) -> Self {
        let mut profile = overrides.profile.clone().unwrap_or(base_profile);
        apply_profile_overrides(&mut profile, &overrides);
        let origin = overrides.origin.unwrap_or(PointOrigin::Desktop);
        Self { device, settings, profile, origin, desktop_bounds, sleep }
    }

    pub fn default_button(&self) -> PointerButton {
        self.settings.default_button
    }

    pub fn move_to(&self, point: Point) -> Result<Point, PointerError> {
        let target = self.resolve_point(point);
        let target = self.clamp_to_desktop(target);
        let start = self.device.position()?;
        self.perform_move(start, target)?;
        self.sleep(self.profile.after_move_delay);
        if self.profile.ensure_move_position {
            self.ensure_position(target)?;
        }
        Ok(target)
    }

    pub fn click(&self, point: Point, button: Option<PointerButton>) -> Result<(), PointerError> {
        let target_button = button.unwrap_or_else(|| self.default_button());
        self.move_to(point)?;
        self.device.press(target_button)?;
        self.sleep(self.profile.after_input_delay);
        self.sleep(self.profile.press_release_delay);
        self.device.release(target_button)?;
        self.sleep(self.profile.after_input_delay);
        self.sleep(self.profile.after_click_delay);
        Ok(())
    }

    pub fn press(&self, button: PointerButton) -> Result<(), PointerError> {
        self.device.press(button)?;
        self.sleep(self.profile.after_input_delay);
        Ok(())
    }

    pub fn release(&self, button: PointerButton) -> Result<(), PointerError> {
        self.device.release(button)?;
        self.sleep(self.profile.after_input_delay);
        Ok(())
    }

    pub fn scroll(&self, delta: ScrollDelta) -> Result<(), PointerError> {
        if delta.horizontal == 0.0 && delta.vertical == 0.0 {
            return Ok(());
        }

        let steps = scroll_steps(delta, self.profile.scroll_step);
        let steps = steps.max(1);
        let mut emitted_x = 0.0;
        let mut emitted_y = 0.0;
        for index in 1..=steps {
            let fraction = index as f64 / steps as f64;
            let target_x = delta.horizontal * fraction;
            let target_y = delta.vertical * fraction;
            let step_delta = ScrollDelta::new(target_x - emitted_x, target_y - emitted_y);
            emitted_x = target_x;
            emitted_y = target_y;
            self.device.scroll(step_delta)?;
            self.sleep(self.profile.scroll_delay);
        }
        self.sleep(self.profile.after_input_delay);
        Ok(())
    }

    pub fn drag(
        &self,
        start: Point,
        end: Point,
        button: Option<PointerButton>,
    ) -> Result<(), PointerError> {
        let active_button = button.unwrap_or_else(|| self.default_button());
        let start_target = self.resolve_point(start);
        let start_target = self.clamp_to_desktop(start_target);
        let end_target = self.resolve_point(end);
        let end_target = self.clamp_to_desktop(end_target);

        let current = self.device.position()?;
        self.perform_move(current, start_target)?;
        self.sleep(self.profile.after_move_delay);
        if self.profile.ensure_move_position {
            self.ensure_position(start_target)?;
        }

        self.device.press(active_button)?;
        self.sleep(self.profile.after_input_delay);

        let current = self.device.position()?;
        self.perform_move(current, end_target)?;
        self.sleep(self.profile.after_move_delay);
        if self.profile.ensure_move_position {
            self.ensure_position(end_target)?;
        }

        self.device.release(active_button)?;
        self.sleep(self.profile.after_input_delay);
        self.sleep(self.profile.after_click_delay);
        Ok(())
    }

    fn resolve_point(&self, point: Point) -> Point {
        match &self.origin {
            PointOrigin::Desktop => point,
            PointOrigin::Bounds(rect) => Point::new(rect.x() + point.x(), rect.y() + point.y()),
            PointOrigin::Absolute(anchor) => {
                Point::new(anchor.x() + point.x(), anchor.y() + point.y())
            }
        }
    }

    fn clamp_to_desktop(&self, point: Point) -> Point {
        let left = self.desktop_bounds.x();
        let top = self.desktop_bounds.y();
        let right = self.desktop_bounds.right();
        let bottom = self.desktop_bounds.bottom();
        let x = point.x().clamp(left, right);
        let y = point.y().clamp(top, bottom);
        Point::new(x, y)
    }

    fn perform_move(&self, start: Point, target: Point) -> Result<(), PointerError> {
        if matches!(self.profile.mode, PointerMotionMode::Direct) {
            self.device.move_to(target)?;
            return Ok(());
        }

        let path = generate_path(start, target, &self.profile);
        for point in path {
            self.device.move_to(point)?;
        }
        Ok(())
    }

    fn ensure_position(&self, target: Point) -> Result<(), PointerError> {
        if self.profile.ensure_move_threshold <= 0.0 {
            return Ok(());
        }
        let deadline = Instant::now() + self.profile.ensure_move_timeout;
        loop {
            let actual = self.device.position()?;
            if distance(actual, target) <= self.profile.ensure_move_threshold {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(PointerError::EnsureMove {
                    expected: target,
                    actual,
                    threshold: self.profile.ensure_move_threshold,
                });
            }
            self.device.move_to(target)?;
            self.sleep(Duration::from_millis(5));
        }
    }

    fn sleep(&self, duration: Duration) {
        if duration.is_zero() {
            return;
        }
        (self.sleep)(duration);
    }
}

fn apply_profile_overrides(profile: &mut PointerProfile, overrides: &PointerOverrides) {
    if let Some(delay) = overrides.after_move_delay {
        profile.after_move_delay = delay;
    }
    if let Some(delay) = overrides.after_input_delay {
        profile.after_input_delay = delay;
    }
    if let Some(delay) = overrides.press_release_delay {
        profile.press_release_delay = delay;
    }
    if let Some(delay) = overrides.after_click_delay {
        profile.after_click_delay = delay;
    }
    if let Some(delay) = overrides.before_next_click_delay {
        profile.before_next_click_delay = delay;
    }
    if let Some(delay) = overrides.multi_click_delay {
        profile.multi_click_delay = delay;
    }
    if let Some(threshold) = overrides.ensure_move_threshold {
        profile.ensure_move_threshold = threshold;
    }
    if let Some(timeout) = overrides.ensure_move_timeout {
        profile.ensure_move_timeout = timeout;
    }
    if let Some(delta) = overrides.scroll_step {
        profile.scroll_step = delta;
    }
    if let Some(delay) = overrides.scroll_delay {
        profile.scroll_delay = delay;
    }
}

fn generate_path(start: Point, target: Point, profile: &PointerProfile) -> Vec<Point> {
    let distance = distance(start, target);
    if distance <= f64::EPSILON {
        return vec![target];
    }

    let steps_per_pixel = profile.steps_per_pixel.max(1.0);
    let mut steps = (distance * steps_per_pixel).ceil() as usize;
    if steps == 0 {
        steps = 1;
    }

    match profile.mode {
        PointerMotionMode::Linear | PointerMotionMode::Direct => {
            generate_linear_path(start, target, steps)
        }
        PointerMotionMode::Bezier => {
            generate_bezier_path(start, target, steps, profile.curve_amplitude)
        }
        PointerMotionMode::Overshoot => generate_overshoot_path(start, target, steps, profile),
        PointerMotionMode::Jitter => {
            generate_jitter_path(start, target, steps, profile.jitter_amplitude)
        }
    }
}

fn generate_linear_path(start: Point, target: Point, steps: usize) -> Vec<Point> {
    let mut path = Vec::with_capacity(steps);
    for index in 1..=steps {
        let t = index as f64 / steps as f64;
        let x = start.x() + (target.x() - start.x()) * t;
        let y = start.y() + (target.y() - start.y()) * t;
        path.push(Point::new(x, y));
    }
    path
}

fn generate_bezier_path(start: Point, target: Point, steps: usize, amplitude: f64) -> Vec<Point> {
    let direction = direction_vector(start, target);
    let perpendicular = (-direction.1, direction.0);
    let mid_x = (start.x() + target.x()) / 2.0 + perpendicular.0 * amplitude;
    let mid_y = (start.y() + target.y()) / 2.0 + perpendicular.1 * amplitude;
    let control = Point::new(mid_x, mid_y);

    let mut path = Vec::with_capacity(steps);
    for index in 1..=steps {
        let t = index as f64 / steps as f64;
        let one_minus_t = 1.0 - t;
        let x = one_minus_t * one_minus_t * start.x()
            + 2.0 * one_minus_t * t * control.x()
            + t * t * target.x();
        let y = one_minus_t * one_minus_t * start.y()
            + 2.0 * one_minus_t * t * control.y()
            + t * t * target.y();
        path.push(Point::new(x, y));
    }
    path
}

fn generate_overshoot_path(
    start: Point,
    target: Point,
    steps: usize,
    profile: &PointerProfile,
) -> Vec<Point> {
    let distance = distance(start, target);
    let direction = direction_vector(start, target);
    let overshoot_dist = distance * profile.overshoot_ratio;
    let overshoot_point = Point::new(
        target.x() + direction.0 * overshoot_dist,
        target.y() + direction.1 * overshoot_dist,
    );

    let mut path = generate_linear_path(start, overshoot_point, steps);
    let settle_steps = profile.overshoot_settle_steps.max(1) as usize;
    path.extend(generate_linear_path(overshoot_point, target, settle_steps));
    path
}

fn generate_jitter_path(start: Point, target: Point, steps: usize, amplitude: f64) -> Vec<Point> {
    let direction = direction_vector(start, target);
    let perpendicular = (-direction.1, direction.0);
    let mut path = Vec::with_capacity(steps);
    for index in 1..=steps {
        let t = index as f64 / steps as f64;
        let base_x = start.x() + (target.x() - start.x()) * t;
        let base_y = start.y() + (target.y() - start.y()) * t;
        let jitter = (t * PI).sin() * amplitude;
        let x = base_x + perpendicular.0 * jitter;
        let y = base_y + perpendicular.1 * jitter;
        path.push(Point::new(x, y));
    }
    path
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x() - a.x()).hypot(b.y() - a.y())
}

fn direction_vector(start: Point, target: Point) -> (f64, f64) {
    let dx = target.x() - start.x();
    let dy = target.y() - start.y();
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f64::EPSILON { (0.0, 0.0) } else { (dx / length, dy / length) }
}

fn scroll_steps(delta: ScrollDelta, step: ScrollDelta) -> usize {
    let horizontal_steps = component_steps(delta.horizontal, step.horizontal);
    let vertical_steps = component_steps(delta.vertical, step.vertical);
    horizontal_steps.max(vertical_steps)
}

fn component_steps(value: f64, base: f64) -> usize {
    if value == 0.0 {
        0
    } else if base.abs() < f64::EPSILON {
        value.abs().ceil() as usize
    } else {
        (value / base).abs().ceil() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{PointerOverrides, PointerProfile, PointerSettings};
    use platynui_core::types::Rect;
    use rstest::rstest;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Debug, PartialEq)]
    enum Action {
        Move(Point),
        Press(PointerButton),
        Release(PointerButton),
        Scroll(ScrollDelta),
    }

    struct RecordingPointer {
        moves: AtomicUsize,
        position: Mutex<Point>,
        log: Mutex<Vec<Action>>,
    }

    impl RecordingPointer {
        fn new() -> Self {
            Self {
                moves: AtomicUsize::new(0),
                position: Mutex::new(Point::new(0.0, 0.0)),
                log: Mutex::new(Vec::new()),
            }
        }

        fn take_log(&self) -> Vec<Action> {
            let mut log = self.log.lock().unwrap();
            let entries = log.clone();
            log.clear();
            entries
        }
    }

    impl PointerDevice for RecordingPointer {
        fn position(&self) -> Result<Point, PlatformError> {
            Ok(*self.position.lock().unwrap())
        }

        fn move_to(&self, point: Point) -> Result<(), PlatformError> {
            self.moves.fetch_add(1, Ordering::SeqCst);
            *self.position.lock().unwrap() = point;
            self.log.lock().unwrap().push(Action::Move(point));
            Ok(())
        }

        fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
            self.log.lock().unwrap().push(Action::Press(button));
            Ok(())
        }

        fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
            self.log.lock().unwrap().push(Action::Release(button));
            Ok(())
        }

        fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
            self.log.lock().unwrap().push(Action::Scroll(delta));
            Ok(())
        }
    }

    fn noop_sleep(_: Duration) {}

    #[rstest]
    fn linear_move_generates_steps() {
        let device = RecordingPointer::new();
        let settings =
            PointerSettings { after_move_delay: Duration::ZERO, ..PointerSettings::default() };
        let mut profile = PointerProfile::named_default(&settings);
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        let engine = PointerEngine::new(
            &device,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            settings,
            profile,
            PointerOverrides::new(),
            &noop_sleep,
        );

        engine.move_to(Point::new(10.0, 0.0)).unwrap();
        assert!(device.moves.load(Ordering::SeqCst) >= 1);
    }

    #[rstest]
    fn bounds_origin_translates_coordinates() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let profile = PointerProfile::named_default(&settings);
        let overrides = PointerOverrides::new()
            .origin(PointOrigin::Bounds(Rect::new(100.0, 200.0, 50.0, 50.0)));
        let engine = PointerEngine::new(
            &device,
            Rect::new(0.0, 0.0, 500.0, 500.0),
            settings,
            profile,
            overrides,
            &noop_sleep,
        );

        engine.move_to(Point::new(5.0, 10.0)).unwrap();
        let log = device.take_log();
        assert!(matches!(log.last(), Some(Action::Move(pt)) if *pt == Point::new(105.0, 210.0)));
    }

    #[rstest]
    fn absolute_origin_translates_coordinates() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let profile = PointerProfile::named_default(&settings);
        let overrides =
            PointerOverrides::new().origin(PointOrigin::Absolute(Point::new(50.0, 75.0)));
        let engine = PointerEngine::new(
            &device,
            Rect::new(0.0, 0.0, 500.0, 500.0),
            settings,
            profile,
            overrides,
            &noop_sleep,
        );

        engine.move_to(Point::new(-10.0, 25.0)).unwrap();
        let log = device.take_log();
        assert!(matches!(log.last(), Some(Action::Move(pt)) if *pt == Point::new(40.0, 100.0)));
    }

    #[rstest]
    fn drag_executes_press_and_release() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default(&settings);
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        let engine = PointerEngine::new(
            &device,
            Rect::new(0.0, 0.0, 300.0, 300.0),
            settings,
            profile,
            PointerOverrides::new(),
            &noop_sleep,
        );

        engine
            .drag(Point::new(10.0, 10.0), Point::new(20.0, 20.0), Some(PointerButton::Right))
            .unwrap();

        let log = device.take_log();
        assert!(log.iter().any(|action| matches!(action, Action::Press(PointerButton::Right))));
        assert!(log.iter().any(|action| matches!(action, Action::Release(PointerButton::Right))));
    }
}
