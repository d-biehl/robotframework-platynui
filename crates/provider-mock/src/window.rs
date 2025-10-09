use crate::events;
use crate::focus;
use crate::tree::AttributeSpec;
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{element, window_surface};
use platynui_core::ui::{
    Namespace, PatternError, PatternRegistry, RuntimeId, UiAttribute, UiPattern, UiValue, WindowSurfaceActions,
};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

#[derive(Clone, Debug)]
pub(crate) struct WindowConfig {
    pub bounds: Rect,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub is_topmost: bool,
    pub supports_move: bool,
    pub supports_resize: bool,
    pub accepts_user_input: Option<bool>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            is_minimized: false,
            is_maximized: false,
            is_topmost: false,
            supports_move: true,
            supports_resize: true,
            accepts_user_input: None,
        }
    }
}

#[derive(Clone, Debug)]
struct WindowState {
    bounds: Rect,
    is_minimized: bool,
    is_maximized: bool,
    is_topmost: bool,
    supports_move: bool,
    supports_resize: bool,
    accepts_user_input: Option<bool>,
}

impl From<WindowConfig> for WindowState {
    fn from(config: WindowConfig) -> Self {
        Self {
            bounds: config.bounds,
            is_minimized: config.is_minimized,
            is_maximized: config.is_maximized,
            is_topmost: config.is_topmost,
            supports_move: config.supports_move,
            supports_resize: config.supports_resize,
            accepts_user_input: config.accepts_user_input,
        }
    }
}

impl WindowState {
    fn accepts_user_input(&self) -> Option<bool> {
        if let Some(value) = self.accepts_user_input { Some(value) } else { Some(!self.is_minimized) }
    }
}

static WINDOW_STATES: LazyLock<RwLock<HashMap<RuntimeId, WindowState>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

pub(crate) fn reset() {
    WINDOW_STATES.write().unwrap().clear();
}

pub(crate) fn derive_config(attributes: &[AttributeSpec]) -> WindowConfig {
    let mut config = WindowConfig::default();
    for attr in attributes {
        if attr.name() == element::BOUNDS {
            if let UiValue::Rect(rect) = attr.value().clone() {
                config.bounds = rect;
            }
            continue;
        }
        match attr.name() {
            window_surface::IS_MINIMIZED => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_minimized = value;
                }
            }
            window_surface::IS_MAXIMIZED => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_maximized = value;
                }
            }
            window_surface::IS_TOPMOST => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_topmost = value;
                }
            }
            window_surface::SUPPORTS_MOVE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.supports_move = value;
                }
            }
            window_surface::SUPPORTS_RESIZE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.supports_resize = value;
                }
            }
            window_surface::ACCEPTS_USER_INPUT => {
                config.accepts_user_input = as_bool(attr.value());
            }
            _ => {}
        }
    }
    config
}

pub(crate) fn should_filter_attribute(name: &str) -> bool {
    matches!(
        name,
        n if n == element::BOUNDS
            || n == window_surface::IS_MINIMIZED
            || n == window_surface::IS_MAXIMIZED
            || n == window_surface::IS_TOPMOST
            || n == window_surface::SUPPORTS_MOVE
            || n == window_surface::SUPPORTS_RESIZE
            || n == window_surface::ACCEPTS_USER_INPUT
    )
}

pub(crate) fn register_window(
    runtime_id: RuntimeId,
    namespace: Namespace,
    config: WindowConfig,
    registry: &PatternRegistry,
) -> Vec<Arc<dyn UiAttribute>> {
    WINDOW_STATES.write().unwrap().insert(runtime_id.clone(), WindowState::from(config));

    let register_id = runtime_id.clone();
    registry.register_lazy(WindowSurfaceActions::static_id(), move || {
        state_exists(&register_id).then(|| pattern_for(&register_id))
    });

    vec![
        window_attribute(namespace, runtime_id.clone(), element::BOUNDS, WindowAttributeKind::Bounds),
        window_attribute(namespace, runtime_id.clone(), window_surface::IS_MINIMIZED, WindowAttributeKind::IsMinimized),
        window_attribute(namespace, runtime_id.clone(), window_surface::IS_MAXIMIZED, WindowAttributeKind::IsMaximized),
        window_attribute(namespace, runtime_id.clone(), window_surface::IS_TOPMOST, WindowAttributeKind::IsTopmost),
        window_attribute(
            namespace,
            runtime_id.clone(),
            window_surface::SUPPORTS_MOVE,
            WindowAttributeKind::SupportsMove,
        ),
        window_attribute(
            namespace,
            runtime_id.clone(),
            window_surface::SUPPORTS_RESIZE,
            WindowAttributeKind::SupportsResize,
        ),
        window_attribute(
            namespace,
            runtime_id,
            window_surface::ACCEPTS_USER_INPUT,
            WindowAttributeKind::AcceptsUserInput,
        ),
    ]
}

fn pattern_for(runtime_id: &RuntimeId) -> Arc<dyn UiPattern> {
    let activate_id = runtime_id.clone();
    let minimize_id = runtime_id.clone();
    let maximize_id = runtime_id.clone();
    let restore_id = runtime_id.clone();
    let close_id = runtime_id.clone();
    let move_id = runtime_id.clone();
    let resize_id = runtime_id.clone();
    let input_id = runtime_id.clone();

    Arc::new(
        WindowSurfaceActions::new()
            .with_activate(move || activate(&activate_id))
            .with_minimize(move || minimize(&minimize_id))
            .with_maximize(move || maximize(&maximize_id))
            .with_restore(move || restore(&restore_id))
            .with_close(move || close(&close_id))
            .with_move_to(move |point| move_to(&move_id, point))
            .with_resize(move |size| resize(&resize_id, size))
            .with_accepts_user_input(move || accepts_user_input(&input_id)),
    )
}

fn as_bool(value: &UiValue) -> Option<bool> {
    match value {
        UiValue::Bool(v) => Some(*v),
        UiValue::Integer(v) => Some(*v != 0),
        UiValue::Number(v) => Some(*v != 0.0),
        _ => None,
    }
}

fn state_exists(runtime_id: &RuntimeId) -> bool {
    WINDOW_STATES.read().unwrap().contains_key(runtime_id)
}

fn read_state(runtime_id: &RuntimeId) -> Option<WindowState> {
    WINDOW_STATES.read().unwrap().get(runtime_id).cloned()
}

fn mutate_state<F>(runtime_id: &RuntimeId, mutator: F) -> Result<(), PatternError>
where
    F: FnOnce(&mut WindowState) -> Result<(), PatternError>,
{
    let mut guard = WINDOW_STATES.write().unwrap();
    let state = guard.get_mut(runtime_id).ok_or_else(|| PatternError::new("window is no longer available"))?;
    mutator(state)?;
    drop(guard);
    events::emit_node_updated(runtime_id.as_str());
    Ok(())
}

fn activate(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        state.is_minimized = false;
        state.is_topmost = true;
        Ok(())
    })?;
    focus::request_focus(runtime_id.clone())
}

fn minimize(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        state.is_minimized = true;
        state.is_maximized = false;
        Ok(())
    })?;
    focus::clear_if_matches(runtime_id);
    Ok(())
}

fn maximize(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        state.is_maximized = true;
        state.is_minimized = false;
        Ok(())
    })
}

fn restore(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        state.is_maximized = false;
        state.is_minimized = false;
        Ok(())
    })?;
    focus::request_focus(runtime_id.clone())
}

fn close(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |_| Ok(()))?;
    focus::clear_if_matches(runtime_id);
    Ok(())
}

fn move_to(runtime_id: &RuntimeId, position: Point) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        if !state.supports_move {
            return Err(PatternError::new("window does not support move"));
        }
        let size = state.bounds.size();
        state.bounds = Rect::new(position.x(), position.y(), size.width(), size.height());
        Ok(())
    })
}

fn resize(runtime_id: &RuntimeId, size: Size) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        if !state.supports_resize {
            return Err(PatternError::new("window does not support resize"));
        }
        state.bounds = Rect::new(state.bounds.x(), state.bounds.y(), size.width(), size.height());
        Ok(())
    })
}

fn accepts_user_input(runtime_id: &RuntimeId) -> Result<Option<bool>, PatternError> {
    Ok(read_state(runtime_id).and_then(|state| state.accepts_user_input()).or(Some(false)))
}

fn window_attribute(
    namespace: Namespace,
    runtime_id: RuntimeId,
    name: impl Into<String>,
    kind: WindowAttributeKind,
) -> Arc<dyn UiAttribute> {
    Arc::new(WindowAttribute { namespace, runtime_id, name: name.into(), kind })
}

struct WindowAttribute {
    namespace: Namespace,
    runtime_id: RuntimeId,
    name: String,
    kind: WindowAttributeKind,
}

enum WindowAttributeKind {
    Bounds,
    IsMinimized,
    IsMaximized,
    IsTopmost,
    SupportsMove,
    SupportsResize,
    AcceptsUserInput,
}

impl UiAttribute for WindowAttribute {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn value(&self) -> UiValue {
        let state = read_state(&self.runtime_id);
        match self.kind {
            WindowAttributeKind::Bounds => {
                state.map(|s| UiValue::from(s.bounds)).unwrap_or(UiValue::Rect(Rect::default()))
            }
            WindowAttributeKind::IsMinimized => {
                state.map(|s| UiValue::from(s.is_minimized)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::IsMaximized => {
                state.map(|s| UiValue::from(s.is_maximized)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::IsTopmost => {
                state.map(|s| UiValue::from(s.is_topmost)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::SupportsMove => {
                state.map(|s| UiValue::from(s.supports_move)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::SupportsResize => {
                state.map(|s| UiValue::from(s.supports_resize)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::AcceptsUserInput => {
                state.and_then(|s| s.accepts_user_input()).map(UiValue::from).unwrap_or(UiValue::from(false))
            }
        }
    }
}
