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
}

impl Default for PointerSettings {
    fn default() -> Self {
        Self {
            double_click_time: Duration::from_millis(500),
            double_click_size: Size::new(4.0, 4.0),
            default_button: PointerButton::Left,
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
    pub fn named_default() -> Self {
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
            after_move_delay: Duration::from_millis(40),
            after_input_delay: Duration::from_millis(35),
            press_release_delay: Duration::from_millis(50),
            after_click_delay: Duration::from_millis(80),
            before_next_click_delay: Duration::from_millis(120),
            multi_click_delay: Duration::from_millis(500),
            ensure_move_position: true,
            ensure_move_threshold: 2.0,
            ensure_move_timeout: Duration::from_millis(250),
            scroll_step: ScrollDelta::new(0.0, -120.0),
            scroll_delay: Duration::from_millis(40),
            move_time_per_pixel: Duration::from_micros(800),
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
        PointerProfile::named_default()
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
    pub speed_factor: Option<f64>,
    pub acceleration_profile: Option<PointerAccelerationProfile>,
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

    pub fn speed_factor(mut self, speed: f64) -> Self {
        self.speed_factor = Some(speed);
        self
    }

    pub fn acceleration_profile(mut self, profile: PointerAccelerationProfile) -> Self {
        self.acceleration_profile = Some(profile);
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

    pub fn click(
        &self,
        point: Point,
        button: Option<PointerButton>,
        last_click: &mut Option<Instant>,
    ) -> Result<(), PointerError> {
        let target_button = button.unwrap_or_else(|| self.default_button());
        self.enforce_inter_click_delay(last_click);
        self.move_to(point)?;
        self.device.press(target_button)?;
        self.sleep(self.profile.after_input_delay);
        self.sleep(self.profile.press_release_delay);
        self.device.release(target_button)?;
        self.sleep(self.profile.after_input_delay);
        self.sleep(self.profile.after_click_delay);
        *last_click = Some(Instant::now());
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
            let fraction = easing_fraction(&self.profile, index, steps);
            let desired = total_duration.mul_f64(fraction);
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
            let speed =
                if self.profile.speed_factor > 0.0 { self.profile.speed_factor } else { 1.0 };
            let adjusted_distance = distance / speed;
            Duration::from_secs_f64(per_pixel.as_secs_f64() * adjusted_distance)
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

    fn enforce_inter_click_delay(&self, last_click: &mut Option<Instant>) {
        if let Some(previous) = *last_click {
            let elapsed = previous.elapsed();
            if !self.profile.multi_click_delay.is_zero() && elapsed > self.profile.multi_click_delay
            {
                *last_click = None;
                return;
            }

            if self.profile.before_next_click_delay > Duration::ZERO {
                if let Some(remaining) = self.profile.before_next_click_delay.checked_sub(elapsed) {
                    if !remaining.is_zero() {
                        self.sleep(remaining);
                    }
                }
            }
        }
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
    if let Some(speed) = overrides.speed_factor {
        profile.speed_factor = speed;
    }
    if let Some(acceleration) = overrides.acceleration_profile {
        profile.acceleration_profile = acceleration;
    }
}

fn easing_fraction(profile: &PointerProfile, step_index: usize, steps: usize) -> f64 {
    if steps == 0 {
        return 1.0;
    }
    let t = ((step_index + 1) as f64 / steps as f64).clamp(0.0, 1.0);
    match profile.acceleration_profile {
        PointerAccelerationProfile::Constant => t,
        PointerAccelerationProfile::EaseIn => t * t,
        PointerAccelerationProfile::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        PointerAccelerationProfile::SmoothStep => t * t * (3.0 - 2.0 * t),
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
    use platynui_core::platform::pointer_devices;
    use platynui_core::types::Rect;
    use platynui_platform_mock::{PointerLogEntry, reset_pointer_state, take_pointer_log};
    use rstest::rstest;
    use serial_test::serial;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

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
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.acceleration_profile = PointerAccelerationProfile::Constant;
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
        let profile = PointerProfile::named_default();
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
        let profile = PointerProfile::named_default();
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
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.steps_per_pixel = 1.0;
        profile.mode = PointerMotionMode::Linear;
        profile.max_move_duration = Duration::from_millis(120);
        profile.move_time_per_pixel = Duration::from_millis(80);
        profile.acceleration_profile = PointerAccelerationProfile::Constant;

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

        let mut previous = Duration::ZERO;
        for duration in recorded {
            let step_duration = duration.saturating_sub(previous);
            assert!((step_duration.as_secs_f64() - expected_step.as_secs_f64()).abs() < 1e-3);
            previous = duration;
        }
    }

    #[rstest]
    fn motion_scales_with_distance() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.steps_per_pixel = 1.0;
        profile.mode = PointerMotionMode::Linear;
        profile.max_move_duration = Duration::ZERO;
        profile.move_time_per_pixel = Duration::from_millis(10);
        profile.acceleration_profile = PointerAccelerationProfile::Constant;

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
    fn speed_factor_scales_duration() {
        let total_sleep = Mutex::new(Duration::ZERO);
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                *total_sleep.lock().unwrap() += duration;
            }
        };

        let slow_device = RecordingPointer::new();
        let mut slow_profile = PointerProfile::named_default();
        slow_profile.after_move_delay = Duration::ZERO;
        slow_profile.after_input_delay = Duration::ZERO;
        slow_profile.ensure_move_position = false;
        slow_profile.mode = PointerMotionMode::Linear;
        slow_profile.steps_per_pixel = 1.0;
        slow_profile.max_move_duration = Duration::ZERO;
        slow_profile.move_time_per_pixel = Duration::from_millis(10);
        slow_profile.acceleration_profile = PointerAccelerationProfile::Constant;

        let slow_engine = PointerEngine::new(
            &slow_device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            slow_profile.clone(),
            PointerOverrides::new(),
            &sleep,
        );

        slow_engine.move_to(Point::new(10.0, 0.0)).unwrap();
        let slow_duration = *total_sleep.lock().unwrap();

        *total_sleep.lock().unwrap() = Duration::ZERO;

        let fast_device = RecordingPointer::new();
        let mut fast_profile = slow_profile;
        fast_profile.speed_factor = 2.0;

        let fast_engine = PointerEngine::new(
            &fast_device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            fast_profile,
            PointerOverrides::new(),
            &sleep,
        );

        fast_engine.move_to(Point::new(10.0, 0.0)).unwrap();
        let fast_duration = *total_sleep.lock().unwrap();

        assert!(fast_duration < slow_duration);
        let ratio = fast_duration.as_secs_f64() / slow_duration.as_secs_f64();
        assert!((ratio - 0.5).abs() < 0.05, "ratio {ratio}");
    }

    #[rstest]
    fn acceleration_profile_ease_in_increases_step_durations() {
        let durations = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                durations.lock().unwrap().push(duration);
            }
        };

        let device = RecordingPointer::new();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.mode = PointerMotionMode::Linear;
        profile.steps_per_pixel = 1.0;
        profile.max_move_duration = Duration::from_millis(120);
        profile.move_time_per_pixel = Duration::ZERO;
        profile.acceleration_profile = PointerAccelerationProfile::EaseIn;
        let profile_clone = profile.clone();
        let engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0)).unwrap();
        let recorded = durations.lock().unwrap().clone();
        assert_eq!(recorded.len(), 4);
        let mut previous = Duration::ZERO;
        let increments: Vec<Duration> = recorded
            .iter()
            .map(|&value| {
                let slice = value.saturating_sub(previous);
                previous = value;
                slice
            })
            .collect();
        let total: Duration = increments.iter().copied().fold(Duration::ZERO, |acc, d| acc + d);
        let total_secs = total.as_secs_f64();
        assert!(total_secs > 0.0);
        let mut previous_fraction = 0.0;
        for (index, actual) in increments.iter().enumerate() {
            let fraction = super::easing_fraction(&profile_clone, index, recorded.len());
            let expected_slice = fraction - previous_fraction;
            previous_fraction = fraction;
            let actual_slice = actual.as_secs_f64() / total_secs;
            assert!(
                (actual_slice - expected_slice).abs() < 0.05,
                "index {index}, expected {expected_slice}, actual {actual_slice}"
            );
        }
    }

    #[rstest]
    fn acceleration_profile_ease_out_decreases_step_durations() {
        let durations = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                durations.lock().unwrap().push(duration);
            }
        };

        let device = RecordingPointer::new();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.mode = PointerMotionMode::Linear;
        profile.steps_per_pixel = 1.0;
        profile.max_move_duration = Duration::from_millis(120);
        profile.move_time_per_pixel = Duration::ZERO;
        profile.acceleration_profile = PointerAccelerationProfile::EaseOut;
        let profile_clone = profile.clone();
        let engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0)).unwrap();
        let recorded = durations.lock().unwrap().clone();
        assert_eq!(recorded.len(), 4);
        let mut previous = Duration::ZERO;
        let increments: Vec<Duration> = recorded
            .iter()
            .map(|&value| {
                let slice = value.saturating_sub(previous);
                previous = value;
                slice
            })
            .collect();
        let total: Duration = increments.iter().copied().fold(Duration::ZERO, |acc, d| acc + d);
        let total_secs = total.as_secs_f64();
        assert!(total_secs > 0.0);
        let mut previous_fraction = 0.0;
        for (index, actual) in increments.iter().enumerate() {
            let fraction = super::easing_fraction(&profile_clone, index, recorded.len());
            let expected_slice = fraction - previous_fraction;
            previous_fraction = fraction;
            let actual_slice = actual.as_secs_f64() / total_secs;
            assert!(
                (actual_slice - expected_slice).abs() < 0.05,
                "index {index}, expected {expected_slice}, actual {actual_slice}"
            );
        }
    }

    #[rstest]
    fn click_respects_before_next_delay() {
        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let device = RecordingPointer::new();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.before_next_click_delay = Duration::from_millis(80);
        profile.multi_click_delay = Duration::from_millis(200);
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        let mut last_click = None;
        engine.click(Point::new(10.0, 10.0), None, &mut last_click).unwrap();

        last_click = Some(Instant::now() - Duration::from_millis(20));
        sleeps.lock().unwrap().clear();
        engine.click(Point::new(12.0, 10.0), None, &mut last_click).unwrap();
        let recorded = sleeps.lock().unwrap().clone();
        assert_eq!(recorded.len(), 1);
        let waited = recorded[0];
        assert!(waited >= Duration::from_millis(59) && waited <= Duration::from_millis(80));

        last_click = Some(Instant::now() - Duration::from_millis(400));
        sleeps.lock().unwrap().clear();
        engine.click(Point::new(14.0, 10.0), None, &mut last_click).unwrap();
        assert!(sleeps.lock().unwrap().is_empty());
    }

    #[rstest]
    #[serial]
    fn mock_pointer_speed_factor_scales_duration() {
        reset_pointer_state();
        let device = pointer_devices().next().expect("mock pointer registered");

        let slow_sleeps = Mutex::new(Vec::new());
        let slow_sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                slow_sleeps.lock().unwrap().push(duration);
            }
        };

        let mut base_profile = PointerProfile::named_default();
        base_profile.after_move_delay = Duration::ZERO;
        base_profile.after_input_delay = Duration::ZERO;
        base_profile.ensure_move_position = false;
        base_profile.mode = PointerMotionMode::Linear;
        base_profile.steps_per_pixel = 1.0;
        base_profile.max_move_duration = Duration::ZERO;
        base_profile.move_time_per_pixel = Duration::from_millis(10);
        base_profile.acceleration_profile = PointerAccelerationProfile::Constant;

        let slow_engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            base_profile.clone(),
            PointerOverrides::new(),
            &slow_sleep,
        );

        slow_engine.move_to(Point::new(20.0, 0.0)).unwrap();
        let slow_total = slow_sleeps.lock().unwrap().last().copied().unwrap_or(Duration::ZERO);
        assert!(slow_total > Duration::ZERO);

        reset_pointer_state();
        let fast_sleeps = Mutex::new(Vec::new());
        let fast_sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                fast_sleeps.lock().unwrap().push(duration);
            }
        };

        let mut fast_profile = base_profile;
        fast_profile.speed_factor = 2.0;

        let fast_engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            fast_profile,
            PointerOverrides::new(),
            &fast_sleep,
        );

        fast_engine.move_to(Point::new(20.0, 0.0)).unwrap();
        let fast_total = fast_sleeps.lock().unwrap().last().copied().unwrap_or(Duration::ZERO);

        assert!(fast_total > Duration::ZERO);
        assert!(fast_total < slow_total);
        let ratio = fast_total.as_secs_f64() / slow_total.as_secs_f64();
        assert!(ratio < 0.6 && ratio > 0.3, "unexpected ratio {ratio}");
    }

    fn mock_sleep_increments(entries: &[Duration]) -> Vec<Duration> {
        let mut prev = Duration::ZERO;
        entries
            .iter()
            .map(|&value| {
                let slice = value.saturating_sub(prev);
                prev = value;
                slice
            })
            .collect()
    }

    #[rstest]
    #[serial]
    fn mock_pointer_acceleration_ease_in_trends_upwards() {
        reset_pointer_state();
        let device = pointer_devices().next().expect("mock pointer registered");
        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.mode = PointerMotionMode::Linear;
        profile.steps_per_pixel = 1.0;
        profile.max_move_duration = Duration::from_millis(160);
        profile.move_time_per_pixel = Duration::ZERO;
        profile.acceleration_profile = PointerAccelerationProfile::EaseIn;

        let engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0)).unwrap();
        let increments = mock_sleep_increments(&sleeps.lock().unwrap());
        assert_eq!(increments.len(), 4);
        assert!(
            increments.windows(2).all(|w| w[0] <= w[1] + Duration::from_millis(2)),
            "{:?}",
            increments
        );
    }

    #[rstest]
    #[serial]
    fn mock_pointer_acceleration_ease_out_trends_downwards() {
        reset_pointer_state();
        let device = pointer_devices().next().expect("mock pointer registered");
        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.mode = PointerMotionMode::Linear;
        profile.steps_per_pixel = 1.0;
        profile.max_move_duration = Duration::from_millis(160);
        profile.move_time_per_pixel = Duration::ZERO;
        profile.acceleration_profile = PointerAccelerationProfile::EaseOut;

        let engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0)).unwrap();
        let increments = mock_sleep_increments(&sleeps.lock().unwrap());
        assert_eq!(increments.len(), 4);
        assert!(
            increments.windows(2).all(|w| w[0] >= w[1] - Duration::from_millis(2)),
            "{:?}",
            increments
        );
    }

    #[rstest]
    #[serial]
    fn mock_pointer_click_enforces_before_next_delay() {
        reset_pointer_state();
        let device = pointer_devices().next().expect("mock pointer registered");
        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;
        profile.before_next_click_delay = Duration::from_millis(80);
        profile.multi_click_delay = Duration::from_millis(200);

        let engine = PointerEngine::new(
            device,
            Rect::new(-10.0, -10.0, 20.0, 20.0),
            PointerSettings::default(),
            profile,
            PointerOverrides::new(),
            &sleep,
        );

        let mut last_click = None;
        engine.click(Point::new(0.0, 0.0), None, &mut last_click).unwrap();
        sleeps.lock().unwrap().clear();

        last_click = Some(Instant::now() - Duration::from_millis(25));
        engine.click(Point::new(0.0, 0.0), None, &mut last_click).unwrap();
        let enforced = sleeps.lock().unwrap().first().copied().unwrap_or(Duration::ZERO);
        assert!(enforced >= Duration::from_millis(50) && enforced <= Duration::from_millis(90));

        let log = take_pointer_log();
        let press_count =
            log.iter().filter(|entry| matches!(entry, PointerLogEntry::Press(_))).count();
        assert!(press_count >= 2);
    }

    #[rstest]
    fn drag_executes_press_and_release() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
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
