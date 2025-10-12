use std::f64::consts::PI;
use std::time::{Duration, Instant};

use platynui_core::platform::{
    PlatformError, PointOrigin, PointerAccelerationProfile, PointerButton, PointerDevice, PointerMotionMode,
    ScrollDelta,
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
    #[error("pointer could not reach target {expected:?} (actual {actual:?}, threshold {threshold})")]
    EnsureMove { expected: Point, actual: Point, threshold: f64 },
    #[error("click count must be greater than zero (got {provided})")]
    InvalidClickCount { provided: u32 },
}

pub(crate) struct PointerEngine<'a> {
    device: &'a dyn PointerDevice,
    desktop_bounds: Rect,
    settings: PointerSettings,
    profile: PointerProfile,
    last_click: Option<ClickStamp>,
    sleep: &'a (dyn Fn(Duration) + Send + Sync),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickStamp {
    pub time: Instant,
    pub position: Point,
}

struct EffectiveProfile<'a> {
    profile: &'a PointerProfile,
    overrides: Option<&'a PointerOverrides>,
    settings: &'a PointerSettings,
}

impl<'a> PointerEngine<'a> {
    pub fn new(
        device: &'a dyn PointerDevice,
        desktop_bounds: Rect,
        settings: PointerSettings,
        profile: PointerProfile,
        sleep: &'a (dyn Fn(Duration) + Send + Sync),
    ) -> Self {
        Self { device, desktop_bounds, settings, profile, last_click: None, sleep }
    }

    pub fn set_settings(&mut self, settings: PointerSettings) {
        self.settings = settings;
        self.last_click = None;
    }

    pub fn set_profile(&mut self, profile: PointerProfile) {
        self.profile = profile;
    }

    pub fn set_desktop_bounds(&mut self, bounds: Rect) {
        self.desktop_bounds = bounds;
    }

    pub fn default_button(&self) -> PointerButton {
        self.settings.default_button
    }

    fn effective_profile<'b>(&'b self, overrides: Option<&'b PointerOverrides>) -> EffectiveProfile<'b> {
        let profile = overrides.and_then(|o| o.profile.as_ref()).unwrap_or(&self.profile);
        EffectiveProfile::new(profile, overrides, &self.settings)
    }

    fn resolve_origin(&self, overrides: Option<&PointerOverrides>) -> PointOrigin {
        overrides.and_then(|o| o.origin.clone()).unwrap_or(PointOrigin::Desktop)
    }

    pub fn move_to(&mut self, point: Point, overrides: Option<&PointerOverrides>) -> Result<Point, PointerError> {
        let effective = self.effective_profile(overrides);
        let origin = self.resolve_origin(overrides);
        let target = self.resolve_point(point, &origin);
        let target = self.clamp_to_desktop(target);
        let start = self.device.position()?;
        self.perform_move(start, target, &effective)?;
        self.sleep(effective.after_move_delay());
        if effective.ensure_move_position() {
            self.ensure_position(target, &effective)?;
        }
        Ok(target)
    }

    pub fn click(
        &mut self,
        point: Point,
        button: Option<PointerButton>,
        overrides: Option<&PointerOverrides>,
    ) -> Result<(), PointerError> {
        self.multi_click(point, button, 1, overrides)
    }

    pub fn multi_click(
        &mut self,
        point: Point,
        button: Option<PointerButton>,
        clicks: u32,
        overrides: Option<&PointerOverrides>,
    ) -> Result<(), PointerError> {
        if clicks == 0 {
            return Err(PointerError::InvalidClickCount { provided: clicks });
        }

        let target_button = button.unwrap_or(self.settings.default_button);
        let anchor = self.move_to(point, overrides)?;
        let (press_release_delay, after_click_delay, after_input_delay, multi_click_delay, before_next_click_delay) = {
            let effective = self.effective_profile(overrides);
            (
                effective.press_release_delay(),
                effective.after_click_delay(),
                effective.after_input_delay(),
                effective.multi_click_delay(),
                effective.before_next_click_delay(),
            )
        };

        self.enforce_inter_click_delay(multi_click_delay, before_next_click_delay, after_click_delay, anchor);

        let inter_between_clicks = if clicks > 1 && !self.settings.double_click_time.is_zero() {
            self.settings.double_click_time / 2
        } else {
            Duration::ZERO
        };

        for index in 0..clicks {
            self.device.press(target_button)?;
            self.sleep(press_release_delay);
            self.device.release(target_button)?;

            let is_last = index + 1 == clicks;
            if !is_last && !inter_between_clicks.is_zero() {
                self.sleep(inter_between_clicks);
            }
        }

        self.sleep(after_click_delay);
        self.sleep(after_input_delay);

        self.last_click = Some(ClickStamp { time: Instant::now(), position: anchor });

        Ok(())
    }

    pub fn press(&mut self, button: PointerButton, overrides: Option<&PointerOverrides>) -> Result<(), PointerError> {
        let effective = self.effective_profile(overrides);
        self.device.press(button)?;
        self.sleep(effective.press_release_delay());
        self.sleep(effective.after_input_delay());
        Ok(())
    }

    pub fn release(&mut self, button: PointerButton, overrides: Option<&PointerOverrides>) -> Result<(), PointerError> {
        let effective = self.effective_profile(overrides);
        self.device.release(button)?;
        self.sleep(effective.press_release_delay());
        self.sleep(effective.after_input_delay());
        Ok(())
    }

    pub fn scroll(&mut self, delta: ScrollDelta, overrides: Option<&PointerOverrides>) -> Result<(), PointerError> {
        if delta.horizontal == 0.0 && delta.vertical == 0.0 {
            return Ok(());
        }

        let effective = self.effective_profile(overrides);
        let step = effective.scroll_step();
        let steps = scroll_steps(delta, step).max(1);
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
            self.sleep(effective.scroll_delay());
        }
        self.sleep(effective.after_input_delay());
        Ok(())
    }

    pub fn drag(
        &mut self,
        start: Point,
        end: Point,
        button: Option<PointerButton>,
        overrides: Option<&PointerOverrides>,
    ) -> Result<(), PointerError> {
        let effective = self.effective_profile(overrides);
        let origin = self.resolve_origin(overrides);
        let active_button = button.unwrap_or(self.settings.default_button);
        let start_target = self.clamp_to_desktop(self.resolve_point(start, &origin));
        let end_target = self.clamp_to_desktop(self.resolve_point(end, &origin));

        let current = self.device.position()?;
        self.perform_move(current, start_target, &effective)?;
        self.sleep(effective.after_move_delay());
        if effective.ensure_move_position() {
            self.ensure_position(start_target, &effective)?;
        }

        self.device.press(active_button)?;
        self.sleep(effective.after_input_delay());

        let current = self.device.position()?;
        self.perform_move(current, end_target, &effective)?;
        self.sleep(effective.after_move_delay());
        if effective.ensure_move_position() {
            self.ensure_position(end_target, &effective)?;
        }

        self.device.release(active_button)?;
        self.sleep(effective.after_input_delay());
        self.sleep(effective.after_click_delay());
        Ok(())
    }

    fn resolve_point(&self, point: Point, origin: &PointOrigin) -> Point {
        match origin {
            PointOrigin::Desktop => point,
            PointOrigin::Bounds(rect) => Point::new(rect.x() + point.x(), rect.y() + point.y()),
            PointOrigin::Absolute(anchor) => Point::new(anchor.x() + point.x(), anchor.y() + point.y()),
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

    fn perform_move(&self, start: Point, target: Point, effective: &EffectiveProfile) -> Result<(), PointerError> {
        let profile = effective.profile();
        let distance = distance(start, target);
        let total_duration = self.desired_move_duration(distance, effective);

        if matches!(profile.mode, PointerMotionMode::Direct) {
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

        let path = generate_path(start, target, profile);
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
            let fraction = easing_fraction(effective.acceleration_profile(), index, steps);
            let desired = total_duration.mul_f64(fraction);
            let elapsed = start_time.elapsed();
            if desired > elapsed {
                self.sleep(desired - elapsed);
            }
        }

        Ok(())
    }

    fn desired_move_duration(&self, distance: f64, effective: &EffectiveProfile) -> Duration {
        if distance <= f64::EPSILON {
            return Duration::ZERO;
        }

        let per_pixel = effective.move_time_per_pixel();
        let base = if per_pixel.is_zero() {
            Duration::ZERO
        } else {
            let speed = if effective.speed_factor() > 0.0 { effective.speed_factor() } else { 1.0 };
            let adjusted_distance = distance / speed;
            Duration::from_secs_f64(per_pixel.as_secs_f64() * adjusted_distance)
        };

        let max = effective.max_move_duration();
        if max.is_zero() {
            base
        } else if base.is_zero() {
            max
        } else {
            base.min(max)
        }
    }

    fn ensure_position(&self, target: Point, effective: &EffectiveProfile) -> Result<(), PointerError> {
        let threshold = effective.ensure_move_threshold();
        if threshold <= 0.0 {
            return Ok(());
        }
        let deadline = Instant::now() + effective.ensure_move_timeout();
        loop {
            let actual = self.device.position()?;
            if distance(actual, target) <= threshold {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(PointerError::EnsureMove { expected: target, actual, threshold });
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

    fn enforce_inter_click_delay(
        &mut self,
        multi_click_delay: Duration,
        before_next_click_delay: Duration,
        after_click_delay: Duration,
        target: Point,
    ) {
        if let Some(stamp) = self.last_click {
            if !self.is_within_double_click_bounds(stamp.position, target) {
                self.last_click = None;
                return;
            }

            let elapsed = stamp.time.elapsed();
            if !multi_click_delay.is_zero() && elapsed > multi_click_delay {
                self.last_click = None;
                return;
            }

            let target_wait = if before_next_click_delay.is_zero() {
                after_click_delay
            } else if self.settings.double_click_time.is_zero() {
                before_next_click_delay
            } else {
                before_next_click_delay.min(self.settings.double_click_time / 2)
            };

            if !target_wait.is_zero() && elapsed < target_wait {
                self.sleep(target_wait - elapsed);
            }
        }
    }

    fn is_within_double_click_bounds(&self, anchor: Point, point: Point) -> bool {
        let size = self.settings.double_click_size;
        if size.is_empty() {
            return true;
        }

        let half_width = (size.width() / 2.0).max(0.0);
        let half_height = (size.height() / 2.0).max(0.0);
        let within_width = half_width <= 0.0 || (point.x() - anchor.x()).abs() <= half_width;
        let within_height = half_height <= 0.0 || (point.y() - anchor.y()).abs() <= half_height;
        within_width && within_height
    }
}

impl<'a> EffectiveProfile<'a> {
    fn new(
        profile: &'a PointerProfile,
        overrides: Option<&'a PointerOverrides>,
        settings: &'a PointerSettings,
    ) -> Self {
        Self { profile, overrides, settings }
    }

    fn profile(&self) -> &'a PointerProfile {
        self.profile
    }

    fn after_move_delay(&self) -> Duration {
        self.overrides.and_then(|o| o.after_move_delay).unwrap_or(self.profile.after_move_delay)
    }

    fn after_input_delay(&self) -> Duration {
        self.overrides.and_then(|o| o.after_input_delay).unwrap_or(self.profile.after_input_delay)
    }

    fn press_release_delay(&self) -> Duration {
        self.overrides.and_then(|o| o.press_release_delay).unwrap_or(self.profile.press_release_delay)
    }

    fn after_click_delay(&self) -> Duration {
        self.overrides.and_then(|o| o.after_click_delay).unwrap_or(self.profile.after_click_delay)
    }

    fn before_next_click_delay(&self) -> Duration {
        self.overrides.and_then(|o| o.before_next_click_delay).unwrap_or(self.profile.before_next_click_delay)
    }

    fn multi_click_delay(&self) -> Duration {
        let mut delay = self.overrides.and_then(|o| o.multi_click_delay).unwrap_or(self.profile.multi_click_delay);
        if !self.settings.double_click_time.is_zero() && !delay.is_zero() {
            delay = delay.min(self.settings.double_click_time);
        }
        delay
    }

    fn ensure_move_position(&self) -> bool {
        self.profile.ensure_move_position
    }

    fn ensure_move_threshold(&self) -> f64 {
        self.overrides.and_then(|o| o.ensure_move_threshold).unwrap_or(self.profile.ensure_move_threshold)
    }

    fn ensure_move_timeout(&self) -> Duration {
        self.overrides.and_then(|o| o.ensure_move_timeout).unwrap_or(self.profile.ensure_move_timeout)
    }

    fn max_move_duration(&self) -> Duration {
        self.overrides.and_then(|o| o.max_move_duration).unwrap_or(self.profile.max_move_duration)
    }

    fn move_time_per_pixel(&self) -> Duration {
        self.overrides.and_then(|o| o.move_time_per_pixel).unwrap_or(self.profile.move_time_per_pixel)
    }

    fn speed_factor(&self) -> f64 {
        self.overrides.and_then(|o| o.speed_factor).unwrap_or(self.profile.speed_factor)
    }

    fn acceleration_profile(&self) -> PointerAccelerationProfile {
        self.overrides.and_then(|o| o.acceleration_profile).unwrap_or(self.profile.acceleration_profile)
    }

    fn scroll_step(&self) -> ScrollDelta {
        self.overrides.and_then(|o| o.scroll_step).unwrap_or(self.profile.scroll_step)
    }

    fn scroll_delay(&self) -> Duration {
        self.overrides.and_then(|o| o.scroll_delay).unwrap_or(self.profile.scroll_delay)
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
        PointerMotionMode::Linear | PointerMotionMode::Direct => generate_linear_path(start, target, steps),
        PointerMotionMode::Bezier => generate_bezier_path(start, target, steps, profile.curve_amplitude),
        PointerMotionMode::Overshoot => generate_overshoot_path(start, target, steps, profile),
        PointerMotionMode::Jitter => generate_jitter_path(start, target, steps, profile.jitter_amplitude),
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
        let x = one_minus_t * one_minus_t * start.x() + 2.0 * one_minus_t * t * control.x() + t * t * target.x();
        let y = one_minus_t * one_minus_t * start.y() + 2.0 * one_minus_t * t * control.y() + t * t * target.y();
        path.push(Point::new(x, y));
    }
    path
}

fn generate_overshoot_path(start: Point, target: Point, steps: usize, profile: &PointerProfile) -> Vec<Point> {
    let distance = distance(start, target);
    let direction = direction_vector(start, target);
    let overshoot_dist = distance * profile.overshoot_ratio;
    let overshoot_point =
        Point::new(target.x() + direction.0 * overshoot_dist, target.y() + direction.1 * overshoot_dist);

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

fn easing_fraction(acceleration: PointerAccelerationProfile, step_index: usize, steps: usize) -> f64 {
    if steps == 0 {
        return 1.0;
    }
    let t = ((step_index + 1) as f64 / steps as f64).clamp(0.0, 1.0);
    match acceleration {
        PointerAccelerationProfile::Constant => t,
        PointerAccelerationProfile::EaseIn => t * t,
        PointerAccelerationProfile::EaseOut => {
            let inverse = 1.0 - t;
            1.0 - inverse * inverse
        }
        PointerAccelerationProfile::SmoothStep => t * t * (3.0 - 2.0 * t),
    }
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
    use super::{ClickStamp, PointerOverrides, PointerProfile, PointerSettings};
    use platynui_core::types::{Rect, Size};
    use platynui_platform_mock::{MOCK_POINTER, PointerLogEntry, reset_pointer_state, take_pointer_log};
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
            Self { moves: AtomicUsize::new(0), position: Mutex::new(Point::new(0.0, 0.0)), log: Mutex::new(Vec::new()) }
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
        let mut engine = PointerEngine::new(&device, Rect::new(0.0, 0.0, 100.0, 100.0), settings, profile, &noop_sleep);

        engine.move_to(Point::new(10.0, 0.0), None).unwrap();
        assert!(device.moves.load(Ordering::SeqCst) >= 1);
    }

    #[rstest]
    fn bounds_origin_translates_coordinates() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let profile = PointerProfile::named_default();
        let overrides = PointerOverrides::new().origin(PointOrigin::Bounds(Rect::new(100.0, 200.0, 50.0, 50.0)));
        let mut engine = PointerEngine::new(&device, Rect::new(0.0, 0.0, 500.0, 500.0), settings, profile, &noop_sleep);

        engine.move_to(Point::new(5.0, 10.0), Some(&overrides)).unwrap();
        let log = device.take_log();
        assert!(matches!(log.last(), Some(Action::Move(pt)) if *pt == Point::new(105.0, 210.0)));
    }

    #[rstest]
    fn absolute_origin_translates_coordinates() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let profile = PointerProfile::named_default();
        let overrides = PointerOverrides::new().origin(PointOrigin::Absolute(Point::new(50.0, 75.0)));
        let mut engine = PointerEngine::new(&device, Rect::new(0.0, 0.0, 500.0, 500.0), settings, profile, &noop_sleep);

        engine.move_to(Point::new(-10.0, 25.0), Some(&overrides)).unwrap();
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

        let expected_steps =
            (distance(Point::new(0.0, 0.0), Point::new(4.0, 0.0)) * profile.steps_per_pixel).ceil() as usize;
        let max_move_duration = profile.max_move_duration;

        let mut engine =
            PointerEngine::new(&device, Rect::new(-4000.0, -2000.0, 8000.0, 4000.0), settings, profile, &sleep);

        engine.move_to(Point::new(4.0, 0.0), None).unwrap();
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

        let mut engine =
            PointerEngine::new(&device, Rect::new(-4000.0, -2000.0, 8000.0, 4000.0), settings, profile, &sleep);

        engine.move_to(Point::new(2.0, 0.0), None).unwrap();
        let short_duration = *total_sleep.lock().unwrap();
        *total_sleep.lock().unwrap() = Duration::ZERO;
        engine.move_to(Point::new(6.0, 0.0), None).unwrap();
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

        let mut slow_engine = PointerEngine::new(
            &slow_device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            slow_profile.clone(),
            &sleep,
        );

        slow_engine.move_to(Point::new(10.0, 0.0), None).unwrap();
        let slow_duration = *total_sleep.lock().unwrap();

        *total_sleep.lock().unwrap() = Duration::ZERO;

        let fast_device = RecordingPointer::new();
        let mut fast_profile = slow_profile;
        fast_profile.speed_factor = 2.0;

        let mut fast_engine = PointerEngine::new(
            &fast_device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            fast_profile,
            &sleep,
        );

        fast_engine.move_to(Point::new(10.0, 0.0), None).unwrap();
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
        let mut engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0), None).unwrap();
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
            let fraction = super::easing_fraction(profile_clone.acceleration_profile, index, recorded.len());
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
        let mut engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0), None).unwrap();
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
            let fraction = super::easing_fraction(profile_clone.acceleration_profile, index, recorded.len());
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
        profile.press_release_delay = Duration::from_millis(80);
        profile.after_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.before_next_click_delay = Duration::from_millis(80);
        profile.multi_click_delay = Duration::from_millis(200);
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let mut engine = PointerEngine::new(
            &device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            &sleep,
        );

        engine.click(Point::new(10.0, 10.0), None, None).unwrap();

        engine.last_click =
            Some(ClickStamp { time: Instant::now() - Duration::from_millis(20), position: Point::new(10.0, 10.0) });
        sleeps.lock().unwrap().clear();
        engine.click(Point::new(12.0, 10.0), None, None).unwrap();
        let recorded = sleeps.lock().unwrap().clone();
        assert!(!recorded.is_empty());
        let enforced = *recorded.last().unwrap();
        assert_duration_approx(enforced, Duration::from_millis(80));
        assert!(engine.last_click.is_some());

        engine.last_click =
            Some(ClickStamp { time: Instant::now() - Duration::from_millis(400), position: Point::new(12.0, 10.0) });
        sleeps.lock().unwrap().clear();
        engine.click(Point::new(14.0, 10.0), None, None).unwrap();
        let recorded = sleeps.lock().unwrap().clone();
        assert!(recorded.len() <= 1);
        if let Some(&duration) = recorded.first() {
            assert_duration_approx(duration, Duration::from_millis(80));
        }
    }

    #[rstest]
    fn click_skips_delay_when_target_outside_double_click_bounds() {
        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let device = RecordingPointer::new();
        let settings = PointerSettings { double_click_size: Size::new(4.0, 4.0), ..PointerSettings::default() };

        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::ZERO;
        profile.before_next_click_delay = Duration::from_millis(80);
        profile.multi_click_delay = Duration::from_millis(200);
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let mut engine =
            PointerEngine::new(&device, Rect::new(-4000.0, -2000.0, 8000.0, 4000.0), settings, profile, &sleep);

        engine.last_click =
            Some(ClickStamp { time: Instant::now() - Duration::from_millis(20), position: Point::new(0.0, 0.0) });

        engine.click(Point::new(200.0, 0.0), None, None).unwrap();

        assert!(sleeps.lock().unwrap().is_empty());
        assert!(engine.last_click.is_some());
    }

    #[rstest]
    #[serial]
    fn mock_pointer_speed_factor_scales_duration() {
        reset_pointer_state();
        let device = &MOCK_POINTER;

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

        let mut slow_engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            base_profile.clone(),
            &slow_sleep,
        );

        slow_engine.move_to(Point::new(20.0, 0.0), None).unwrap();
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

        let mut fast_engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            fast_profile,
            &fast_sleep,
        );

        fast_engine.move_to(Point::new(20.0, 0.0), None).unwrap();
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
        let device = &MOCK_POINTER;
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

        let mut engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0), None).unwrap();
        let increments = mock_sleep_increments(&sleeps.lock().unwrap());
        assert_eq!(increments.len(), 4);
        assert!(increments.windows(2).all(|w| w[0] <= w[1] + Duration::from_millis(2)), "{:?}", increments);
    }

    #[rstest]
    #[serial]
    fn mock_pointer_acceleration_ease_out_trends_downwards() {
        reset_pointer_state();
        let device = &MOCK_POINTER;
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

        let mut engine = PointerEngine::new(
            device,
            Rect::new(-4000.0, -2000.0, 8000.0, 4000.0),
            PointerSettings::default(),
            profile,
            &sleep,
        );

        engine.move_to(Point::new(4.0, 0.0), None).unwrap();
        let increments = mock_sleep_increments(&sleeps.lock().unwrap());
        assert_eq!(increments.len(), 4);
        assert!(increments.windows(2).all(|w| w[0] >= w[1] - Duration::from_millis(2)), "{:?}", increments);
    }

    #[rstest]
    #[serial]
    fn mock_pointer_click_enforces_before_next_delay() {
        reset_pointer_state();
        let device = &MOCK_POINTER;
        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let mut profile = PointerProfile::named_default();
        let configured_delay = Duration::from_millis(80);
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::from_millis(80);
        profile.after_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;
        profile.before_next_click_delay = configured_delay;
        profile.multi_click_delay = Duration::from_millis(200);

        let mut engine = PointerEngine::new(
            device,
            Rect::new(-10.0, -10.0, 20.0, 20.0),
            PointerSettings::default(),
            profile,
            &sleep,
        );

        let half_double = device.double_click_time().ok().flatten().map(|time| time / 2).unwrap_or(Duration::ZERO);
        let target_total = if configured_delay.is_zero() {
            half_double
        } else if half_double.is_zero() {
            configured_delay
        } else {
            configured_delay.min(half_double)
        };

        engine.click(Point::new(0.0, 0.0), None, None).unwrap();
        sleeps.lock().unwrap().clear();

        engine.last_click =
            Some(ClickStamp { time: Instant::now() - Duration::from_millis(25), position: Point::new(0.0, 0.0) });
        engine.click(Point::new(0.0, 0.0), None, None).unwrap();
        let enforced = sleeps.lock().unwrap().first().copied().unwrap_or(Duration::ZERO);
        let expected_sleep = target_total.checked_sub(Duration::from_millis(25)).unwrap_or(Duration::ZERO);
        assert_duration_approx(enforced, expected_sleep);

        let log = take_pointer_log();
        let press_count = log.iter().filter(|entry| matches!(entry, PointerLogEntry::Press(_))).count();
        assert!(press_count >= 2);
    }

    #[rstest]
    fn multi_click_emits_multiple_events() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::ZERO;
        profile.before_next_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let mut engine =
            PointerEngine::new(&device, Rect::new(-100.0, -100.0, 200.0, 200.0), settings, profile, &noop_sleep);

        engine.multi_click(Point::new(5.0, 5.0), Some(PointerButton::Right), 3, None).unwrap();

        let log = device.take_log();
        let presses = log.iter().filter(|action| matches!(action, Action::Press(PointerButton::Right))).count();
        let releases = log.iter().filter(|action| matches!(action, Action::Release(PointerButton::Right))).count();
        assert_eq!(presses, 3);
        assert_eq!(releases, 3);
        assert!(engine.last_click.is_some());
    }

    #[rstest]
    fn multi_click_rejects_zero_count() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::ZERO;
        profile.before_next_click_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let mut engine =
            PointerEngine::new(&device, Rect::new(-50.0, -50.0, 100.0, 100.0), settings, profile, &noop_sleep);

        let error = engine.multi_click(Point::new(0.0, 0.0), None, 0, None).unwrap_err();
        match error {
            PointerError::InvalidClickCount { provided } => assert_eq!(provided, 0),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[rstest]
    fn multi_click_limits_post_click_delay() {
        let device = RecordingPointer::new();
        let settings = PointerSettings {
            double_click_time: Duration::from_millis(250),
            double_click_size: Size::new(10.0, 10.0),
            ..PointerSettings::default()
        };

        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::from_millis(80);
        profile.after_click_delay = Duration::from_millis(200);
        profile.before_next_click_delay = Duration::ZERO;
        profile.multi_click_delay = Duration::from_millis(300);
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let mut engine = PointerEngine::new(&device, Rect::new(-10.0, -10.0, 20.0, 20.0), settings, profile, &sleep);

        engine.multi_click(Point::new(0.0, 0.0), None, 3, None).unwrap();

        let recorded = sleeps.lock().unwrap().clone();
        let inter_press = Duration::from_millis(80);
        let inter_between = Duration::from_millis(125);
        let after_click = Duration::from_millis(200);
        assert_eq!(recorded.len(), 6);
        assert_duration_approx(recorded[0], inter_press);
        assert_duration_approx(recorded[1], inter_between);
        assert_duration_approx(recorded[2], inter_press);
        assert_duration_approx(recorded[3], inter_between);
        assert_duration_approx(recorded[4], inter_press);
        assert_eq!(recorded[5], after_click);
    }

    #[rstest]
    fn single_click_emits_only_after_click_delay() {
        let device = RecordingPointer::new();
        let settings = PointerSettings { double_click_time: Duration::from_millis(250), ..PointerSettings::default() };

        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.after_input_delay = Duration::ZERO;
        profile.press_release_delay = Duration::ZERO;
        profile.after_click_delay = Duration::from_millis(40);
        profile.before_next_click_delay = Duration::from_millis(80);
        profile.multi_click_delay = Duration::from_millis(200);
        profile.ensure_move_position = false;
        profile.move_time_per_pixel = Duration::ZERO;
        profile.max_move_duration = Duration::ZERO;

        let sleeps = Mutex::new(Vec::new());
        let sleep = |duration: Duration| {
            if duration > Duration::ZERO {
                sleeps.lock().unwrap().push(duration);
            }
        };

        let mut engine = PointerEngine::new(&device, Rect::new(-10.0, -10.0, 20.0, 20.0), settings, profile, &sleep);

        engine.click(Point::new(0.0, 0.0), None, None).unwrap();

        let recorded = sleeps.lock().unwrap().clone();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], Duration::from_millis(40));
        assert!(engine.last_click.is_some());
    }

    fn assert_duration_approx(actual: Duration, expected: Duration) {
        let delta = (actual.as_secs_f64() - expected.as_secs_f64()).abs();
        assert!(delta <= 0.002, "expected {expected:?}, got {actual:?}");
    }

    #[rstest]
    fn drag_executes_press_and_release() {
        let device = RecordingPointer::new();
        let settings = PointerSettings::default();
        let mut profile = PointerProfile::named_default();
        profile.after_move_delay = Duration::ZERO;
        profile.ensure_move_position = false;
        let mut engine = PointerEngine::new(&device, Rect::new(0.0, 0.0, 300.0, 300.0), settings, profile, &noop_sleep);

        engine.drag(Point::new(10.0, 10.0), Point::new(20.0, 20.0), Some(PointerButton::Right), None).unwrap();

        let log = device.take_log();
        assert!(log.iter().any(|action| matches!(action, Action::Press(PointerButton::Right))));
        assert!(log.iter().any(|action| matches!(action, Action::Release(PointerButton::Right))));
    }
}
