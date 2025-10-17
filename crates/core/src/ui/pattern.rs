use super::value::UiValue;
use crate::types::{Point, Rect, Size};
use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
// std::error::Error is provided by the thiserror derive for PatternError
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex, OnceLock};
use thiserror::Error as ThisError;

use super::identifiers::PatternId;

/// Base trait for runtime patterns that enrich a [`UiNode`](super::UiNode).
///
/// Provider implementations register pattern instances in the [`PatternRegistry`]
/// so `supported_patterns()` and `UiNode::pattern::<T>()` operate on the same data.
pub trait UiPattern: Any + Send + Sync {
    fn id(&self) -> PatternId;

    fn static_id() -> PatternId
    where
        Self: Sized;

    fn as_any(&self) -> &dyn Any;
}

#[inline]
pub fn downcast_pattern_arc<T>(pattern: Arc<dyn UiPattern>) -> Option<Arc<T>>
where
    T: UiPattern + 'static,
{
    if Arc::as_ref(&pattern).as_any().is::<T>() {
        let raw_pattern = Arc::into_raw(pattern);
        let raw_any = raw_pattern as *const (dyn Any + Send + Sync);
        let any_arc = unsafe { Arc::from_raw(raw_any) };
        Arc::downcast::<T>(any_arc).ok()
    } else {
        None
    }
}

#[inline]
pub fn downcast_pattern_ref<T>(pattern: &Arc<dyn UiPattern>) -> Option<Arc<T>>
where
    T: UiPattern + 'static,
{
    downcast_pattern_arc::<T>(Arc::clone(pattern))
}

#[derive(Default)]
pub struct PatternRegistry {
    state: Mutex<PatternRegistryState>,
}

#[derive(Default)]
struct PatternRegistryState {
    order: Vec<PatternId>,
    entries: HashMap<PatternId, RegistryEntry>,
}

enum RegistryEntry {
    Ready(Arc<dyn UiPattern>),
    Lazy {
        probe: Arc<dyn Fn() -> Option<Arc<dyn UiPattern>> + Send + Sync>,
        cached: OnceLock<Option<Arc<dyn UiPattern>>>,
    },
}

impl PatternRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P>(&self, pattern: Arc<P>)
    where
        P: UiPattern + 'static,
    {
        self.register_dyn(pattern as Arc<dyn UiPattern>);
    }

    pub fn register_dyn(&self, pattern: Arc<dyn UiPattern>) {
        let mut state = self.state.lock().unwrap();
        let id = pattern.id();
        if let Some(entry) = state.entries.get_mut(&id) {
            *entry = RegistryEntry::Ready(Arc::clone(&pattern));
        } else {
            state.order.push(id.clone());
            state.entries.insert(id, RegistryEntry::Ready(pattern));
        }
    }

    pub fn register_lazy<F>(&self, id: PatternId, probe: F)
    where
        F: Fn() -> Option<Arc<dyn UiPattern>> + Send + Sync + 'static,
    {
        let mut state = self.state.lock().unwrap();
        let probe_arc: Arc<dyn Fn() -> Option<Arc<dyn UiPattern>> + Send + Sync> = Arc::new(probe);
        if let Some(entry) = state.entries.get_mut(&id) {
            *entry = RegistryEntry::Lazy { probe: Arc::clone(&probe_arc), cached: OnceLock::new() };
        } else {
            state.order.push(id.clone());
            state.entries.insert(id, RegistryEntry::Lazy { probe: probe_arc, cached: OnceLock::new() });
        }
    }

    pub fn get(&self, id: &PatternId) -> Option<Arc<dyn UiPattern>> {
        let mut state = self.state.lock().unwrap();
        let entry = state.entries.get_mut(id)?;
        resolve_entry(entry)
    }

    pub fn get_typed<T>(&self) -> Option<Arc<T>>
    where
        T: UiPattern + 'static,
    {
        let id = T::static_id();
        self.get(&id).and_then(downcast_pattern_arc::<T>)
    }

    pub fn supported(&self) -> Vec<PatternId> {
        let mut state = self.state.lock().unwrap();
        let order_snapshot = state.order.clone();
        let mut supported = Vec::new();
        for id in order_snapshot {
            if let Some(entry) = state.entries.get_mut(&id) {
                if resolve_entry(entry).is_none() {
                    continue;
                }
                supported.push(id);
            }
        }
        supported
    }

    pub fn is_empty(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.entries.is_empty()
    }
}

fn resolve_entry(entry: &mut RegistryEntry) -> Option<Arc<dyn UiPattern>> {
    match entry {
        RegistryEntry::Ready(pattern) => Some(Arc::clone(pattern)),
        RegistryEntry::Lazy { probe, cached } => {
            let value = cached.get_or_init(|| probe.as_ref()());
            match value {
                Some(pattern) => {
                    let cloned = Arc::clone(pattern);
                    *entry = RegistryEntry::Ready(Arc::clone(pattern));
                    Some(cloned)
                }
                None => None,
            }
        }
    }
}

/// Converts a pattern list into the canonical `SupportedPatterns` value.
pub fn supported_patterns_value(patterns: &[PatternId]) -> UiValue {
    UiValue::Array(patterns.iter().map(|id| UiValue::from(id.as_str().to_owned())).collect())
}

type ActionHandler = Arc<dyn Fn() -> Result<(), PatternError> + Send + Sync>;
type MoveHandler = Arc<dyn Fn(Point) -> Result<(), PatternError> + Send + Sync>;
type ResizeHandler = Arc<dyn Fn(Size) -> Result<(), PatternError> + Send + Sync>;
type InputHandler = Arc<dyn Fn() -> Result<Option<bool>, PatternError> + Send + Sync>;

fn arc_action<F>(handler: F) -> ActionHandler
where
    F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
{
    Arc::new(handler)
}

fn arc_move<F>(handler: F) -> MoveHandler
where
    F: Fn(Point) -> Result<(), PatternError> + Send + Sync + 'static,
{
    Arc::new(handler)
}

fn arc_resize<F>(handler: F) -> ResizeHandler
where
    F: Fn(Size) -> Result<(), PatternError> + Send + Sync + 'static,
{
    Arc::new(handler)
}

fn arc_input<F>(handler: F) -> InputHandler
where
    F: Fn() -> Result<Option<bool>, PatternError> + Send + Sync + 'static,
{
    Arc::new(handler)
}

/// Simple focus implementation backed by a closure.
pub struct FocusableAction {
    handler: ActionHandler,
}

impl FocusableAction {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
    {
        Self { handler: arc_action(handler) }
    }

    pub fn noop() -> Self {
        Self::new(|| Ok(()))
    }
}

impl UiPattern for FocusableAction {
    fn id(&self) -> PatternId {
        Self::static_id()
    }

    fn static_id() -> PatternId
    where
        Self: Sized,
    {
        PatternId::from("Focusable")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FocusablePattern for FocusableAction {
    fn focus(&self) -> Result<(), PatternError> {
        (self.handler)()
    }
}

/// Configurable window-surface implementation used in tests and default runtime wiring.
pub struct WindowSurfaceActions {
    activate: ActionHandler,
    minimize: ActionHandler,
    maximize: ActionHandler,
    restore: ActionHandler,
    close: ActionHandler,
    move_to: MoveHandler,
    resize: ResizeHandler,
    accepts_user_input: InputHandler,
}

impl WindowSurfaceActions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_activate<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.activate = arc_action(handler);
        self
    }

    pub fn with_minimize<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.minimize = arc_action(handler);
        self
    }

    pub fn with_maximize<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.maximize = arc_action(handler);
        self
    }

    pub fn with_restore<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.restore = arc_action(handler);
        self
    }

    pub fn with_close<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.close = arc_action(handler);
        self
    }

    pub fn with_move_to<F>(mut self, handler: F) -> Self
    where
        F: Fn(Point) -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.move_to = arc_move(handler);
        self
    }

    pub fn with_resize<F>(mut self, handler: F) -> Self
    where
        F: Fn(Size) -> Result<(), PatternError> + Send + Sync + 'static,
    {
        self.resize = arc_resize(handler);
        self
    }

    pub fn with_accepts_user_input<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> Result<Option<bool>, PatternError> + Send + Sync + 'static,
    {
        self.accepts_user_input = arc_input(handler);
        self
    }
}

impl Default for WindowSurfaceActions {
    fn default() -> Self {
        Self {
            activate: arc_action(|| Ok(())),
            minimize: arc_action(|| Ok(())),
            maximize: arc_action(|| Ok(())),
            restore: arc_action(|| Ok(())),
            close: arc_action(|| Ok(())),
            move_to: arc_move(|_| Ok(())),
            resize: arc_resize(|_| Ok(())),
            accepts_user_input: arc_input(|| Ok(None)),
        }
    }
}

impl UiPattern for WindowSurfaceActions {
    fn id(&self) -> PatternId {
        Self::static_id()
    }

    fn static_id() -> PatternId
    where
        Self: Sized,
    {
        PatternId::from("WindowSurface")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl WindowSurfacePattern for WindowSurfaceActions {
    fn activate(&self) -> Result<(), PatternError> {
        (self.activate)()
    }

    fn minimize(&self) -> Result<(), PatternError> {
        (self.minimize)()
    }

    fn maximize(&self) -> Result<(), PatternError> {
        (self.maximize)()
    }

    fn restore(&self) -> Result<(), PatternError> {
        (self.restore)()
    }

    fn close(&self) -> Result<(), PatternError> {
        (self.close)()
    }

    fn move_to(&self, position: Point) -> Result<(), PatternError> {
        (self.move_to)(position)
    }

    fn resize(&self, size: Size) -> Result<(), PatternError> {
        (self.resize)(size)
    }

    fn accepts_user_input(&self) -> Result<Option<bool>, PatternError> {
        (self.accepts_user_input)()
    }
}

/// Error object for runtime actions triggered from a pattern implementation.
#[derive(Debug, Clone, ThisError)]
pub struct PatternError {
    message: Cow<'static, str>,
}

impl PatternError {
    pub fn new<M: Into<Cow<'static, str>>>(message: M) -> Self {
        Self { message: message.into() }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for PatternError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Pattern for focus changes – requests focus via the runtime.
pub trait FocusablePattern: UiPattern {
    fn focus(&self) -> Result<(), PatternError>;
}

/// Pattern for window control via platform‑specific window APIs.
pub trait WindowSurfacePattern: UiPattern {
    fn activate(&self) -> Result<(), PatternError>;
    fn minimize(&self) -> Result<(), PatternError>;
    fn maximize(&self) -> Result<(), PatternError>;
    fn restore(&self) -> Result<(), PatternError>;
    fn close(&self) -> Result<(), PatternError>;

    fn move_to(&self, position: Point) -> Result<(), PatternError>;
    fn resize(&self, size: Size) -> Result<(), PatternError>;

    fn accepts_user_input(&self) -> Result<Option<bool>, PatternError>;

    fn move_and_resize(&self, bounds: Rect) -> Result<(), PatternError> {
        self.move_to(bounds.position())?;
        self.resize(bounds.size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::sync::{Arc, Mutex};

    struct DummyPattern;

    impl UiPattern for DummyPattern {
        fn id(&self) -> PatternId {
            Self::static_id()
        }

        fn static_id() -> PatternId
        where
            Self: Sized,
        {
            PatternId::from("Dummy")
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[rstest]
    fn registry_registers_and_retrieves_typed_pattern() {
        let registry = PatternRegistry::new();
        registry.register(Arc::new(DummyPattern));

        let stored = registry.get_typed::<DummyPattern>();
        assert!(stored.is_some());
        let supported = registry.supported();
        assert_eq!(supported.len(), 1);
        assert_eq!(supported[0], DummyPattern::static_id());
    }

    #[rstest]
    fn register_lazy_resolves_on_demand() {
        struct LazyPattern;

        impl UiPattern for LazyPattern {
            fn id(&self) -> PatternId {
                Self::static_id()
            }

            fn static_id() -> PatternId
            where
                Self: Sized,
            {
                PatternId::from("Lazy")
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        let registry = PatternRegistry::new();
        let counter = Arc::new(Mutex::new(0u32));
        let counter_clone = Arc::clone(&counter);
        registry.register_lazy(LazyPattern::static_id(), move || {
            *counter_clone.lock().unwrap() += 1;
            Some(Arc::new(LazyPattern) as Arc<dyn UiPattern>)
        });

        // First access resolves the pattern via the probe.
        let ids = registry.supported();
        assert_eq!(ids, vec![LazyPattern::static_id()]);
        assert_eq!(*counter.lock().unwrap(), 1);

        // Subsequent lookups reuse the cached pattern without invoking the probe again.
        assert!(registry.get(&LazyPattern::static_id()).is_some());
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[rstest]
    fn downcast_returns_none_for_mismatched_type() {
        let arc: Arc<dyn UiPattern> = Arc::new(DummyPattern);
        assert!(downcast_pattern_arc::<DummyPattern>(Arc::clone(&arc)).is_some());

        struct OtherPattern;
        impl UiPattern for OtherPattern {
            fn id(&self) -> PatternId {
                Self::static_id()
            }

            fn static_id() -> PatternId
            where
                Self: Sized,
            {
                PatternId::from("Other")
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        assert!(downcast_pattern_arc::<OtherPattern>(arc).is_none());
    }

    #[rstest]
    #[case("error message")]
    #[case("technical detail")]
    fn pattern_error_exposes_message(#[case] message: &str) {
        let err = PatternError::new(message.to_string());
        assert_eq!(err.message(), message);
        assert_eq!(format!("{}", err), message);
    }

    #[rstest]
    fn focusable_action_invokes_handler() {
        let calls = Arc::new(Mutex::new(0));
        let action = {
            let calls = Arc::clone(&calls);
            FocusableAction::new(move || {
                *calls.lock().unwrap() += 1;
                Ok(())
            })
        };

        action.focus().expect("focus should succeed");
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[rstest]
    fn focusable_action_propagates_error() {
        let action = FocusableAction::new(|| Err(PatternError::new("fail")));
        let err = action.focus().expect_err("should bubble up error");
        assert_eq!(err.message(), "fail");
    }

    #[rstest]
    fn window_surface_actions_execute_handlers() {
        let moves: Arc<Mutex<Vec<Point>>> = Arc::new(Mutex::new(Vec::new()));
        let sizes: Arc<Mutex<Vec<Size>>> = Arc::new(Mutex::new(Vec::new()));

        let actions = WindowSurfaceActions::new()
            .with_move_to({
                let moves = Arc::clone(&moves);
                move |point| {
                    moves.lock().unwrap().push(point);
                    Ok(())
                }
            })
            .with_resize({
                let sizes = Arc::clone(&sizes);
                move |size| {
                    sizes.lock().unwrap().push(size);
                    Ok(())
                }
            });

        actions.move_and_resize(Rect::new(10.0, 20.0, 300.0, 200.0)).expect("default implementation should succeed");

        assert_eq!(moves.lock().unwrap().as_slice(), &[Point::new(10.0, 20.0)]);
        assert_eq!(sizes.lock().unwrap().as_slice(), &[Size::new(300.0, 200.0)]);
    }

    #[rstest]
    fn window_surface_actions_propagate_error() {
        let actions = WindowSurfaceActions::new().with_activate(|| Err(PatternError::new("fail")));
        let err = actions.activate().expect_err("should propagate");
        assert_eq!(err.message(), "fail");
    }

    #[rstest]
    fn window_surface_accepts_user_input_reports_value() {
        let actions = WindowSurfaceActions::new().with_accepts_user_input(|| Ok(Some(true)));
        assert_eq!(actions.accepts_user_input().unwrap(), Some(true));
    }

    #[rstest]
    fn window_surface_accepts_user_input_propagates_error() {
        let actions = WindowSurfaceActions::new().with_accepts_user_input(|| Err(PatternError::new("io")));
        let err = actions.accepts_user_input().expect_err("should bubble up");
        assert_eq!(err.message(), "io");
    }

    #[rstest]
    fn supported_patterns_value_converts_ids() {
        let patterns = vec![PatternId::from("Focusable"), PatternId::from("WindowSurface")];
        let value = supported_patterns_value(&patterns);
        assert_eq!(value, UiValue::Array(vec![UiValue::from("Focusable"), UiValue::from("WindowSurface")]));
    }
}
