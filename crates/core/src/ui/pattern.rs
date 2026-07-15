use super::value::UiValue;
use crate::platform::PlatformError;
use crate::types::{Point, Size};
use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
// std::error::Error is provided by the thiserror derive for PatternError
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex, OnceLock};
use thiserror::Error as ThisError;

use super::identifiers::{PatternName, pattern_names};

/// Base trait for runtime patterns that enrich a [`UiNode`](super::UiNode).
///
/// Provider implementations register pattern instances in the [`PatternRegistry`]
/// so `supported_patterns()` and `UiNode::pattern::<T>()` operate on the same data.
pub trait UiPattern: Any + Send + Sync {
    fn pattern_name(&self) -> PatternName;

    fn static_pattern_name() -> PatternName
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
        let raw = Arc::into_raw(pattern) as *const T;
        // SAFETY: `is::<T>()` verified that the concrete type behind `dyn UiPattern`
        // is `T`. `Arc::into_raw` returns a pointer whose data component points to the
        // actual `T` value, and casting to `*const T` (thin pointer) discards the
        // trait-object vtable. The `Arc` allocation layout is identical regardless of
        // whether it was created as `Arc<T>` or `Arc<dyn UiPattern>` (same data, same
        // refcounts), so `Arc::from_raw` reconstructs a valid `Arc<T>`.
        #[allow(unsafe_code)]
        Some(unsafe { Arc::from_raw(raw) })
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
    order: Vec<PatternName>,
    entries: HashMap<PatternName, RegistryEntry>,
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
        let mut state = self.state.lock().expect("PatternRegistry lock poisoned");
        let id = pattern.pattern_name();
        if let Some(entry) = state.entries.get_mut(&id) {
            *entry = RegistryEntry::Ready(Arc::clone(&pattern));
        } else {
            state.order.push(id.clone());
            state.entries.insert(id, RegistryEntry::Ready(pattern));
        }
    }

    pub fn register_lazy<F>(&self, id: PatternName, probe: F)
    where
        F: Fn() -> Option<Arc<dyn UiPattern>> + Send + Sync + 'static,
    {
        let mut state = self.state.lock().expect("PatternRegistry lock poisoned");
        let probe_arc: Arc<dyn Fn() -> Option<Arc<dyn UiPattern>> + Send + Sync> = Arc::new(probe);
        if let Some(entry) = state.entries.get_mut(&id) {
            *entry = RegistryEntry::Lazy { probe: Arc::clone(&probe_arc), cached: OnceLock::new() };
        } else {
            state.order.push(id.clone());
            state.entries.insert(id, RegistryEntry::Lazy { probe: probe_arc, cached: OnceLock::new() });
        }
    }

    pub fn get(&self, id: &PatternName) -> Option<Arc<dyn UiPattern>> {
        let mut state = self.state.lock().expect("PatternRegistry lock poisoned");
        let entry = state.entries.get_mut(id)?;
        resolve_entry(entry)
    }

    pub fn get_typed<T>(&self) -> Option<Arc<T>>
    where
        T: UiPattern + 'static,
    {
        let id = T::static_pattern_name();
        self.get(&id).and_then(downcast_pattern_arc::<T>)
    }

    pub fn supported(&self) -> Vec<PatternName> {
        let mut state = self.state.lock().expect("PatternRegistry lock poisoned");
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
        let state = self.state.lock().expect("PatternRegistry lock poisoned");
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
pub fn supported_patterns_value(patterns: &[PatternName]) -> UiValue {
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
    fn pattern_name(&self) -> PatternName {
        Self::static_pattern_name()
    }

    fn static_pattern_name() -> PatternName
    where
        Self: Sized,
    {
        PatternName::from(pattern_names::FOCUSABLE)
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

/// Macro to declare a simple pure-action pattern (closure -> Result<(), PatternError>)
/// together with its `*Action` builder struct.
macro_rules! declare_action_pattern {
    ($trait_name:ident, $action_struct:ident, $method:ident, $pattern_const:ident) => {
        pub trait $trait_name: UiPattern {
            fn $method(&self) -> Result<(), PatternError>;
        }

        #[must_use]
        pub struct $action_struct {
            handler: ActionHandler,
        }

        impl $action_struct {
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

        impl Default for $action_struct {
            fn default() -> Self {
                Self::noop()
            }
        }

        impl UiPattern for $action_struct {
            fn pattern_name(&self) -> PatternName {
                Self::static_pattern_name()
            }

            fn static_pattern_name() -> PatternName
            where
                Self: Sized,
            {
                PatternName::from(pattern_names::$pattern_const)
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        impl $trait_name for $action_struct {
            fn $method(&self) -> Result<(), PatternError> {
                (self.handler)()
            }
        }
    };
}

declare_action_pattern!(ActivatablePattern, ActivatableAction, activate, ACTIVATABLE);
declare_action_pattern!(MinimizablePattern, MinimizableAction, minimize, MINIMIZABLE);
declare_action_pattern!(MaximizablePattern, MaximizableAction, maximize, MAXIMIZABLE);
declare_action_pattern!(RestorablePattern, RestorableAction, restore, RESTORABLE);
declare_action_pattern!(CloseablePattern, CloseableAction, close, CLOSEABLE);

/// Pattern for window movement \u2014 places the surface at a screen point.
pub trait MovablePattern: UiPattern {
    fn move_to(&self, position: Point) -> Result<(), PatternError>;
}

#[must_use]
pub struct MovableAction {
    handler: MoveHandler,
}

impl MovableAction {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(Point) -> Result<(), PatternError> + Send + Sync + 'static,
    {
        Self { handler: arc_move(handler) }
    }

    pub fn noop() -> Self {
        Self::new(|_| Ok(()))
    }
}

impl Default for MovableAction {
    fn default() -> Self {
        Self::noop()
    }
}

impl UiPattern for MovableAction {
    fn pattern_name(&self) -> PatternName {
        Self::static_pattern_name()
    }

    fn static_pattern_name() -> PatternName
    where
        Self: Sized,
    {
        PatternName::from(pattern_names::MOVABLE)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MovablePattern for MovableAction {
    fn move_to(&self, position: Point) -> Result<(), PatternError> {
        (self.handler)(position)
    }
}

/// Pattern for window resizing \u2014 changes the surface size.
pub trait ResizablePattern: UiPattern {
    fn resize(&self, size: Size) -> Result<(), PatternError>;
}

#[must_use]
pub struct ResizableAction {
    handler: ResizeHandler,
}

impl ResizableAction {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(Size) -> Result<(), PatternError> + Send + Sync + 'static,
    {
        Self { handler: arc_resize(handler) }
    }

    pub fn noop() -> Self {
        Self::new(|_| Ok(()))
    }
}

impl Default for ResizableAction {
    fn default() -> Self {
        Self::noop()
    }
}

impl UiPattern for ResizableAction {
    fn pattern_name(&self) -> PatternName {
        Self::static_pattern_name()
    }

    fn static_pattern_name() -> PatternName
    where
        Self: Sized,
    {
        PatternName::from(pattern_names::RESIZABLE)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ResizablePattern for ResizableAction {
    fn resize(&self, size: Size) -> Result<(), PatternError> {
        (self.handler)(size)
    }
}

/// Pattern for programmatic text replacement — writes the full text content
/// of an editable text element through the backing accessibility API (as
/// opposed to synthesizing keystrokes).
pub trait TextEditablePattern: UiPattern {
    fn set_text(&self, text: &str) -> Result<(), PatternError>;
}

type TextHandler = Arc<dyn Fn(&str) -> Result<(), PatternError> + Send + Sync>;

#[must_use]
pub struct TextEditableAction {
    handler: TextHandler,
}

impl TextEditableAction {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&str) -> Result<(), PatternError> + Send + Sync + 'static,
    {
        Self { handler: Arc::new(handler) }
    }

    pub fn noop() -> Self {
        Self::new(|_| Ok(()))
    }
}

impl Default for TextEditableAction {
    fn default() -> Self {
        Self::noop()
    }
}

impl UiPattern for TextEditableAction {
    fn pattern_name(&self) -> PatternName {
        Self::static_pattern_name()
    }

    fn static_pattern_name() -> PatternName
    where
        Self: Sized,
    {
        PatternName::from(pattern_names::TEXT_EDITABLE)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TextEditablePattern for TextEditableAction {
    fn set_text(&self, text: &str) -> Result<(), PatternError> {
        (self.handler)(text)
    }
}

/// Pattern that polls whether a surface currently accepts user input.
pub trait ResponsivePattern: UiPattern {
    fn accepts_user_input(&self) -> Result<Option<bool>, PatternError>;
}

#[must_use]
pub struct ResponsiveAction {
    handler: InputHandler,
}

impl ResponsiveAction {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn() -> Result<Option<bool>, PatternError> + Send + Sync + 'static,
    {
        Self { handler: arc_input(handler) }
    }

    pub fn unknown() -> Self {
        Self::new(|| Ok(None))
    }
}

impl Default for ResponsiveAction {
    fn default() -> Self {
        Self::unknown()
    }
}

impl UiPattern for ResponsiveAction {
    fn pattern_name(&self) -> PatternName {
        Self::static_pattern_name()
    }

    fn static_pattern_name() -> PatternName
    where
        Self: Sized,
    {
        PatternName::from(pattern_names::RESPONSIVE)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ResponsivePattern for ResponsiveAction {
    fn accepts_user_input(&self) -> Result<Option<bool>, PatternError> {
        (self.handler)()
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

impl From<PlatformError> for PatternError {
    fn from(err: PlatformError) -> Self {
        Self::new(err.to_string())
    }
}

/// Pattern for focus changes – requests focus via the runtime.
pub trait FocusablePattern: UiPattern {
    fn focus(&self) -> Result<(), PatternError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::sync::{Arc, Mutex};

    struct DummyPattern;

    impl UiPattern for DummyPattern {
        fn pattern_name(&self) -> PatternName {
            Self::static_pattern_name()
        }

        fn static_pattern_name() -> PatternName
        where
            Self: Sized,
        {
            PatternName::from(pattern_names::DUMMY)
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
        assert_eq!(supported[0], DummyPattern::static_pattern_name());
    }

    #[rstest]
    fn register_lazy_resolves_on_demand() {
        struct LazyPattern;

        impl UiPattern for LazyPattern {
            fn pattern_name(&self) -> PatternName {
                Self::static_pattern_name()
            }

            fn static_pattern_name() -> PatternName
            where
                Self: Sized,
            {
                PatternName::from(pattern_names::LAZY)
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        let registry = PatternRegistry::new();
        let counter = Arc::new(Mutex::new(0u32));
        let counter_clone = Arc::clone(&counter);
        registry.register_lazy(LazyPattern::static_pattern_name(), move || {
            *counter_clone.lock().unwrap() += 1;
            Some(Arc::new(LazyPattern) as Arc<dyn UiPattern>)
        });

        // First access resolves the pattern via the probe.
        let ids = registry.supported();
        assert_eq!(ids, vec![LazyPattern::static_pattern_name()]);
        assert_eq!(*counter.lock().unwrap(), 1);

        // Subsequent lookups reuse the cached pattern without invoking the probe again.
        assert!(registry.get(&LazyPattern::static_pattern_name()).is_some());
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[rstest]
    fn downcast_returns_none_for_mismatched_type() {
        let arc: Arc<dyn UiPattern> = Arc::new(DummyPattern);
        assert!(downcast_pattern_arc::<DummyPattern>(Arc::clone(&arc)).is_some());

        struct OtherPattern;
        impl UiPattern for OtherPattern {
            fn pattern_name(&self) -> PatternName {
                Self::static_pattern_name()
            }

            fn static_pattern_name() -> PatternName
            where
                Self: Sized,
            {
                PatternName::from(pattern_names::OTHER)
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
    fn movable_action_invokes_handler() {
        let moves: Arc<Mutex<Vec<Point>>> = Arc::new(Mutex::new(Vec::new()));
        let action = MovableAction::new({
            let moves = Arc::clone(&moves);
            move |point| {
                moves.lock().unwrap().push(point);
                Ok(())
            }
        });

        action.move_to(Point::new(10.0, 20.0)).expect("move should succeed");
        assert_eq!(moves.lock().unwrap().as_slice(), &[Point::new(10.0, 20.0)]);
    }

    #[rstest]
    fn resizable_action_invokes_handler() {
        let sizes: Arc<Mutex<Vec<Size>>> = Arc::new(Mutex::new(Vec::new()));
        let action = ResizableAction::new({
            let sizes = Arc::clone(&sizes);
            move |size| {
                sizes.lock().unwrap().push(size);
                Ok(())
            }
        });

        action.resize(Size::new(300.0, 200.0)).expect("resize should succeed");
        assert_eq!(sizes.lock().unwrap().as_slice(), &[Size::new(300.0, 200.0)]);
    }

    #[rstest]
    fn activatable_action_propagates_error() {
        let action = ActivatableAction::new(|| Err(PatternError::new("fail")));
        let err = action.activate().expect_err("should propagate");
        assert_eq!(err.message(), "fail");
    }

    #[rstest]
    fn responsive_action_reports_value() {
        let action = ResponsiveAction::new(|| Ok(Some(true)));
        assert_eq!(action.accepts_user_input().unwrap(), Some(true));
    }

    #[rstest]
    fn text_editable_action_invokes_handler_with_text() {
        let written: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let action = TextEditableAction::new({
            let written = Arc::clone(&written);
            move |text| {
                written.lock().unwrap().push(text.to_string());
                Ok(())
            }
        });

        action.set_text("hello").expect("set_text should succeed");
        assert_eq!(written.lock().unwrap().as_slice(), &["hello".to_string()]);
        assert_eq!(action.pattern_name(), TextEditableAction::static_pattern_name());
    }

    #[rstest]
    fn text_editable_action_propagates_error() {
        let action = TextEditableAction::new(|_| Err(PatternError::new("read-only")));
        let err = action.set_text("x").expect_err("should bubble up");
        assert_eq!(err.message(), "read-only");
    }

    #[rstest]
    fn responsive_action_propagates_error() {
        let action = ResponsiveAction::new(|| Err(PatternError::new("io")));
        let err = action.accepts_user_input().expect_err("should bubble up");
        assert_eq!(err.message(), "io");
    }

    #[rstest]
    fn supported_patterns_value_converts_ids() {
        let patterns = vec![PatternName::from(pattern_names::FOCUSABLE), PatternName::from(pattern_names::ACTIVATABLE)];
        let value = supported_patterns_value(&patterns);
        assert_eq!(
            value,
            UiValue::Array(vec![UiValue::from(pattern_names::FOCUSABLE), UiValue::from(pattern_names::ACTIVATABLE),])
        );
    }
}
