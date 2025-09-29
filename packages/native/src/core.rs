#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
use pyo3::prelude::*;
use pyo3::types::PyDict;

use platynui_core as core_rs;

// ---------- Point ----------

#[pyclass(name = "Point", module = "platynui_native.core")]
#[derive(Clone)]
pub struct PyPoint {
    inner: core_rs::types::Point,
}

#[pymethods]
impl PyPoint {
    #[new]
    fn new(x: f64, y: f64) -> Self {
        Self { inner: core_rs::types::Point::new(x, y) }
    }
    #[getter]
    fn x(&self) -> f64 {
        self.inner.x()
    }
    #[getter]
    fn y(&self) -> f64 {
        self.inner.y()
    }
    fn to_tuple(&self) -> (f64, f64) {
        (self.inner.x(), self.inner.y())
    }
    fn __repr__(&self) -> String {
        format!("Point({}, {})", self.inner.x(), self.inner.y())
    }
    // richcmp intentionally omitted for now; default identity semantics are fine for MVP
}

impl From<core_rs::types::Point> for PyPoint {
    fn from(p: core_rs::types::Point) -> Self {
        Self { inner: p }
    }
}

// ---------- Size ----------

#[pyclass(name = "Size", module = "platynui_native.core")]
#[derive(Clone)]
pub struct PySize {
    inner: core_rs::types::Size,
}

#[pymethods]
impl PySize {
    #[new]
    fn new(width: f64, height: f64) -> Self {
        Self { inner: core_rs::types::Size::new(width, height) }
    }
    #[getter]
    fn width(&self) -> f64 {
        self.inner.width()
    }
    #[getter]
    fn height(&self) -> f64 {
        self.inner.height()
    }
    fn to_tuple(&self) -> (f64, f64) {
        (self.inner.width(), self.inner.height())
    }
    fn __repr__(&self) -> String {
        format!("Size({}, {})", self.inner.width(), self.inner.height())
    }
}

impl From<core_rs::types::Size> for PySize {
    fn from(s: core_rs::types::Size) -> Self {
        Self { inner: s }
    }
}

// ---------- Rect ----------

#[pyclass(name = "Rect", module = "platynui_native.core")]
#[derive(Clone)]
pub struct PyRect {
    inner: core_rs::types::Rect,
}

#[pymethods]
impl PyRect {
    #[new]
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { inner: core_rs::types::Rect::new(x, y, width, height) }
    }
    #[getter]
    fn x(&self) -> f64 {
        self.inner.x()
    }
    #[getter]
    fn y(&self) -> f64 {
        self.inner.y()
    }
    #[getter]
    fn width(&self) -> f64 {
        self.inner.width()
    }
    #[getter]
    fn height(&self) -> f64 {
        self.inner.height()
    }
    fn to_tuple(&self) -> (f64, f64, f64, f64) {
        (self.inner.x(), self.inner.y(), self.inner.width(), self.inner.height())
    }
    fn __repr__(&self) -> String {
        format!(
            "Rect({}, {}, {}, {})",
            self.inner.x(),
            self.inner.y(),
            self.inner.width(),
            self.inner.height()
        )
    }
}

impl From<core_rs::types::Rect> for PyRect {
    fn from(r: core_rs::types::Rect) -> Self {
        Self { inner: r }
    }
}

// ---------- IDs ----------

macro_rules! define_id {
    ($name:ident, $rust:ty) => {
        #[pyclass(module = "platynui_native.core", frozen)]
        #[derive(Clone)]
        pub struct $name {
            inner: $rust,
        }

        #[pymethods]
        impl $name {
            #[new]
            fn new(value: &str) -> Self {
                Self { inner: value.to_string().into() }
            }
            fn as_str(&self) -> &str {
                self.inner.as_str()
            }
            fn __repr__(&self) -> String {
                format!("{}('{}')", stringify!($name), self.inner.as_str())
            }
            fn __str__(&self) -> &str {
                self.inner.as_str()
            }
        }
    };
}

define_id!(PatternId, core_rs::ui::identifiers::PatternId);
define_id!(RuntimeId, core_rs::ui::identifiers::RuntimeId);
define_id!(TechnologyId, core_rs::ui::identifiers::TechnologyId);

// ---------- Namespace ----------

#[pyclass(name = "Namespace", module = "platynui_native.core", frozen)]
#[derive(Clone, Copy)]
pub struct PyNamespace {
    inner: core_rs::ui::namespace::Namespace,
}

#[pymethods]
impl PyNamespace {
    #[allow(non_snake_case)]
    #[classattr]
    fn Control() -> Self {
        Self { inner: core_rs::ui::namespace::Namespace::Control }
    }
    #[allow(non_snake_case)]
    #[classattr]
    fn Item() -> Self {
        Self { inner: core_rs::ui::namespace::Namespace::Item }
    }
    #[allow(non_snake_case)]
    #[classattr]
    fn App() -> Self {
        Self { inner: core_rs::ui::namespace::Namespace::App }
    }
    #[allow(non_snake_case)]
    #[classattr]
    fn Native() -> Self {
        Self { inner: core_rs::ui::namespace::Namespace::Native }
    }

    fn as_str(&self) -> &'static str {
        self.inner.as_str()
    }
    fn is_default(&self) -> bool {
        self.inner.is_default()
    }
    fn __repr__(&self) -> String {
        format!("Namespace('{}')", self.inner.as_str())
    }
    fn __str__(&self) -> &'static str {
        self.inner.as_str()
    }
}

pub(crate) fn py_namespace_from_inner(ns: core_rs::ui::namespace::Namespace) -> PyNamespace {
    PyNamespace { inner: ns }
}

#[pyfunction]
fn all_namespaces(py: Python<'_>) -> PyResult<PyObject> {
    let list = pyo3::types::PyList::empty_bound(py);
    for ns in core_rs::ui::all_namespaces() {
        list.append(Py::new(py, PyNamespace { inner: ns })?)?;
    }
    Ok(list.into_py(py))
}

#[pyfunction]
#[pyo3(signature = (prefix=None))]
fn resolve_namespace(prefix: Option<&str>) -> PyNamespace {
    PyNamespace { inner: core_rs::ui::resolve_namespace(prefix) }
}

// ---------- attribute_names() ----------

#[pyfunction]
fn attribute_names(py: Python<'_>) -> PyResult<PyObject> {
    let m = pyo3::types::PyDict::new_bound(py);
    macro_rules! group {
        ($ident:ident, $name:literal, { $($k:ident),* $(,)? }) => {{
            let d = PyDict::new_bound(py);
            $( d.set_item(stringify!($k), core_rs::ui::attributes::pattern::$ident::$k)?; )*
            m.set_item($name, d)?;
        }};
    }
    group!(common, "common", { ROLE, NAME, RUNTIME_ID, TECHNOLOGY, SUPPORTED_PATTERNS });
    group!(element, "element", { BOUNDS, IS_VISIBLE, IS_ENABLED, IS_OFFSCREEN });
    group!(desktop, "desktop", { BOUNDS, DISPLAY_COUNT, MONITORS, OS_NAME, OS_VERSION });
    group!(text_content, "text_content", { TEXT, LOCALE, IS_TRUNCATED });
    group!(text_editable, "text_editable", { IS_READ_ONLY, MAX_LENGTH, SUPPORTS_PASSWORD_MODE });
    group!(text_selection, "text_selection", { CARET_POSITION, SELECTION_RANGES, SELECTION_ANCHOR, SELECTION_ACTIVE });
    group!(selectable, "selectable", { IS_SELECTED, SELECTION_CONTAINER_ID, SELECTION_ORDER });
    group!(selection_provider, "selection_provider", { SELECTION_MODE, SELECTED_IDS, SUPPORTS_RANGE_SELECTION });
    group!(toggleable, "toggleable", { TOGGLE_STATE, SUPPORTS_THREE_STATE });
    group!(stateful_value, "stateful_value", { CURRENT_VALUE, MINIMUM, MAXIMUM, SMALL_CHANGE, LARGE_CHANGE, UNIT });
    group!(activatable, "activatable", { IS_ACTIVATION_ENABLED, DEFAULT_ACCELERATOR });
    group!(activation_target, "activation_target", { ACTIVATION_POINT, ACTIVATION_AREA, ACTIVATION_HINT });
    group!(focusable, "focusable", { IS_FOCUSED });
    group!(scrollable, "scrollable", { HORIZONTAL_PERCENT, VERTICAL_PERCENT, CAN_SCROLL_HORIZONTALLY, CAN_SCROLL_VERTICALLY, HORIZONTAL_VIEW_SIZE, VERTICAL_VIEW_SIZE, SCROLL_GRANULARITY });
    group!(expandable, "expandable", { IS_EXPANDED, HAS_CHILDREN });
    group!(item_container, "item_container", { ITEM_COUNT, IS_VIRTUALIZED, VIRTUALIZATION_HINT, SUPPORTS_CONTAINER_SEARCH });
    group!(window_surface, "window_surface", { IS_MINIMIZED, IS_MAXIMIZED, IS_TOPMOST, SUPPORTS_RESIZE, SUPPORTS_MOVE, ACCEPTS_USER_INPUT });
    group!(dialog_surface, "dialog_surface", { IS_MODAL, DEFAULT_RESULT });
    group!(application, "application", { PROCESS_ID, PROCESS_NAME, EXECUTABLE_PATH, COMMAND_LINE, USER_NAME, START_TIME, MAIN_WINDOW_IDS, ARCHITECTURE, ACCEPTS_USER_INPUT });
    group!(highlightable, "highlightable", { SUPPORTS_HIGHLIGHT, HIGHLIGHT_STYLES });
    group!(annotatable, "annotatable", { ANNOTATIONS });
    Ok(m.into_py(py))
}

pub fn init_submodule(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPoint>()?;
    m.add_class::<PySize>()?;
    m.add_class::<PyRect>()?;
    m.add_class::<PatternId>()?;
    m.add_class::<RuntimeId>()?;
    m.add_class::<TechnologyId>()?;
    m.add_class::<PyNamespace>()?;
    m.add_function(wrap_pyfunction!(all_namespaces, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_namespace, m)?)?;
    m.add_function(wrap_pyfunction!(attribute_names, m)?)?;
    Ok(())
}

// expose inner values for runtime conversions
impl PyPoint {
    pub(crate) fn as_inner(&self) -> core_rs::types::Point {
        self.inner
    }
}
impl PyRect {
    pub(crate) fn as_inner(&self) -> core_rs::types::Rect {
        self.inner
    }
}
