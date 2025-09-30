#![allow(clippy::useless_conversion)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]
use std::sync::Arc;

use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods, PyDict, PyList, PyTuple};
use std::str::FromStr;
use pyo3::IntoPyObject;

use platynui_core as core_rs;
use platynui_runtime as runtime_rs;

use crate::core::{PyNamespace, PyPoint, PyRect, PySize, py_namespace_from_inner};
use platynui_core::ui::FocusablePattern as _;

use pyo3::prelude::PyRef;

// ---------------- Node wrapper ----------------

#[pyclass(name = "UiNode", module = "platynui_native.runtime", unsendable)]
pub struct PyNode {
    pub(crate) inner: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyNode {
    #[getter]
    fn runtime_id(&self) -> String {
        self.inner.runtime_id().as_str().to_string()
    }
    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }
    #[getter]
    fn role(&self) -> &str {
        self.inner.role()
    }
    #[getter]
    fn namespace(&self) -> PyNamespace {
        py_namespace_from_inner(self.inner.namespace())
    }

    /// Returns the attribute value as a Python-native object (None/bool/int/float/str/list/dict/tuples).
    #[pyo3(signature = (name, namespace=None), text_signature = "(self, name, namespace=None)")]
    fn attribute(&self, name: &str, namespace: Option<&str>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ns = core_rs::ui::resolve_namespace(namespace);
        match self.inner.attribute(ns, name) {
            Some(attr) => ui_value_to_py(py, &attr.value()),
            None => Ok(py.None()),
        }
    }

    /// Parent node if available.
    fn parent(&self, py: Python<'_>) -> Option<Py<PyNode>> {
        self.inner
            .parent()
            .and_then(|w| w.upgrade())
            .and_then(|arc| Py::new(py, PyNode { inner: arc }).ok())
    }

    /// Child nodes as a list.
    fn children(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let out = PyList::empty(py);
        for child in self.inner.children() {
            out.append(Py::new(py, PyNode { inner: child })?)?;
        }
        Ok(out.into())
    }

    /// All attributes as objects with `namespace`, `name`, and `value`.
    fn attributes(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let out = PyList::empty(py);
        for a in self.inner.attributes() {
            let ns = a.namespace().as_str().to_string();
            let name = a.name().to_string();
            out.append(Py::new(py, PyAttribute { namespace: ns, name, owner: self.inner.clone() })?)?;
        }
        Ok(out.into())
    }

    /// Pattern identifiers supported by this node.
    fn supported_patterns(&self) -> Vec<String> {
        self.inner.supported_patterns().into_iter().map(|p| p.as_str().to_string()).collect()
    }

    /// Optional document order key used for consistent ordering.
    fn doc_order_key(&self) -> Option<u64> {
        self.inner.doc_order_key()
    }

    /// Invalidate cached state on the underlying node.
    fn invalidate(&self) {
        self.inner.invalidate();
    }

    /// Returns a pattern object for known pattern ids or None if unsupported.
    /// Currently supported ids: "Focusable", "WindowSurface".
    fn pattern_by_id(&self, py: Python<'_>, id: &str) -> Option<Py<PyAny>> {
        match id {
            "Focusable" => {
                Py::new(py, PyFocusable { node: self.inner.clone() }).ok().map(|p| p.into_any())
            }
            "WindowSurface" => Py::new(py, PyWindowSurface { node: self.inner.clone() })
                .ok()
                .map(|p| p.into_any()),
            _ => None,
        }
    }

    /// Convenience boolean: returns True if the node advertises the given pattern id.
    fn has_pattern(&self, id: &str) -> bool {
        self.inner.supported_patterns().iter().any(|p| p.as_str() == id)
    }
}

// ---------------- Pattern wrappers ----------------

#[pyclass(module = "platynui_native.runtime", name = "Focusable")]
pub struct PyFocusable {
    node: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyFocusable {
    fn id(&self) -> &'static str {
        "Focusable"
    }
    fn focus(&self) -> PyResult<()> {
        if let Some(p) = self.node.pattern::<core_rs::ui::pattern::FocusableAction>() {
            p.focus().map_err(|e| PatternError::new_err(e.to_string()))
        } else {
            Err(PatternError::new_err("pattern not available"))
        }
    }
}

#[pyclass(module = "platynui_native.runtime", name = "WindowSurface")]
pub struct PyWindowSurface {
    node: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyWindowSurface {
    fn id(&self) -> &'static str {
        "WindowSurface"
    }

    fn activate(&self) -> PyResult<()> {
        self.call(|p| p.activate())
    }
    fn minimize(&self) -> PyResult<()> {
        self.call(|p| p.minimize())
    }
    fn maximize(&self) -> PyResult<()> {
        self.call(|p| p.maximize())
    }
    fn restore(&self) -> PyResult<()> {
        self.call(|p| p.restore())
    }
    fn close(&self) -> PyResult<()> {
        self.call(|p| p.close())
    }

    fn move_to(&self, x: f64, y: f64) -> PyResult<()> {
        self.call(|p| p.move_to(core_rs::types::Point::new(x, y)))
    }
    fn resize(&self, width: f64, height: f64) -> PyResult<()> {
        self.call(|p| p.resize(core_rs::types::Size::new(width, height)))
    }
    fn move_and_resize(&self, x: f64, y: f64, width: f64, height: f64) -> PyResult<()> {
        self.call(|p| p.move_and_resize(core_rs::types::Rect::new(x, y, width, height)))
    }
    fn accepts_user_input(&self) -> PyResult<Option<bool>> {
        self.with_pattern(|p| p.accepts_user_input())
    }
}

// ---------------- UiAttribute wrapper ----------------

#[pyclass(module = "platynui_native.runtime", name = "UiAttribute", subclass)]
pub struct PyAttribute {
    namespace: String,
    name: String,
    owner: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyAttribute {
    #[getter]
    fn namespace(&self) -> &str {
        &self.namespace
    }
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    /// Lazily resolves the attribute value on demand.
    /// Returns None if the attribute is no longer available.
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ns = core_rs::ui::namespace::Namespace::from_str(self.namespace.as_str())
            .unwrap_or_default();
        match self.owner.attribute(ns, &self.name) {
            Some(attr) => ui_value_to_py(py, &attr.value()),
            None => Ok(py.None()),
        }
    }
    fn __repr__(&self) -> String {
        format!("Attribute(namespace='{}', name='{}')", self.namespace, self.name)
    }
}

#[pyclass(module = "platynui_native.runtime", name = "EvaluatedAttribute")]
pub struct PyEvaluatedAttribute {
    namespace: String,
    name: String,
    value: Py<PyAny>,
    owner: Option<Py<PyNode>>,
}

#[pymethods]
impl PyEvaluatedAttribute {
    #[new]
    #[pyo3(signature = (namespace, name, value, owner=None))]
    fn new(namespace: String, name: String, value: Py<PyAny>, owner: Option<Py<PyNode>>) -> Self {
        Self { namespace, name, value, owner }
    }
    #[getter]
    fn namespace(&self) -> &str {
        &self.namespace
    }
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    #[getter]
    fn value(&self, py: Python<'_>) -> Py<PyAny> {
        self.value.clone_ref(py)
    }
    fn owner(&self, py: Python<'_>) -> Option<Py<PyNode>> {
        self.owner.as_ref().map(|o| o.clone_ref(py))
    }
    fn __repr__(&self) -> String {
        format!("EvaluatedAttribute(namespace='{}', name='{}')", self.namespace, self.name)
    }
}

impl PyWindowSurface {
    fn with_pattern<T, F>(&self, f: F) -> PyResult<T>
    where
        F: FnOnce(
            &dyn core_rs::ui::pattern::WindowSurfacePattern,
        ) -> Result<T, core_rs::ui::pattern::PatternError>,
    {
        // Try to obtain a concrete pattern instance registered for this node.
        // We first attempt the default WindowSurfaceActions type; if not present, fall back to trait-object style via as_any.
        if let Some(p) = self.node.pattern::<core_rs::ui::pattern::WindowSurfaceActions>() {
            return f(&*p).map_err(|e| PatternError::new_err(e.to_string()));
        }
        // Not available as known concrete type; report not available.
        Err(PatternError::new_err("pattern not available"))
    }

    fn call<F>(&self, f: F) -> PyResult<()>
    where
        F: FnOnce(
            &dyn core_rs::ui::pattern::WindowSurfacePattern,
        ) -> Result<(), core_rs::ui::pattern::PatternError>,
    {
        self.with_pattern(|p| f(p))
    }
}

// ---------------- Runtime wrapper ----------------

#[pyclass(name = "Runtime", module = "platynui_native.runtime", unsendable)]
pub struct PyRuntime {
    inner: runtime_rs::Runtime,
}

#[pymethods]
impl PyRuntime {
    #[new]
    fn new() -> PyResult<Self> {
        runtime_rs::Runtime::new().map(|inner| Self { inner }).map_err(map_provider_err)
    }

    /// Evaluates an XPath expression; returns a list of Python-native items:
    /// - Node objects
    /// - dicts for attributes: {"type":"attr", "owner": Node, "namespace": str, "name": str, "value": object}
    /// - plain Python values
    #[pyo3(signature = (xpath, node=None), text_signature = "(xpath: str, node: UiNode | None = None)")]
    fn evaluate(
        &self,
        py: Python<'_>,
        xpath: &str,
        node: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyList>> {
        let node_arc = match node {
            Some(obj) => match obj.extract::<PyRef<PyNode>>() {
                Ok(cellref) => Some(cellref.inner.clone()),
                Err(_) => {
                    return Err(PyTypeError::new_err(
                        "node must be platynui_native.runtime.UiNode",
                    ));
                }
            },
            None => None,
        };
        let items = self.inner.evaluate(node_arc, xpath).map_err(map_eval_err)?;
        let out = PyList::empty(py);
        for item in items {
            match item {
                runtime_rs::EvaluationItem::Node(n) => {
                    let py_node = PyNode { inner: n };
                    out.append(Py::new(py, py_node)?)?;
                }
                runtime_rs::EvaluationItem::Attribute(a) => {
                    let ns = a.namespace.as_str().to_string();
                    let name = a.name.clone();
                    let value = ui_value_to_py(py, &a.value)?;
                    let owner = Py::new(py, PyNode { inner: a.owner.clone() })?;
                    out.append(Py::new(
                        py,
                        PyEvaluatedAttribute::new(ns, name, value, Some(owner)),
                    )?)?;
                }
                runtime_rs::EvaluationItem::Value(v) => out.append(ui_value_to_py(py, &v)?)?,
            }
        }
        Ok(out.into())
    }

    fn shutdown(&mut self) {
        self.inner.shutdown();
    }

    // ---------------- Pointer minimal API ----------------

    /// Returns the current pointer position.
    #[pyo3(text_signature = "(self)")]
    fn pointer_position(&self, py: Python<'_>) -> PyResult<Py<PyPoint>> {
        let p = self.inner.pointer_position().map_err(map_pointer_err)?;
        Py::new(py, PyPoint::from(p))
    }

    /// Moves the pointer to the given point. Accepts a core.Point.
    #[pyo3(signature = (point, overrides=None), text_signature = "(self, point, overrides=None)")]
    fn pointer_move_to(
        &self,
        py: Python<'_>,
        point: PointLike,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<Py<PyPoint>> {
        let p: core_rs::types::Point = point.into();
        let ov = overrides.map(|o| o.inner.clone());
        let new_pos = self.inner.pointer_move_to(p, ov).map_err(map_pointer_err)?;
        Py::new(py, PyPoint::from(new_pos))
    }

    /// Click at point using optional button and overrides.
    #[pyo3(signature = (point, button=None, overrides=None), text_signature = "(self, point, button=None, overrides=None)")]
    fn pointer_click(
        &self,
        point: PointLike,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p: core_rs::types::Point = point.into();
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_click(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Multiple clicks at point.
    #[pyo3(signature = (point, clicks, button=None, overrides=None), text_signature = "(self, point, clicks, button=None, overrides=None)")]
    fn pointer_multi_click(
        &self,
        point: PointLike,
        clicks: u32,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p: core_rs::types::Point = point.into();
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_multi_click(p, btn, clicks, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Drag from start to end with optional button.
    #[pyo3(signature = (start, end, button=None, overrides=None), text_signature = "(self, start, end, button=None, overrides=None)")]
    fn pointer_drag(
        &self,
        start: PointLike,
        end: PointLike,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let s: core_rs::types::Point = start.into();
        let e: core_rs::types::Point = end.into();
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_drag(s, e, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Press pointer button (optionally move first).
    #[pyo3(signature = (point=None, button=None, overrides=None), text_signature = "(self, point=None, button=None, overrides=None)")]
    fn pointer_press(
        &self,
        point: Option<PointLike>,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p = point.map(Into::<core_rs::types::Point>::into);
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_press(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Release pointer button.
    #[pyo3(signature = (button=None, overrides=None), text_signature = "(self, button=None, overrides=None)")]
    fn pointer_release(
        &self,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_release(btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Scroll by delta (h, v) with optional overrides.
    #[pyo3(signature = (delta, overrides=None), text_signature = "(self, delta, overrides=None)")]
    fn pointer_scroll(
        &self,
        delta: ScrollLike,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<()> {
        let ScrollLike::Tuple((h, v)) = delta;
        let ov = overrides.map(|o| o.into());
        self.inner
            .pointer_scroll(core_rs::platform::ScrollDelta::new(h, v), ov)
            .map_err(map_pointer_err)?;
        Ok(())
    }

    // ---------------- Keyboard minimal API ----------------

    /// Types the given keyboard sequence (see runtime docs for syntax).
    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_type(
        &self,
        sequence: &str,
        overrides: Option<PyRef<'_, PyKeyboardOverrides>>,
    ) -> PyResult<()> {
        let ov = overrides.map(|d| d.inner.clone());
        self.inner.keyboard_type(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_press(
        &self,
        sequence: &str,
        overrides: Option<PyRef<'_, PyKeyboardOverrides>>,
    ) -> PyResult<()> {
        let ov = overrides.map(|d| d.inner.clone());
        self.inner.keyboard_press(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_release(
        &self,
        sequence: &str,
        overrides: Option<PyRef<'_, PyKeyboardOverrides>>,
    ) -> PyResult<()> {
        let ov = overrides.map(|d| d.inner.clone());
        self.inner.keyboard_release(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    // ---------------- Desktop & Focus ----------------

    /// Returns the desktop root node.
    fn desktop_node(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        let node = self.inner.desktop_node();
        Py::new(py, PyNode { inner: node })
    }

    /// Returns desktop metadata (dict) including bounds and monitors.
    fn desktop_info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let info = self.inner.desktop_info();
        desktop_info_to_py(py, info)
    }

    /// Sets focus to the given node via the Focusable pattern.
    fn focus(&self, node: PyRef<'_, PyNode>) -> PyResult<()> {
        self.inner.focus(&node.inner).map_err(map_focus_err)?;
        Ok(())
    }
}

// ---------------- Conversions ----------------

fn ui_value_to_py(py: Python<'_>, value: &core_rs::ui::value::UiValue) -> PyResult<Py<PyAny>> {
    use core_rs::ui::value::UiValue as V;
    Ok(match value {
        V::Null => py.None(),
        V::Bool(b) => (*b as i32).into_pyobject(py)?.unbind().into_any(),
        V::Integer(i) => i.into_pyobject(py)?.unbind().into_any(),
        V::Number(n) => n.into_pyobject(py)?.unbind().into_any(),
        V::String(s) => s.clone().into_pyobject(py)?.unbind().into_any(),
        V::Array(items) => {
            let list = PyList::empty(py);
            for it in items {
                list.append(ui_value_to_py(py, it)?)?;
            }
            list.into_pyobject(py)?.unbind().into_any()
        }
        V::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map.iter() {
                dict.set_item(k, ui_value_to_py(py, v)?)?;
            }
            dict.into_pyobject(py)?.unbind().into_any()
        }
        V::Point(p) => Py::new(py, PyPoint::from(*p))?.into_any(),
        V::Size(s) => Py::new(py, PySize::from(*s))?.into_any(),
        V::Rect(r) => Py::new(py, PyRect::from(*r))?.into_any(),
    })
}

fn rect_to_py(py: Python<'_>, r: &core_rs::types::Rect) -> PyResult<Py<PyAny>> {
    Py::new(py, PyRect::from(*r)).map(|p| p.into_any())
}

fn desktop_info_to_py(py: Python<'_>, info: &core_rs::platform::DesktopInfo) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("runtime_id", info.runtime_id.as_str())?;
    dict.set_item("name", &info.name)?;
    dict.set_item("technology", info.technology.as_str())?;
    dict.set_item("bounds", rect_to_py(py, &info.bounds)?)?;
    dict.set_item("os_name", &info.os_name)?;
    dict.set_item("os_version", &info.os_version)?;

    let monitors = PyList::empty(py);
    for m in &info.monitors {
        let md = PyDict::new(py);
        md.set_item("id", &m.id)?;
        if let Some(name) = &m.name { md.set_item("name", name)?; } else { md.set_item("name", py.None())?; }
        md.set_item("bounds", rect_to_py(py, &m.bounds)?)?;
        md.set_item("is_primary", m.is_primary)?;
        if let Some(scale) = m.scale_factor { md.set_item("scale_factor", scale)?; } else { md.set_item("scale_factor", py.None())?; }
        monitors.append(md)?;
    }
    dict.set_item("monitors", monitors)?;
    Ok(dict.into_pyobject(py)?.unbind().into_any())
}

// ---------------- Error mapping ----------------

fn map_provider_err(err: core_rs::provider::ProviderError) -> PyErr {
    ProviderError::new_err(err.to_string())
}
fn map_eval_err(err: runtime_rs::EvaluateError) -> PyErr {
    EvaluationError::new_err(err.to_string())
}
fn map_pointer_err(err: runtime_rs::PointerError) -> PyErr {
    PointerError::new_err(err.to_string())
}
fn map_keyboard_err(err: runtime_rs::runtime::KeyboardActionError) -> PyErr {
    KeyboardError::new_err(err.to_string())
}

fn map_focus_err(err: runtime_rs::runtime::FocusError) -> PyErr {
    PatternError::new_err(err.to_string())
}

// ---------------- Module init ----------------

// ---------------- Exceptions ----------------

pyo3::create_exception!(runtime, EvaluationError, PyException);
pyo3::create_exception!(runtime, ProviderError, PyException);
pyo3::create_exception!(runtime, PointerError, PyException);
pyo3::create_exception!(runtime, KeyboardError, PyException);
pyo3::create_exception!(runtime, PatternError, PyException);

pub fn init_submodule(
    py: Python<'_>,
    m: &Bound<'_, PyModule>,
    _core: &Bound<'_, PyModule>,
) -> PyResult<()> {
    m.add_class::<PyRuntime>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyAttribute>()?;
    m.add_class::<PyEvaluatedAttribute>()?;
    m.add_class::<PyPointerOverrides>()?;
    m.add_class::<PyKeyboardOverrides>()?;
    // Create a Python IntEnum for pointer buttons: 1=LEFT, 2=MIDDLE, 3=RIGHT
    {
        let enum_mod = PyModule::import(py, "enum")?;
        let int_enum = enum_mod.getattr("IntEnum")?;
        let dict = PyDict::new(py);
        dict.set_item("LEFT", 1)?;
        dict.set_item("MIDDLE", 2)?;
        dict.set_item("RIGHT", 3)?;
        let args = ("PointerButton", dict);
        let py_enum = int_enum.call1(args)?;
        m.add("PointerButton", py_enum)?;
    }
    // exceptions
    m.add("EvaluationError", py.get_type::<EvaluationError>())?;
    m.add("ProviderError", py.get_type::<ProviderError>())?;
    m.add("PointerError", py.get_type::<PointerError>())?;
    m.add("KeyboardError", py.get_type::<KeyboardError>())?;
    m.add("PatternError", py.get_type::<PatternError>())?;
    Ok(())
}

// ---------------- FromPyObject helpers ----------------

#[derive(FromPyObject)]
pub enum PointLike<'py> {
    Point(PyRef<'py, PyPoint>),
}

impl From<PointLike<'_>> for core_rs::types::Point {
    fn from(v: PointLike<'_>) -> Self {
        match v {
            PointLike::Point(p) => p.as_inner(),
        }
    }
}

#[derive(FromPyObject)]
pub enum RectLike<'py> {
    Tuple((f64, f64, f64, f64)),
    Rect(PyRef<'py, PyRect>),
}

impl From<RectLike<'_>> for core_rs::types::Rect {
    fn from(v: RectLike<'_>) -> Self {
        match v {
            RectLike::Tuple((x, y, w, h)) => core_rs::types::Rect::new(x, y, w, h),
            RectLike::Rect(r) => r.as_inner(),
        }
    }
}

// ---------------- Helpers ----------------

fn dict_get<'py>(d: &Bound<'py, PyDict>, key: &str) -> Option<Bound<'py, PyAny>> {
    d.get_item(key).ok().flatten()
}

// ---------------- FromPyObject-friendly wrappers ----------------

#[derive(FromPyObject)]
pub enum PointerButtonLike {
    Int(u16),
}

impl From<PointerButtonLike> for core_rs::platform::PointerButton {
    fn from(v: PointerButtonLike) -> Self {
        match v {
            // Ints map 1=Left, 2=Middle, 3=Right, else Other(n). This also covers IntEnum instances.
            PointerButtonLike::Int(n) => match n {
                1 => Self::Left,
                2 => Self::Middle,
                3 => Self::Right,
                _ => Self::Other(n),
            },
        }
    }
}

#[derive(FromPyObject)]
pub enum ScrollLike {
    Tuple((f64, f64)),
}

// ---------------- Concrete overrides classes (Python-visible) ----------------

#[pyclass(module = "platynui_native.runtime", name = "PointerOverrides")]
pub struct PyPointerOverrides {
    pub(crate) inner: runtime_rs::PointerOverrides,
}

#[pymethods]
impl PyPointerOverrides {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (*,
        origin=None,
        speed_factor=None,
        acceleration_profile=None,
        max_move_duration_ms=None,
        move_time_per_pixel_us=None,
        after_move_delay_ms=None,
        after_input_delay_ms=None,
        press_release_delay_ms=None,
        after_click_delay_ms=None,
        before_next_click_delay_ms=None,
        multi_click_delay_ms=None,
        ensure_move_threshold=None,
        ensure_move_timeout_ms=None,
        scroll_step=None,
        scroll_delay_ms=None,
    ))]
    fn new(
        origin: Option<OriginInput>,
        speed_factor: Option<f64>,
        acceleration_profile: Option<String>,
        max_move_duration_ms: Option<f64>,
        move_time_per_pixel_us: Option<f64>,
        after_move_delay_ms: Option<f64>,
        after_input_delay_ms: Option<f64>,
        press_release_delay_ms: Option<f64>,
        after_click_delay_ms: Option<f64>,
        before_next_click_delay_ms: Option<f64>,
        multi_click_delay_ms: Option<f64>,
        ensure_move_threshold: Option<f64>,
        ensure_move_timeout_ms: Option<f64>,
        scroll_step: Option<(f64, f64)>,
        scroll_delay_ms: Option<f64>,
    ) -> Self {
        let input = PointerOverridesInput {
            origin,
            speed_factor,
            acceleration_profile,
            max_move_duration_ms,
            move_time_per_pixel_us,
            after_move_delay_ms,
            after_input_delay_ms,
            press_release_delay_ms,
            after_click_delay_ms,
            before_next_click_delay_ms,
            multi_click_delay_ms,
            ensure_move_threshold,
            ensure_move_timeout_ms,
            scroll_step,
            scroll_delay_ms,
        };
        Self { inner: input.into() }
    }

    fn __repr__(&self) -> String {
        "PointerOverrides(...)".to_string()
    }

    // ----- getters (read-only properties) -----
    #[getter]
    fn origin(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        use core_rs::platform::PointOrigin as O;
        self.inner.origin.as_ref().and_then(|o| match o {
            O::Desktop => "desktop".into_pyobject(py).ok().map(|v| v.unbind().into_any()),
            O::Absolute(p) => Py::new(py, PyPoint::from(*p)).ok().map(|v| v.into_any()),
            O::Bounds(r) => Py::new(py, PyRect::from(*r)).ok().map(|v| v.into_any()),
        })
    }
    #[getter]
    fn speed_factor(&self) -> Option<f64> {
        self.inner.speed_factor
    }
    #[getter]
    fn acceleration_profile(&self) -> Option<String> {
        use core_rs::platform::PointerAccelerationProfile as Accel;
        self.inner.acceleration_profile.map(|p| {
            match p {
                Accel::Constant => "constant",
                Accel::EaseIn => "ease_in",
                Accel::EaseOut => "ease_out",
                Accel::SmoothStep => "smooth_step",
            }
            .to_string()
        })
    }
    #[getter]
    fn max_move_duration_ms(&self) -> Option<f64> {
        self.inner.max_move_duration.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn move_time_per_pixel_us(&self) -> Option<f64> {
        self.inner.move_time_per_pixel.map(|d| d.as_micros() as f64)
    }
    #[getter]
    fn after_move_delay_ms(&self) -> Option<f64> {
        self.inner.after_move_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn after_input_delay_ms(&self) -> Option<f64> {
        self.inner.after_input_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn press_release_delay_ms(&self) -> Option<f64> {
        self.inner.press_release_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn after_click_delay_ms(&self) -> Option<f64> {
        self.inner.after_click_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn before_next_click_delay_ms(&self) -> Option<f64> {
        self.inner.before_next_click_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn multi_click_delay_ms(&self) -> Option<f64> {
        self.inner.multi_click_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn ensure_move_threshold(&self) -> Option<f64> {
        self.inner.ensure_move_threshold
    }
    #[getter]
    fn ensure_move_timeout_ms(&self) -> Option<f64> {
        self.inner.ensure_move_timeout.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn scroll_step(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.inner
            .scroll_step
            .and_then(|d| PyTuple::new(py, [d.horizontal, d.vertical]).ok().map(|t| t.unbind().into_any()))
    }
    #[getter]
    fn scroll_delay_ms(&self) -> Option<f64> {
        self.inner.scroll_delay.map(|d| d.as_millis() as f64)
    }
}

#[pyclass(module = "platynui_native.runtime", name = "KeyboardOverrides")]
pub struct PyKeyboardOverrides {
    pub(crate) inner: core_rs::platform::KeyboardOverrides,
}

#[pymethods]
impl PyKeyboardOverrides {
    #[new]
    #[pyo3(signature = (*,
        press_delay_ms=None,
        release_delay_ms=None,
        between_keys_delay_ms=None,
        chord_press_delay_ms=None,
        chord_release_delay_ms=None,
        after_sequence_delay_ms=None,
        after_text_delay_ms=None,
    ))]
    fn new(
        press_delay_ms: Option<f64>,
        release_delay_ms: Option<f64>,
        between_keys_delay_ms: Option<f64>,
        chord_press_delay_ms: Option<f64>,
        chord_release_delay_ms: Option<f64>,
        after_sequence_delay_ms: Option<f64>,
        after_text_delay_ms: Option<f64>,
    ) -> Self {
        let input = KeyboardOverridesInput {
            press_delay_ms,
            release_delay_ms,
            between_keys_delay_ms,
            chord_press_delay_ms,
            chord_release_delay_ms,
            after_sequence_delay_ms,
            after_text_delay_ms,
        };
        Self { inner: input.into() }
    }

    fn __repr__(&self) -> String {
        "KeyboardOverrides(...)".to_string()
    }

    // ----- getters (read-only properties) -----
    #[getter]
    fn press_delay_ms(&self) -> Option<f64> {
        self.inner.press_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn release_delay_ms(&self) -> Option<f64> {
        self.inner.release_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn between_keys_delay_ms(&self) -> Option<f64> {
        self.inner.between_keys_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn chord_press_delay_ms(&self) -> Option<f64> {
        self.inner.chord_press_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn chord_release_delay_ms(&self) -> Option<f64> {
        self.inner.chord_release_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn after_sequence_delay_ms(&self) -> Option<f64> {
        self.inner.after_sequence_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    fn after_text_delay_ms(&self) -> Option<f64> {
        self.inner.after_text_delay.map(|d| d.as_millis() as f64)
    }
}
pub struct PointerOverridesInput {
    pub origin: Option<OriginInput>,
    pub speed_factor: Option<f64>,
    pub acceleration_profile: Option<String>,
    pub max_move_duration_ms: Option<f64>,
    pub move_time_per_pixel_us: Option<f64>,
    pub after_move_delay_ms: Option<f64>,
    pub after_input_delay_ms: Option<f64>,
    pub press_release_delay_ms: Option<f64>,
    pub after_click_delay_ms: Option<f64>,
    pub before_next_click_delay_ms: Option<f64>,
    pub multi_click_delay_ms: Option<f64>,
    pub ensure_move_threshold: Option<f64>,
    pub ensure_move_timeout_ms: Option<f64>,
    pub scroll_step: Option<(f64, f64)>,
    pub scroll_delay_ms: Option<f64>,
}

impl<'py> pyo3::FromPyObject<'py> for PointerOverridesInput {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let d = ob.downcast::<PyDict>()?;
        Ok(Self {
            origin: dict_get(d, "origin").map(|v| OriginInput::extract_bound(&v)).transpose()?,
            speed_factor: dict_get(d, "speed_factor").and_then(|v| v.extract().ok()),
            acceleration_profile: dict_get(d, "acceleration_profile")
                .and_then(|v| v.extract().ok()),
            max_move_duration_ms: dict_get(d, "max_move_duration_ms")
                .and_then(|v| v.extract().ok()),
            move_time_per_pixel_us: dict_get(d, "move_time_per_pixel_us")
                .and_then(|v| v.extract().ok()),
            after_move_delay_ms: dict_get(d, "after_move_delay_ms").and_then(|v| v.extract().ok()),
            after_input_delay_ms: dict_get(d, "after_input_delay_ms")
                .and_then(|v| v.extract().ok()),
            press_release_delay_ms: dict_get(d, "press_release_delay_ms")
                .and_then(|v| v.extract().ok()),
            after_click_delay_ms: dict_get(d, "after_click_delay_ms")
                .and_then(|v| v.extract().ok()),
            before_next_click_delay_ms: dict_get(d, "before_next_click_delay_ms")
                .and_then(|v| v.extract().ok()),
            multi_click_delay_ms: dict_get(d, "multi_click_delay_ms")
                .and_then(|v| v.extract().ok()),
            ensure_move_threshold: dict_get(d, "ensure_move_threshold")
                .and_then(|v| v.extract().ok()),
            ensure_move_timeout_ms: dict_get(d, "ensure_move_timeout_ms")
                .and_then(|v| v.extract().ok()),
            scroll_step: dict_get(d, "scroll_step").and_then(|v| v.extract().ok()),
            scroll_delay_ms: dict_get(d, "scroll_delay_ms").and_then(|v| v.extract().ok()),
        })
    }
}

impl From<PointerOverridesInput> for runtime_rs::PointerOverrides {
    fn from(s: PointerOverridesInput) -> Self {
        use core_rs::platform::PointerAccelerationProfile as Accel;
        let mut ov = runtime_rs::PointerOverrides::new();
        if let Some(origin) = s.origin {
            ov = ov.origin(origin.into());
        }
        if let Some(v) = s.speed_factor {
            ov = ov.speed_factor(v);
        }
        if let Some(ms) = s.after_move_delay_ms {
            ov = ov.after_move_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.after_input_delay_ms {
            ov = ov.after_input_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.press_release_delay_ms {
            ov = ov.press_release_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.after_click_delay_ms {
            ov = ov.after_click_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.before_next_click_delay_ms {
            ov = ov.before_next_click_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.multi_click_delay_ms {
            ov = ov.multi_click_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(v) = s.ensure_move_threshold {
            ov = ov.ensure_move_threshold(v);
        }
        if let Some(ms) = s.ensure_move_timeout_ms {
            ov = ov.ensure_move_timeout(std::time::Duration::from_millis(ms as u64));
        }
        if let Some((h, v)) = s.scroll_step {
            ov = ov.scroll_step(core_rs::platform::ScrollDelta::new(h, v));
        }
        if let Some(ms) = s.scroll_delay_ms {
            ov = ov.scroll_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.max_move_duration_ms {
            ov = ov.move_duration(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(us) = s.move_time_per_pixel_us {
            ov = ov.move_time_per_pixel(std::time::Duration::from_micros(us as u64));
        }
        if let Some(s) = s.acceleration_profile {
            let ap = match s.to_ascii_lowercase().as_str() {
                "constant" => Accel::Constant,
                "ease_in" => Accel::EaseIn,
                "ease_out" => Accel::EaseOut,
                _ => Accel::SmoothStep,
            };
            ov = ov.acceleration_profile(ap);
        }
        ov
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(FromPyObject)]
pub enum PointerOverridesLike<'py> {
    Dict(PointerOverridesInput),
    Class(PyRef<'py, PyPointerOverrides>),
}

impl From<PointerOverridesLike<'_>> for runtime_rs::PointerOverrides {
    fn from(v: PointerOverridesLike<'_>) -> Self {
        match v {
            PointerOverridesLike::Dict(d) => d.into(),
            PointerOverridesLike::Class(c) => c.inner.clone(),
        }
    }
}

pub enum OriginInput {
    Desktop,
    Absolute((f64, f64)),
    Bounds((f64, f64, f64, f64)),
}

impl<'py> pyo3::FromPyObject<'py> for OriginInput {
    fn extract_bound(obj: &Bound<'py, PyAny>) -> PyResult<Self> {
        if let Ok(s) = obj.extract::<String>()
            && s.eq_ignore_ascii_case("desktop")
        {
            return Ok(OriginInput::Desktop);
        }
        if let Ok(p) = obj.extract::<PyRef<PyPoint>>() {
            let pi = p.as_inner();
            return Ok(OriginInput::Absolute((pi.x(), pi.y())));
        }
        if let Ok(r) = obj.extract::<PyRef<PyRect>>() {
            let ri = r.as_inner();
            return Ok(OriginInput::Bounds((ri.x(), ri.y(), ri.width(), ri.height())));
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "invalid origin: expected 'desktop', core.Point or core.Rect",
        ))
    }
}

impl From<OriginInput> for core_rs::platform::PointOrigin {
    fn from(o: OriginInput) -> Self {
        match o {
            OriginInput::Desktop => Self::Desktop,
            OriginInput::Absolute((x, y)) => Self::Absolute(core_rs::types::Point::new(x, y)),
            OriginInput::Bounds((x, y, w, h)) => {
                Self::Bounds(core_rs::types::Rect::new(x, y, w, h))
            }
        }
    }
}

pub struct KeyboardOverridesInput {
    pub press_delay_ms: Option<f64>,
    pub release_delay_ms: Option<f64>,
    pub between_keys_delay_ms: Option<f64>,
    pub chord_press_delay_ms: Option<f64>,
    pub chord_release_delay_ms: Option<f64>,
    pub after_sequence_delay_ms: Option<f64>,
    pub after_text_delay_ms: Option<f64>,
}

impl<'py> pyo3::FromPyObject<'py> for KeyboardOverridesInput {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let d = ob.downcast::<PyDict>()?;
        Ok(Self {
            press_delay_ms: dict_get(d, "press_delay_ms").and_then(|v| v.extract().ok()),
            release_delay_ms: dict_get(d, "release_delay_ms").and_then(|v| v.extract().ok()),
            between_keys_delay_ms: dict_get(d, "between_keys_delay_ms")
                .and_then(|v| v.extract().ok()),
            chord_press_delay_ms: dict_get(d, "chord_press_delay_ms")
                .and_then(|v| v.extract().ok()),
            chord_release_delay_ms: dict_get(d, "chord_release_delay_ms")
                .and_then(|v| v.extract().ok()),
            after_sequence_delay_ms: dict_get(d, "after_sequence_delay_ms")
                .and_then(|v| v.extract().ok()),
            after_text_delay_ms: dict_get(d, "after_text_delay_ms").and_then(|v| v.extract().ok()),
        })
    }
}

impl From<KeyboardOverridesInput> for core_rs::platform::KeyboardOverrides {
    fn from(s: KeyboardOverridesInput) -> Self {
        use core_rs::platform::KeyboardOverrides as KO;
        let mut ov = KO::new();
        if let Some(ms) = s.press_delay_ms {
            ov = ov.press_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.release_delay_ms {
            ov = ov.release_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.between_keys_delay_ms {
            ov = ov.between_keys_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.chord_press_delay_ms {
            ov = ov.chord_press_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.chord_release_delay_ms {
            ov = ov.chord_release_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.after_sequence_delay_ms {
            ov = ov.after_sequence_delay(std::time::Duration::from_millis(ms as u64));
        }
        if let Some(ms) = s.after_text_delay_ms {
            ov = ov.after_text_delay(std::time::Duration::from_millis(ms as u64));
        }
        ov
    }
}

#[derive(FromPyObject)]
pub enum KeyboardOverridesLike<'py> {
    Dict(KeyboardOverridesInput),
    Class(PyRef<'py, PyKeyboardOverrides>),
}

impl From<KeyboardOverridesLike<'_>> for core_rs::platform::KeyboardOverrides {
    fn from(v: KeyboardOverridesLike<'_>) -> Self {
        match v {
            KeyboardOverridesLike::Dict(d) => d.into(),
            KeyboardOverridesLike::Class(c) => c.inner.clone(),
        }
    }
}
