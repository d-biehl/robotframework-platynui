//! Pure decision logic for the live mouse picker.
//!
//! This module holds no egui or platform state — it decides, per tick, whether
//! picking should resolve the element under the cursor, stay idle, or stop. The
//! egui layer feeds it the current armed/available state, the held modifiers,
//! and the cursor position (when available), and acts on the returned
//! [`PickerDecision`]. Keeping it pure makes the core behaviour unit-testable
//! without a display or a real platform.

use platynui_core::types::Point;

/// The modifier keys the picker tracks. The configured activation combination
/// is expressed as the set that must be held (default Ctrl+Alt+Shift).
/// Serializable so the configured combination persists with the Inspector's
/// other settings (eframe storage); missing fields deserialize as unset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Modifiers {
    pub const CTRL_ALT_SHIFT: Modifiers = Modifiers { ctrl: true, alt: true, shift: true };

    /// True when no modifier is set. An empty set is not a valid activation
    /// combination — it would mean "picking is active whenever no key is held".
    pub fn is_empty(self) -> bool {
        !(self.ctrl || self.alt || self.shift)
    }

    /// Human-readable combination label, e.g. `"Ctrl+Alt+Shift"`.
    pub fn label(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.join("+")
    }
}

/// What the egui layer should do this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickerDecision {
    /// Resolve the element at `point` and reveal it, tagged with `epoch`.
    Resolve { point: Point, epoch: u64 },
    /// Do nothing this tick (not armed/active, combination not held, position
    /// unavailable, or the cursor has not moved to a new element).
    Idle,
}

/// Only re-resolve once the cursor has moved at least this far (logical px),
/// so a stationary cursor does not thrash the tree / async work.
const MOVE_THRESHOLD: f64 = 2.0;

/// Pure picker state machine.
pub struct PickerState {
    /// Feature toggle — picking only happens while armed.
    armed: bool,
    /// The modifier combination that must be held to pick.
    combo: Modifiers,
    /// Whether picking is actively following the cursor right now.
    active: bool,
    /// Monotonic epoch bumped whenever picking stops, so async results from a
    /// previous active span are discarded rather than moving the selection.
    epoch: u64,
    /// Last cursor point that triggered a resolve, for the move threshold.
    last_point: Option<Point>,
}

impl Default for PickerState {
    fn default() -> Self {
        Self { armed: false, combo: Modifiers::CTRL_ALT_SHIFT, active: false, epoch: 0, last_point: None }
    }
}

impl PickerState {
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn combo(&self) -> Modifiers {
        self.combo
    }

    /// Change the activation combination. An empty set is rejected (it would
    /// activate picking whenever no key is held). If picking is active under
    /// the old combination it stops — the epoch bump discards in-flight
    /// results, and the next tick re-evaluates against the new combination.
    pub fn set_combo(&mut self, combo: Modifiers) {
        if combo.is_empty() || combo == self.combo {
            return;
        }
        self.stop();
        self.combo = combo;
    }

    /// True when `epoch` matches the current active span — the egui layer tags
    /// each resolve with the epoch from [`PickerDecision::Resolve`] and drops
    /// results whose epoch is stale (picking stopped in between).
    pub fn is_current_epoch(&self, epoch: u64) -> bool {
        self.epoch == epoch
    }

    /// Arm or disarm the picker. Disarming stops picking immediately and bumps
    /// the epoch so in-flight results are discarded.
    pub fn set_armed(&mut self, armed: bool) {
        if self.armed && !armed {
            self.stop();
        }
        self.armed = armed;
    }

    fn stop(&mut self) {
        if self.active {
            self.active = false;
            self.epoch = self.epoch.wrapping_add(1);
            self.last_point = None;
        }
    }

    /// Advance one tick.
    ///
    /// - `available`: the platform can report a live cursor position and hit-test.
    /// - `held`: the modifiers currently held.
    /// - `position`: the cursor position this tick, or `None` when unavailable.
    pub fn on_tick(&mut self, available: bool, held: Modifiers, position: Option<Point>) -> PickerDecision {
        // Not armed or unavailable → picking cannot happen; stop if it was active.
        if !self.armed || !available {
            self.stop();
            return PickerDecision::Idle;
        }

        // The exact configured combination must be held (and only that set).
        if held != self.combo {
            self.stop();
            return PickerDecision::Idle;
        }

        // Combination held: picking is active. A position we cannot read this
        // tick must never move the selection.
        let Some(point) = position else {
            self.active = true;
            return PickerDecision::Idle;
        };

        // Becoming active (or already active): only resolve when the cursor has
        // moved past the threshold since the last resolve.
        let moved = self.last_point.is_none_or(|last| {
            (last.x() - point.x()).abs() > MOVE_THRESHOLD || (last.y() - point.y()).abs() > MOVE_THRESHOLD
        });
        self.active = true;
        if moved {
            self.last_point = Some(point);
            PickerDecision::Resolve { point, epoch: self.epoch }
        } else {
            PickerDecision::Idle
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELD: Modifiers = Modifiers::CTRL_ALT_SHIFT;
    const NONE: Modifiers = Modifiers { ctrl: false, alt: false, shift: false };
    const PARTIAL: Modifiers = Modifiers { ctrl: true, alt: false, shift: true };

    fn armed() -> PickerState {
        let mut s = PickerState::default();
        s.set_armed(true);
        s
    }

    #[test]
    fn disarmed_never_resolves_even_when_held() {
        let mut s = PickerState::default(); // not armed
        assert_eq!(s.on_tick(true, HELD, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
    }

    #[test]
    fn armed_and_held_resolves_at_point() {
        let mut s = armed();
        let decision = s.on_tick(true, HELD, Some(Point::new(10.0, 10.0)));
        assert!(
            matches!(decision, PickerDecision::Resolve { point, .. } if point == Point::new(10.0, 10.0)),
            "expected a resolve at the cursor, got {decision:?}"
        );
        assert!(s.is_active());
    }

    #[test]
    fn partial_combination_does_not_activate() {
        let mut s = armed();
        assert_eq!(s.on_tick(true, PARTIAL, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
        assert!(!s.is_active());
    }

    #[test]
    fn unavailable_position_is_skipped_without_moving_selection() {
        let mut s = armed();
        assert_eq!(s.on_tick(true, HELD, None), PickerDecision::Idle);
    }

    #[test]
    fn unavailable_platform_is_idle() {
        let mut s = armed();
        assert_eq!(s.on_tick(false, HELD, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
    }

    #[test]
    fn stationary_cursor_is_idempotent() {
        let mut s = armed();
        let p = Some(Point::new(10.0, 10.0));
        assert!(matches!(s.on_tick(true, HELD, p), PickerDecision::Resolve { .. }));
        // Same point again → no re-resolve.
        assert_eq!(s.on_tick(true, HELD, p), PickerDecision::Idle);
        // Moved far enough → resolves again.
        assert!(matches!(s.on_tick(true, HELD, Some(Point::new(40.0, 40.0))), PickerDecision::Resolve { .. }));
    }

    #[test]
    fn release_bumps_epoch_so_stale_results_are_discarded() {
        let mut s = armed();
        let PickerDecision::Resolve { epoch, .. } = s.on_tick(true, HELD, Some(Point::new(10.0, 10.0))) else {
            panic!("expected a resolve while armed and held");
        };
        assert!(s.is_current_epoch(epoch));
        // Release the modifiers: picking stops, epoch advances.
        assert_eq!(s.on_tick(true, NONE, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
        assert!(!s.is_active());
        assert!(!s.is_current_epoch(epoch), "a result tagged with the pre-release epoch must be stale");
    }

    #[test]
    fn changed_combination_activates_and_former_does_not() {
        let mut s = armed();
        let new_combo = Modifiers { ctrl: true, alt: false, shift: true };
        s.set_combo(new_combo);
        // The former (default) combination no longer activates.
        assert_eq!(s.on_tick(true, HELD, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
        assert!(!s.is_active());
        // The newly-configured combination picks.
        assert!(matches!(s.on_tick(true, new_combo, Some(Point::new(10.0, 10.0))), PickerDecision::Resolve { .. }));
    }

    #[test]
    fn partial_of_changed_combination_does_not_activate() {
        let mut s = armed();
        s.set_combo(Modifiers { ctrl: true, alt: true, shift: false });
        let partial = Modifiers { ctrl: true, alt: false, shift: false };
        assert_eq!(s.on_tick(true, partial, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
        assert!(!s.is_active());
    }

    #[test]
    fn empty_combination_is_rejected() {
        let mut s = armed();
        s.set_combo(NONE);
        assert_eq!(s.combo(), Modifiers::CTRL_ALT_SHIFT, "an empty combination must not replace the current one");
        // In particular, "no keys held" must never activate picking.
        assert_eq!(s.on_tick(true, NONE, Some(Point::new(10.0, 10.0))), PickerDecision::Idle);
        assert!(!s.is_active());
    }

    #[test]
    fn changing_combination_mid_pick_stops_and_bumps_epoch() {
        let mut s = armed();
        let PickerDecision::Resolve { epoch, .. } = s.on_tick(true, HELD, Some(Point::new(10.0, 10.0))) else {
            panic!("expected a resolve while armed and held");
        };
        s.set_combo(Modifiers { ctrl: true, alt: true, shift: false });
        assert!(!s.is_active());
        assert!(!s.is_current_epoch(epoch), "a result from before the combination change must be stale");
    }

    #[test]
    fn disarm_stops_and_bumps_epoch() {
        let mut s = armed();
        let PickerDecision::Resolve { epoch, .. } = s.on_tick(true, HELD, Some(Point::new(10.0, 10.0))) else {
            panic!("expected a resolve while armed and held");
        };
        s.set_armed(false);
        assert!(!s.is_active());
        assert!(!s.is_current_epoch(epoch));
    }
}
