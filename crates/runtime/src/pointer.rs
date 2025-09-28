use std::f64::consts::PI;
use std::time::{Duration, Instant};

use platynui_core::platform::{
    PlatformError, PointOrigin, PointerAccelerationProfile, PointerButton, PointerDevice,
    PointerMotionMode, ScrollDelta,
};
use platynui_core::types::{Point, Rect, Size};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct PointerSettings {
    pub double_click_time: Duration,
    pub double_click_size: Size,
    pub default_button: PointerButton,
    pub press_release_delay: Duration,
    pub after_input_delay: Duration,
    pub after_click_delay: Duration,
    pub before_next_click_delay: Duration,
    pub multi_click_delay: Duration,
    pub multi_click_threshold: Duration,
    pub after_move_delay: Duration,
    pub ensure_move_timeout: Duration,
    pub ensure_move_threshold: f64,
    pub scroll_step: ScrollDelta,
    pub scroll_delay: Duration,
    pub move_time_per_pixel: Duration,
}

impl Default for PointerSettings {
    fn default() -> Self {
        Self {
            double_click_time: Duration::from_millis(500),
            double_click_size: Size::new(4.0, 4.0),
            default_button: PointerButton::Left,
            press_release_delay: Duration::from_millis(50),
            after_input_delay: Duration::from_millis(35),
            after_click_delay: Duration::from_millis(80),
            before_next_click_delay: Duration::from_millis(120),
            multi_click_delay: Duration::from_millis(500),
            multi_click_threshold: Duration::from_millis(900),
            after_move_delay: Duration::from_millis(40),
            ensure_move_timeout: Duration::from_millis(250),
            ensure_move_threshold: 2.0,
            scroll_step: ScrollDelta::new(0.0, -120.0),
            scroll_delay: Duration::from_millis(40),
            move_time_per_pixel: Duration::from_micros(800),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointerProfile {
    pub mode: PointerMotionMode,
    pub steps_per_pixel: f64,
    pub max_move_duration: Duration,
    pub speed_factor: f64,
    pub acceleration_profile: PointerAccelerationProfile,
    pub overshoot_ratio: f64,
    pub overshoot_settle_steps: u32,
    pub curve_amplitude: f64,
    pub jitter_amplitude: f64,
    pub after_move_delay: Duration,
    pub after_input_delay: Duration,
    pub press_release_delay: Duration,
    pub after_click_delay: Duration,
    pub before_next_click_delay: Duration,
    pub multi_click_delay: Duration,
    pub ensure_move_position: bool,
    pub ensure_move_threshold: f64,
    pub ensure_move_timeout: Duration,
    pub scroll_step: ScrollDelta,
    pub scroll_delay: Duration,
    pub move_time_per_pixel: Duration,
}

impl PointerProfile {
    pub fn named_default(settings: &PointerSettings) -> Self {
        Self {
            mode: PointerMotionMode::Linear,
            steps_per_pixel: 1.5,
            max_move_duration: Duration::from_millis(600),
            speed_factor: 1.0,
            acceleration_profile: PointerAccelerationProfile::SmoothStep,
            overshoot_ratio: 0.08,
            overshoot_settle_steps: 3,
            curve_amplitude: 4.0,
            jitter_amplitude: 1.5,
            after_move_delay: settings.after_move_delay,
            after_input_delay: settings.after_input_delay,
            press_release_delay: settings.press_release_delay,
            after_click_delay: settings.after_click_delay,
            before_next_click_delay: settings.before_next_click_delay,
            multi_click_delay: settings.multi_click_delay,
            ensure_move_position: true,
            ensure_move_threshold: settings.ensure_move_threshold,
            ensure_move_timeout: settings.ensure_move_timeout,
            scroll_step: settings.scroll_step,
            scroll_delay: settings.scroll_delay,
            move_time_per_pixel: settings.move_time_per_pixel,
        }
    }

    pub fn with_mode(mut self, mode: PointerMotionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_speed_factor(mut self, speed: f64) -> Self {
        self.speed_factor = speed;
        self
    }

    pub fn with_curve_amplitude(mut self, amplitude: f64) -> Self {
        self.curve_amplitude = amplitude;
        self
    }
}

impl Default for PointerProfile {
    fn default() -> Self {
        PointerProfile::named_default(&PointerSettings::default())
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PointerOverrides {
    pub origin: Option<PointOrigin>,
    pub profile: Option<PointerProfile>,
    pub after_move_delay: Option<Duration>,
    pub after_input_delay: Option<Duration>,
    pub press_release_delay: Option<Duration>,
    pub after_click_delay: Option<Duration>,
    pub before_next_click_delay: Option<Duration>,
    pub multi_click_delay: Option<Duration>,
    pub ensure_move_threshold: Option<f64>,
    pub ensure_move_timeout: Option<Duration>,
    pub scroll_step: Option<ScrollDelta>,
    pub scroll_delay: Option<Duration>,
    pub max_move_duration: Option<Duration>,
    pub move_time_per_pixel: Option<Duration>,
}

impl PointerOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn origin(mut self, origin: PointOrigin) -> Self {
        self.origin = Some(origin);
        self
    }

    pub fn profile(mut self, profile: PointerProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn after_move_delay(mut self, delay: Duration) -> Self {
        self.after_move_delay = Some(delay);
        self
    }

    pub fn after_input_delay(mut self, delay: Duration) -> Self {
        self.after_input_delay = Some(delay);
        self
    }

    pub fn press_release_delay(mut self, delay: Duration) -> Self {
        self.press_release_delay = Some(delay);
        self
    }

    pub fn after_click_delay(mut self, delay: Duration) -> Self {
        self.after_click_delay = Some(delay);
        self
    }

    pub fn before_next_click_delay(mut self, delay: Duration) -> Self {
        self.before_next_click_delay = Some(delay);
        self
    }

    pub fn multi_click_delay(mut self, delay: Duration) -> Self {
        self.multi_click_delay = Some(delay);
        self
    }

    pub fn ensure_move_threshold(mut self, threshold: f64) -> Self {
        self.ensure_move_threshold = Some(threshold);
        self
    }

    pub fn ensure_move_timeout(mut self, timeout: Duration) -> Self {
        self.ensure_move_timeout = Some(timeout);
        self
    }

    pub fn scroll_step(mut self, delta: ScrollDelta) -> Self {
        self.scroll_step = Some(delta);
        self
    }

    pub fn scroll_delay(mut self, delay: Duration) -> Self {
        self.scroll_delay = Some(delay);
        self
    }

    pub fn move_duration(mut self, duration: Duration) -> Self {
        self.max_move_duration = Some(duration);
        self
    }

    pub fn move_time_per_pixel(mut self, duration: Duration) -> Self {
        self.move_time_per_pixel = Some(duration);
        self
    }
}

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
        let distance = distance(start, target);
        let total_duration = self.desired_move_duration(distance);

        if matches!(self.profile.mode, PointerMotionMode::Direct) {
            let start = Instant::now();
            self.device.move_to(target)?;
            if !total_duration.is_zero() {
                let elapsed = start.elapsed();
                if total_duration > elapsed {
                    self.sleep(total_duration - elapsed);
                }
            }
            return Ok(());
        }

        let path = generate_path(start, target, &self.profile);
        if path.is_empty() {
            return Ok(());
        }

        if total_duration.is_zero() {
            for point in path {
                self.device.move_to(point)?;
            }
            return Ok(());
        }

        let steps = path.len();
        let start_time = Instant::now();
        for (index, point) in path.iter().enumerate() {
            self.device.move_to(*point)?;
            let desired = total_duration.mul_f64((index + 1) as f64 / steps as f64);
            let elapsed = start_time.elapsed();
            if desired > elapsed {
                self.sleep(desired - elapsed);
            }
        }

        Ok(())
    }

    fn desired_move_duration(&self, distance: f64) -> Duration {
        if distance <= f64::EPSILON {
            return Duration::ZERO;
        }

        let per_pixel = self.profile.move_time_per_pixel;
        let base = if per_pixel.is_zero() {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(per_pixel.as_secs_f64() * distance)
        };

        let max = self.profile.max_move_duration;
        if max.is_zero() {
            base
        } else if base.is_zero() {
            max
        } else {
            base.min(max)
        }
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
    if let Some(duration) = overrides.max_move_duration {
        profile.max_move_duration = duration;
    }
    if let Some(duration) = overrides.move_time_per_pixel {
        profile.move_time_per_pixel = duration;
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
    use super::{PointerOverrides, PointerProfile, PointerSettings};
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
    fn motion_respects_max_duration() {
        let device = RecordingPointer::new();
        let settings =
            PointerSettings { after_move_delay: Duration::ZERO, ..PointerSettings::default() };
        let mut profile = PointerProfile::named_default(&settings);
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.steps_per_pixel = 1.0;
        profile.mode = PointerMotionMode::Linear;
        profile.max_move_duration = Duration::from_millis(120);
        profile.move_time_per_pixel = Duration::from_millis(80);

        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let expected_steps = (distance(Point::new(0.0, 0.0), Point::new(4.0, 0.0))
            * profile.steps_per_pixel)
            .ceil() as usize;
        let max_move_duration = profile.max_move_duration;

        let engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            settings,
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0)).unwrap();
        let recorded = sleeps.lock().unwrap().clone();
        assert!(!recorded.is_empty());
        assert_eq!(device.moves.load(Ordering::SeqCst), expected_steps);

        let expected_step = if expected_steps > 1 {
            Duration::from_secs_f64(max_move_duration.as_secs_f64() / expected_steps as f64)
        } else {
            max_move_duration
        };

        for duration in recorded {
            assert!((duration.as_secs_f64() - expected_step.as_secs_f64()).abs() < 1e-3);
        }
    }

    #[rstest]
    fn motion_scales_with_distance() {
        let device = RecordingPointer::new();
        let settings =
            PointerSettings { after_move_delay: Duration::ZERO, ..PointerSettings::default() };
        let mut profile = PointerProfile::named_default(&settings);
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.steps_per_pixel = 1.0;
        profile.mode = PointerMotionMode::Linear;
        profile.max_move_duration = Duration::ZERO;
        profile.move_time_per_pixel = Duration::from_millis(10);

        let total_sleep = Mutex::new(Duration::ZERO);
        let sleep = |duration: Duration| {
            *total_sleep.lock().unwrap() += duration;
        };

        let engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            settings,
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        engine.move_to(Point::new(2.0, 0.0)).unwrap();
        let short_duration = *total_sleep.lock().unwrap();
        *total_sleep.lock().unwrap() = Duration::ZERO;
        engine.move_to(Point::new(6.0, 0.0)).unwrap();
        let long_duration = *total_sleep.lock().unwrap();

        assert!(long_duration > short_duration);
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
