use crate::events;
use crate::focus;
use crate::tree::AttributeSpec;
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{
    activatable, closeable, element, maximizable, minimizable, movable, resizable,
};
use platynui_core::ui::{
    ActivatableAction, CloseableAction, MaximizableAction, MinimizableAction, MovableAction, Namespace, PatternError,
    PatternRegistry, ResizableAction, ResponsiveAction, RestorableAction, RuntimeId, UiAttribute, UiPattern, UiValue,
};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

#[derive(Clone, Debug)]
pub(crate) struct WindowConfig {
    pub bounds: Rect,
    pub is_active: bool,
    pub is_minimized: bool,
    pub is_maximized: bool,
    pub is_topmost: bool,
    pub can_minimize: bool,
    pub can_maximize: bool,
    pub can_close: bool,
    pub can_move: bool,
    pub can_resize: bool,
    pub accepts_user_input: Option<bool>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            is_active: false,
            is_minimized: false,
            is_maximized: false,
            is_topmost: false,
            can_minimize: true,
            can_maximize: true,
            can_close: true,
            can_move: true,
            can_resize: true,
            accepts_user_input: None,
        }
    }
}

#[derive(Clone, Debug)]
struct WindowState {
    bounds: Rect,
    is_active: bool,
    is_minimized: bool,
    is_maximized: bool,
    is_topmost: bool,
    can_minimize: bool,
    can_maximize: bool,
    can_close: bool,
    can_move: bool,
    can_resize: bool,
    accepts_user_input: Option<bool>,
}

impl From<WindowConfig> for WindowState {
    fn from(config: WindowConfig) -> Self {
        Self {
            bounds: config.bounds,
            is_active: config.is_active,
            is_minimized: config.is_minimized,
            is_maximized: config.is_maximized,
            is_topmost: config.is_topmost,
            can_minimize: config.can_minimize,
            can_maximize: config.can_maximize,
            can_close: config.can_close,
            can_move: config.can_move,
            can_resize: config.can_resize,
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
            activatable::IS_ACTIVE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_active = value;
                }
            }
            activatable::IS_TOPMOST => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_topmost = value;
                }
            }
            minimizable::IS_MINIMIZED => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_minimized = value;
                }
            }
            minimizable::CAN_MINIMIZE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.can_minimize = value;
                }
            }
            maximizable::IS_MAXIMIZED => {
                if let Some(value) = as_bool(attr.value()) {
                    config.is_maximized = value;
                }
            }
            maximizable::CAN_MAXIMIZE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.can_maximize = value;
                }
            }
            closeable::CAN_CLOSE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.can_close = value;
                }
            }
            movable::CAN_MOVE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.can_move = value;
                }
            }
            resizable::CAN_RESIZE => {
                if let Some(value) = as_bool(attr.value()) {
                    config.can_resize = value;
                }
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
            || n == activatable::IS_ACTIVE
            || n == activatable::IS_TOPMOST
            || n == minimizable::IS_MINIMIZED
            || n == minimizable::CAN_MINIMIZE
            || n == maximizable::IS_MAXIMIZED
            || n == maximizable::CAN_MAXIMIZE
            || n == closeable::CAN_CLOSE
            || n == movable::CAN_MOVE
            || n == resizable::CAN_RESIZE
    )
}

pub(crate) fn register_window(
    runtime_id: RuntimeId,
    namespace: Namespace,
    config: WindowConfig,
    registry: &PatternRegistry,
) -> Vec<Arc<dyn UiAttribute>> {
    WINDOW_STATES.write().unwrap().insert(runtime_id.clone(), WindowState::from(config));

    register_patterns(runtime_id.clone(), registry);

    vec![
        window_attribute(namespace, runtime_id.clone(), element::BOUNDS, WindowAttributeKind::Bounds),
        window_attribute(namespace, runtime_id.clone(), activatable::IS_ACTIVE, WindowAttributeKind::IsActive),
        window_attribute(namespace, runtime_id.clone(), activatable::IS_TOPMOST, WindowAttributeKind::IsTopmost),
        window_attribute(namespace, runtime_id.clone(), minimizable::IS_MINIMIZED, WindowAttributeKind::IsMinimized),
        window_attribute(namespace, runtime_id.clone(), minimizable::CAN_MINIMIZE, WindowAttributeKind::CanMinimize),
        window_attribute(namespace, runtime_id.clone(), maximizable::IS_MAXIMIZED, WindowAttributeKind::IsMaximized),
        window_attribute(namespace, runtime_id.clone(), maximizable::CAN_MAXIMIZE, WindowAttributeKind::CanMaximize),
        window_attribute(namespace, runtime_id.clone(), closeable::CAN_CLOSE, WindowAttributeKind::CanClose),
        window_attribute(namespace, runtime_id.clone(), movable::CAN_MOVE, WindowAttributeKind::CanMove),
        window_attribute(namespace, runtime_id, resizable::CAN_RESIZE, WindowAttributeKind::CanResize),
    ]
}

fn register_patterns(runtime_id: RuntimeId, registry: &PatternRegistry) {
    let id = runtime_id.clone();
    registry.register_lazy(ActivatableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(ActivatableAction::new(move || activate(&id))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id.clone();
    registry.register_lazy(MinimizableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(MinimizableAction::new(move || minimize(&id))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id.clone();
    registry.register_lazy(MaximizableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(MaximizableAction::new(move || maximize(&id))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id.clone();
    registry.register_lazy(RestorableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(RestorableAction::new(move || restore(&id))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id.clone();
    registry.register_lazy(CloseableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(CloseableAction::new(move || close(&id))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id.clone();
    registry.register_lazy(MovableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(MovableAction::new(move |point| move_to(&id, point))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id.clone();
    registry.register_lazy(ResizableAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id).then(|| Arc::new(ResizableAction::new(move |size| resize(&id, size))) as Arc<dyn UiPattern>)
    });

    let id = runtime_id;
    registry.register_lazy(ResponsiveAction::static_pattern_name(), move || {
        let id = id.clone();
        state_exists(&id)
            .then(|| Arc::new(ResponsiveAction::new(move || accepts_user_input(&id))) as Arc<dyn UiPattern>)
    });
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
        state.is_active = true;
        state.is_minimized = false;
        state.is_topmost = true;
        Ok(())
    })?;
    focus::request_focus(runtime_id.clone())
}

fn minimize(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        if !state.can_minimize {
            return Err(PatternError::new("window does not support minimize"));
        }
        state.is_minimized = true;
        state.is_maximized = false;
        state.is_active = false;
        Ok(())
    })?;
    focus::clear_if_matches(runtime_id);
    Ok(())
}

fn maximize(runtime_id: &RuntimeId) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        if !state.can_maximize {
            return Err(PatternError::new("window does not support maximize"));
        }
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
    mutate_state(runtime_id, |state| {
        if !state.can_close {
            return Err(PatternError::new("window does not support close"));
        }
        Ok(())
    })?;
    focus::clear_if_matches(runtime_id);
    Ok(())
}

fn move_to(runtime_id: &RuntimeId, position: Point) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        if !state.can_move {
            return Err(PatternError::new("window does not support move"));
        }
        let size = state.bounds.size();
        state.bounds = Rect::new(position.x(), position.y(), size.width(), size.height());
        Ok(())
    })
}

fn resize(runtime_id: &RuntimeId, size: Size) -> Result<(), PatternError> {
    mutate_state(runtime_id, |state| {
        if !state.can_resize {
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
    IsActive,
    IsTopmost,
    IsMinimized,
    CanMinimize,
    IsMaximized,
    CanMaximize,
    CanClose,
    CanMove,
    CanResize,
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
            WindowAttributeKind::IsActive => state.map(|s| UiValue::from(s.is_active)).unwrap_or(UiValue::from(false)),
            WindowAttributeKind::IsTopmost => {
                state.map(|s| UiValue::from(s.is_topmost)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::IsMinimized => {
                state.map(|s| UiValue::from(s.is_minimized)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::CanMinimize => {
                state.map(|s| UiValue::from(s.can_minimize)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::IsMaximized => {
                state.map(|s| UiValue::from(s.is_maximized)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::CanMaximize => {
                state.map(|s| UiValue::from(s.can_maximize)).unwrap_or(UiValue::from(false))
            }
            WindowAttributeKind::CanClose => state.map(|s| UiValue::from(s.can_close)).unwrap_or(UiValue::from(false)),
            WindowAttributeKind::CanMove => state.map(|s| UiValue::from(s.can_move)).unwrap_or(UiValue::from(false)),
            WindowAttributeKind::CanResize => {
                state.map(|s| UiValue::from(s.can_resize)).unwrap_or(UiValue::from(false))
            }
        }
    }
}
