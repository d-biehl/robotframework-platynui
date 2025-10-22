#![allow(clippy::useless_conversion)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unexpected_cfgs)]
use std::sync::Arc;

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods, PyDict, PyIterator, PyList, PyTuple};
use std::str::FromStr;

use platynui_core as core_rs;
use platynui_core::platform::{HighlightRequest, PixelFormat, ScreenshotRequest};
use platynui_runtime as runtime_rs;

use crate::core::{PyNamespace, PyPoint, PyRect, PySize, py_namespace_from_inner};
use platynui_core::ui::FocusablePattern as _;

use pyo3::prelude::PyRef;

// ---------------- Node wrapper ----------------

#[pyclass(name = "UiNode", module = "platynui_native")]
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
    fn id(&self) -> Option<String> {
        self.inner.id()
    }
    #[getter]
    fn name(&self) -> String {
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
        self.inner.parent().and_then(|w| w.upgrade()).and_then(|arc| Py::new(py, PyNode { inner: arc }).ok())
    }

    /// Child nodes as an iterator.
    fn children(&self, py: Python<'_>) -> PyResult<Py<PyNodeChildrenIterator>> {
        let iter = self.inner.children();
        Py::new(py, PyNodeChildrenIterator { iter: Some(iter) })
    }

    /// All attributes as an iterator of objects with `namespace`, `name`, and `value`.
    fn attributes(&self, py: Python<'_>) -> PyResult<Py<PyNodeAttributesIterator>> {
        let iter = self.inner.attributes();
        let owner = self.inner.clone();
        Py::new(py, PyNodeAttributesIterator { iter: Some(iter), owner })
    }

    /// Pattern identifiers supported by this node.
    fn supported_patterns(&self) -> Vec<String> {
        self.inner.supported_patterns().into_iter().map(|p| p.as_str().to_string()).collect()
    }

    /// Optional document order key used for consistent ordering.
    fn doc_order_key(&self) -> Option<u64> {
        self.inner.doc_order_key()
    }

    /// Returns whether the underlying platform node is still valid/available.
    /// Providers may override this to perform a cheap liveness check.
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Invalidate cached state on the underlying node.
    fn invalidate(&self) {
        self.inner.invalidate();
    }

    /// Returns a pattern object for known pattern ids or None if unsupported.
    /// Currently supported ids: "Focusable", "WindowSurface".
    fn pattern_by_id(&self, py: Python<'_>, id: &str) -> Option<Py<PyAny>> {
        match id {
            "Focusable" => Py::new(py, PyFocusable { node: self.inner.clone() }).ok().map(|p| p.into_any()),
            "WindowSurface" => Py::new(py, PyWindowSurface { node: self.inner.clone() }).ok().map(|p| p.into_any()),
            _ => None,
        }
    }

    /// Convenience boolean: returns True if the node advertises the given pattern id.
    fn has_pattern(&self, id: &str) -> bool {
        self.inner.supported_patterns().iter().any(|p| p.as_str() == id)
    }
}

// ---------------- Iterator for UiNode children ----------------

#[pyclass(name = "NodeChildrenIterator", module = "platynui_native", unsendable)]
pub struct PyNodeChildrenIterator {
    iter: Option<Box<dyn Iterator<Item = Arc<dyn core_rs::ui::UiNode>> + Send + 'static>>,
}

#[pymethods]
impl PyNodeChildrenIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<PyNode>>> {
        if let Some(ref mut iter) = slf.iter
            && let Some(child) = iter.next()
        {
            return Ok(Some(Py::new(py, PyNode { inner: child })?));
        }
        slf.iter = None;
        Ok(None)
    }
}

// ---------------- Iterator for UiNode attributes ----------------

#[pyclass(name = "NodeAttributesIterator", module = "platynui_native", unsendable)]
pub struct PyNodeAttributesIterator {
    iter: Option<Box<dyn Iterator<Item = Arc<dyn core_rs::ui::UiAttribute>> + Send + 'static>>,
    owner: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyNodeAttributesIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<PyAttribute>>> {
        if let Some(ref mut iter) = slf.iter
            && let Some(attr) = iter.next()
        {
            let ns = attr.namespace().as_str().to_string();
            let name = attr.name().to_string();
            return Ok(Some(Py::new(py, PyAttribute { namespace: ns, name, owner: slf.owner.clone() })?));
        }
        slf.iter = None;
        Ok(None)
    }
}

// ---------------- Iterator for Runtime evaluation results ----------------

#[pyclass(name = "EvaluationIterator", module = "platynui_native", unsendable)]
pub struct PyEvaluationIterator {
    iter: Option<Box<dyn Iterator<Item = runtime_rs::EvaluationItem>>>,
}

#[pymethods]
impl PyEvaluationIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if let Some(ref mut iter) = slf.iter
            && let Some(item) = iter.next()
        {
            let result = evaluation_item_to_py(py, &item)?;
            return Ok(Some(result));
        }
        slf.iter = None;
        Ok(None)
    }
}

// ---------------- Pattern wrappers ----------------

#[pyclass(module = "platynui_native", name = "Focusable")]
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

#[pyclass(module = "platynui_native", name = "WindowSurface")]
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

#[pyclass(module = "platynui_native", name = "UiAttribute", subclass)]
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
        let ns = core_rs::ui::namespace::Namespace::from_str(self.namespace.as_str()).unwrap_or_default();
        match self.owner.attribute(ns, &self.name) {
            Some(attr) => ui_value_to_py(py, &attr.value()),
            None => Ok(py.None()),
        }
    }
    fn __repr__(&self) -> String {
        format!("Attribute(namespace='{}', name='{}')", self.namespace, self.name)
    }
}

#[pyclass(module = "platynui_native", name = "EvaluatedAttribute")]
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
        F: FnOnce(&dyn core_rs::ui::pattern::WindowSurfacePattern) -> Result<T, core_rs::ui::pattern::PatternError>,
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
        F: FnOnce(&dyn core_rs::ui::pattern::WindowSurfacePattern) -> Result<(), core_rs::ui::pattern::PatternError>,
    {
        self.with_pattern(|p| f(p))
    }
}

// ---------------- Runtime wrapper ----------------

// ---------------- Runtime ----------------

#[pyclass(name = "Runtime", module = "platynui_native")]
pub struct PyRuntime {
    inner: runtime_rs::Runtime,
}

#[pymethods]
impl PyRuntime {
    #[new]
    fn new() -> PyResult<Self> {
        runtime_rs::Runtime::new().map(|inner| Self { inner }).map_err(map_provider_err)
    }

    // ---------------- Static builder (mock only) ----------------

    /// Create a Runtime fully backed by the bundled mock provider and mock platform devices.
    /// Compiles only when built with the `mock-provider` feature.
    #[staticmethod]
    fn new_with_mock() -> PyResult<Self> {
        #[cfg(feature = "mock-provider")]
        {
            let factories: [&'static dyn core_rs::provider::UiTreeProviderFactory; 1] =
                [&platynui_provider_mock::MOCK_PROVIDER_FACTORY];
            let platforms = runtime_rs::runtime::PlatformOverrides {
                desktop_info: Some(&platynui_platform_mock::MOCK_PLATFORM),
                highlight: Some(&platynui_platform_mock::MOCK_HIGHLIGHT),
                screenshot: Some(&platynui_platform_mock::MOCK_SCREENSHOT),
                pointer: Some(&platynui_platform_mock::MOCK_POINTER),
                keyboard: Some(&platynui_platform_mock::MOCK_KEYBOARD),
            };
            return runtime_rs::Runtime::new_with_factories_and_platforms(&factories, platforms)
                .map(|inner| Self { inner })
                .map_err(map_provider_err);
        }
        #[cfg(not(feature = "mock-provider"))]
        {
            Err(ProviderError::new_err("Runtime.new_with_mock() requires building with feature 'mock-provider'"))
        }
    }

    /// Evaluates an XPath expression; returns a list of items:
    /// - UiNode objects (platynui_native.UiNode)
    /// - EvaluatedAttribute objects (platynui_native.EvaluatedAttribute)
    /// - plain Python values (None/bool/int/float/str/list/dict/Point/Size/Rect)
    #[pyo3(signature = (xpath, node=None), text_signature = "(xpath: str, node: UiNode | None = None)")]
    fn evaluate(&self, py: Python<'_>, xpath: &str, node: Option<Bound<'_, PyAny>>) -> PyResult<Py<PyList>> {
        let node_arc = match node {
            Some(obj) => match obj.extract::<PyRef<PyNode>>() {
                Ok(cellref) => Some(cellref.inner.clone()),
                Err(_) => {
                    return Err(PyTypeError::new_err("node must be platynui_native.runtime.UiNode"));
                }
            },
            None => None,
        };
        let items = self.inner.evaluate(node_arc, xpath).map_err(map_eval_err)?;
        let out = PyList::empty(py);
        for item in items {
            out.append(evaluation_item_to_py(py, &item)?)?;
        }
        Ok(out.into())
    }

    /// Evaluates an XPath expression and returns the first result, or None if no results.
    /// Possible return types are UiNode, EvaluatedAttribute, or a plain Python value.
    #[pyo3(signature = (xpath, node=None), text_signature = "(xpath: str, node: UiNode | None = None)")]
    fn evaluate_single(&self, py: Python<'_>, xpath: &str, node: Option<Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        let node_arc = match node {
            Some(obj) => match obj.extract::<PyRef<PyNode>>() {
                Ok(cellref) => Some(cellref.inner.clone()),
                Err(_) => {
                    return Err(PyTypeError::new_err("node must be platynui_native.runtime.UiNode"));
                }
            },
            None => None,
        };

        let item = self.inner.evaluate_single(node_arc, xpath).map_err(map_eval_err)?;

        match item {
            Some(it) => evaluation_item_to_py(py, &it),
            None => Ok(py.None()),
        }
    }

    fn shutdown(&mut self) {
        self.inner.shutdown();
    }

    /// Evaluates an XPath expression and returns an iterator over results.
    #[pyo3(signature = (xpath, node=None), text_signature = "(xpath: str, node: UiNode | None = None)")]
    fn evaluate_iter(
        &self,
        py: Python<'_>,
        xpath: &str,
        node: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyEvaluationIterator>> {
        let node_arc = match node {
            Some(obj) => match obj.extract::<PyRef<PyNode>>() {
                Ok(cellref) => Some(cellref.inner.clone()),
                Err(_) => {
                    return Err(PyTypeError::new_err("node must be platynui_native.runtime.UiNode"));
                }
            },
            None => None,
        };

        // Build owned evaluation stream via Runtime helper and box it for Python iterator
        let stream = self.inner.evaluate_iter_owned(node_arc, xpath).map_err(map_eval_err)?;
        Py::new(py, PyEvaluationIterator { iter: Some(Box::new(stream)) })
    }

    /// Returns a list of active provider information dictionaries.
    fn providers(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for provider in self.inner.providers() {
            let desc = provider.descriptor();
            let dict = PyDict::new(py);
            dict.set_item("id", desc.id)?;
            dict.set_item("display_name", desc.display_name)?;
            dict.set_item("technology", desc.technology.as_str())?;
            dict.set_item("kind", format!("{:?}", desc.kind))?;
            list.append(dict)?;
        }
        Ok(list.into())
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
        point: PyRef<'_, crate::core::PyPoint>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<Py<PyPoint>> {
        let p: core_rs::types::Point = point.as_inner();
        let ov = overrides.map(|o| o.inner.clone());
        let new_pos = self.inner.pointer_move_to(p, ov).map_err(map_pointer_err)?;
        Py::new(py, PyPoint::from(new_pos))
    }

    /// Click at point using optional button and overrides.
    #[pyo3(signature = (point, button=None, overrides=None), text_signature = "(self, point, button=None, overrides=None)")]
    fn pointer_click(
        &self,
        point: Option<PyRef<'_, crate::core::PyPoint>>,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p: Option<core_rs::types::Point> = point.map(|r| r.as_inner());
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_click(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Multiple clicks at point.
    #[pyo3(signature = (point=None, clicks=2, button=None, overrides=None), text_signature = "(self, point=None, clicks=2, button=None, overrides=None)")]
    fn pointer_multi_click(
        &self,
        point: Option<PyRef<'_, crate::core::PyPoint>>,
        clicks: u32,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p: Option<core_rs::types::Point> = point.map(|r| r.as_inner());
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_multi_click(p, btn, clicks, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Drag from start to end with optional button.
    #[pyo3(signature = (start, end, button=None, overrides=None), text_signature = "(self, start, end, button=None, overrides=None)")]
    fn pointer_drag(
        &self,
        start: PyRef<'_, crate::core::PyPoint>,
        end: PyRef<'_, crate::core::PyPoint>,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let s: core_rs::types::Point = start.as_inner();
        let e: core_rs::types::Point = end.as_inner();
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_drag(s, e, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Press pointer button (optionally move first).
    #[pyo3(signature = (point=None, button=None, overrides=None), text_signature = "(self, point=None, button=None, overrides=None)")]
    fn pointer_press(
        &self,
        point: Option<PyRef<'_, crate::core::PyPoint>>,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p = point.map(|r| r.as_inner());
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_press(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Release pointer button (optionally move first).
    #[pyo3(signature = (point=None, button=None, overrides=None), text_signature = "(self, point=None, button=None, overrides=None)")]
    fn pointer_release(
        &self,
        point: Option<PyRef<'_, crate::core::PyPoint>>,
        button: Option<PointerButtonLike>,
        overrides: Option<PyRef<'_, PyPointerOverrides>>,
    ) -> PyResult<()> {
        let p: Option<core_rs::types::Point> = point.map(|r| r.as_inner());
        let btn = button.map(|b| b.into());
        let ov = overrides.map(|o| o.inner.clone());
        self.inner.pointer_release(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Scroll by delta (h, v) with optional overrides.
    #[pyo3(signature = (delta, overrides=None), text_signature = "(self, delta, overrides=None)")]
    fn pointer_scroll(&self, delta: ScrollLike, overrides: Option<PointerOverridesLike>) -> PyResult<()> {
        let ScrollLike::Tuple((h, v)) = delta;
        let ov = overrides.map(|o| o.into());
        self.inner.pointer_scroll(core_rs::platform::ScrollDelta::new(h, v), ov).map_err(map_pointer_err)?;
        Ok(())
    }

    // ---------------- Keyboard minimal API ----------------

    /// Types the given keyboard sequence (see runtime docs for syntax).
    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_type(&self, sequence: &str, overrides: Option<PyRef<'_, PyKeyboardOverrides>>) -> PyResult<()> {
        let ov = overrides.map(|d| d.inner.clone());
        self.inner.keyboard_type(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_press(&self, sequence: &str, overrides: Option<PyRef<'_, PyKeyboardOverrides>>) -> PyResult<()> {
        let ov = overrides.map(|d| d.inner.clone());
        self.inner.keyboard_press(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_release(&self, sequence: &str, overrides: Option<PyRef<'_, PyKeyboardOverrides>>) -> PyResult<()> {
        let ov = overrides.map(|d| d.inner.clone());
        self.inner.keyboard_release(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    /// Returns the list of known key names supported by the active keyboard device.
    #[pyo3(text_signature = "(self)")]
    fn keyboard_known_key_names(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let names = self.inner.keyboard_known_key_names().map_err(|e| PyException::new_err(e.to_string()))?;
        let list = PyList::new(py, names)?;
        Ok(list.unbind())
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

    // ---------------- Highlight & Screenshot ----------------

    /// Highlights one or more rectangles for an optional duration (milliseconds).
    /// Accepts a single Rect or any Python Iterable[Rect] (e.g., list, tuple, generator).
    #[pyo3(signature = (rects, duration_ms=None), text_signature = "(self, rects, duration_ms=None)")]
    fn highlight(&self, rects: Bound<'_, PyAny>, duration_ms: Option<f64>) -> PyResult<()> {
        let mut all: Vec<platynui_core::types::Rect> = Vec::new();
        // Fast path: single Rect passed directly
        if let Ok(r) = rects.extract::<PyRef<PyRect>>() {
            all.push(r.as_inner());
        } else {
            // Fallback: consume any iterable of Rects
            let iter = PyIterator::from_object(&rects)?;
            for item in iter {
                let any = item?;
                let r: PyRef<PyRect> = any.extract()?;
                all.push(r.as_inner());
            }
        }
        let mut req = HighlightRequest::from_rects(all);
        if let Some(ms) = duration_ms {
            req = req.with_duration(std::time::Duration::from_millis(ms as u64));
        }
        self.inner.highlight(&req).map_err(map_platform_err)?;
        Ok(())
    }

    /// Clears an active highlight overlay if available.
    fn clear_highlight(&self) -> PyResult<()> {
        self.inner.clear_highlight().map_err(map_platform_err)?;
        Ok(())
    }

    /// Captures a screenshot and returns encoded bytes. Supports only 'image/png'.
    #[pyo3(signature = (rect=None, mime_type=None), text_signature = "(self, rect=None, mime_type=None)")]
    fn screenshot(&self, py: Python<'_>, rect: Option<PyRef<PyRect>>, mime_type: Option<&str>) -> PyResult<Py<PyAny>> {
        let effective_mime = mime_type.unwrap_or("image/png");
        if !effective_mime.eq_ignore_ascii_case("image/png") {
            return Err(PyTypeError::new_err("unsupported mime_type; only 'image/png' is supported"));
        }
        let request = rect
            .map(|r| ScreenshotRequest::with_region(r.as_inner()))
            .unwrap_or_else(ScreenshotRequest::entire_display);
        let shot = self.inner.screenshot(&request).map_err(map_platform_err)?;
        let encoded = encode_png(&shot)?;
        let pybytes = pyo3::types::PyBytes::new(py, &encoded);
        Ok(pybytes.into_pyobject(py)?.unbind().into_any())
    }
}

// ---------------- Conversions ----------------

fn ui_value_to_py(py: Python<'_>, value: &core_rs::ui::value::UiValue) -> PyResult<Py<PyAny>> {
    use core_rs::ui::value::UiValue as V;
    Ok(match value {
        V::Null => py.None(),
        V::Bool(b) => pyo3::types::PyBool::new(py, *b).to_owned().into(),
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

/// Convert a runtime EvaluationItem into its Python representation.
/// - Node      -> platynui_native.UiNode
/// - Attribute -> platynui_native.EvaluatedAttribute
/// - Value     -> native Python value via ui_value_to_py
fn evaluation_item_to_py(py: Python<'_>, item: &runtime_rs::EvaluationItem) -> PyResult<Py<PyAny>> {
    Ok(match item {
        runtime_rs::EvaluationItem::Node(n) => {
            // Clone Arc to create a Python-visible node wrapper
            let py_node = PyNode { inner: n.clone() };
            Py::new(py, py_node)?.into_any()
        }
        runtime_rs::EvaluationItem::Attribute(a) => {
            let ns = a.namespace.as_str().to_string();
            let name = a.name.clone();
            let value = ui_value_to_py(py, &a.value)?;
            let owner = Py::new(py, PyNode { inner: a.owner.clone() })?;
            Py::new(py, PyEvaluatedAttribute::new(ns, name, value, Some(owner)))?.into_any()
        }
        runtime_rs::EvaluationItem::Value(v) => ui_value_to_py(py, v)?,
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
        if let Some(name) = &m.name {
            md.set_item("name", name)?;
        } else {
            md.set_item("name", py.None())?;
        }
        md.set_item("bounds", rect_to_py(py, &m.bounds)?)?;
        md.set_item("is_primary", m.is_primary)?;
        if let Some(scale) = m.scale_factor {
            md.set_item("scale_factor", scale)?;
        } else {
            md.set_item("scale_factor", py.None())?;
        }
        monitors.append(md)?;
    }
    dict.set_item("monitors", monitors)?;
    Ok(dict.into_pyobject(py)?.unbind().into_any())
}

fn to_rgba_bytes(shot: &core_rs::platform::Screenshot) -> Vec<u8> {
    match shot.format {
        PixelFormat::Rgba8 => shot.pixels.clone(),
        PixelFormat::Bgra8 => {
            let mut converted = shot.pixels.clone();
            for chunk in converted.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            converted
        }
    }
}

fn encode_png(shot: &core_rs::platform::Screenshot) -> PyResult<Vec<u8>> {
    use png::{BitDepth, ColorType, Encoder};
    let mut data = Vec::new();
    let mut encoder = Encoder::new(&mut data, shot.width, shot.height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| PyTypeError::new_err(format!("png header error: {e}")))?;
    let rgba = to_rgba_bytes(shot);
    writer.write_image_data(&rgba).map_err(|e| PyTypeError::new_err(format!("png encode error: {e}")))?;
    drop(writer);
    Ok(data)
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

fn map_platform_err(err: core_rs::platform::PlatformError) -> PyErr {
    ProviderError::new_err(err.to_string())
}

// ---------------- Module init ----------------

// ---------------- Exceptions ----------------

pyo3::create_exception!(runtime, EvaluationError, PyException);
pyo3::create_exception!(runtime, ProviderError, PyException);
pyo3::create_exception!(runtime, PointerError, PyException);
pyo3::create_exception!(runtime, KeyboardError, PyException);
pyo3::create_exception!(runtime, PatternError, PyException);

/// Register all runtime types and functions directly into the module (no submodule).
pub fn register_types(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRuntime>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyNodeChildrenIterator>()?;
    m.add_class::<PyNodeAttributesIterator>()?;
    m.add_class::<PyEvaluationIterator>()?;
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

    // ---- Export mock handles for Python (opaque usize values) ----
    // These exist for convenience in tests; they are zero when the mock feature is disabled.
    #[cfg(feature = "mock-provider")]
    {
        // Provider factory handle
        let mock_provider = unsafe {
            std::mem::transmute::<*const platynui_provider_mock::MockProviderFactory, usize>(
                &platynui_provider_mock::MOCK_PROVIDER_FACTORY as *const _,
            )
        };
        m.add("MOCK_PROVIDER", mock_provider)?;

        // Platform devices/providers
        let mock_platform =
            unsafe { std::mem::transmute::<_, usize>(&platynui_platform_mock::MOCK_PLATFORM as *const _) };
        let mock_highlight =
            unsafe { std::mem::transmute::<_, usize>(&platynui_platform_mock::MOCK_HIGHLIGHT as *const _) };
        let mock_screenshot =
            unsafe { std::mem::transmute::<_, usize>(&platynui_platform_mock::MOCK_SCREENSHOT as *const _) };
        let mock_pointer =
            unsafe { std::mem::transmute::<_, usize>(&platynui_platform_mock::MOCK_POINTER as *const _) };
        let mock_keyboard =
            unsafe { std::mem::transmute::<_, usize>(&platynui_platform_mock::MOCK_KEYBOARD as *const _) };

        m.add("MOCK_PLATFORM", mock_platform)?;
        m.add("MOCK_HIGHLIGHT_PROVIDER", mock_highlight)?;
        m.add("MOCK_SCREENSHOT_PROVIDER", mock_screenshot)?;
        m.add("MOCK_POINTER_DEVICE", mock_pointer)?;
        m.add("MOCK_KEYBOARD_DEVICE", mock_keyboard)?;
    }
    #[cfg(not(feature = "mock-provider"))]
    {
        m.add("MOCK_PROVIDER", 0usize)?;
        m.add("MOCK_PLATFORM", 0usize)?;
        m.add("MOCK_HIGHLIGHT_PROVIDER", 0usize)?;
        m.add("MOCK_SCREENSHOT_PROVIDER", 0usize)?;
        m.add("MOCK_POINTER_DEVICE", 0usize)?;
        m.add("MOCK_KEYBOARD_DEVICE", 0usize)?;
    }

    Ok(())
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

#[pyclass(module = "platynui_native", name = "PointerOverrides")]
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

#[pyclass(module = "platynui_native", name = "KeyboardOverrides")]
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

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for PointerOverridesInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let d_borrowed = ob.cast::<PyDict>()?;
        let d: &Bound<'py, PyDict> = &d_borrowed;
        Ok(Self {
            origin: dict_get(d, "origin").map(|v| OriginInput::extract((&v).into())).transpose()?,
            speed_factor: dict_get(d, "speed_factor").and_then(|v| v.extract().ok()),
            acceleration_profile: dict_get(d, "acceleration_profile").and_then(|v| v.extract().ok()),
            max_move_duration_ms: dict_get(d, "max_move_duration_ms").and_then(|v| v.extract().ok()),
            move_time_per_pixel_us: dict_get(d, "move_time_per_pixel_us").and_then(|v| v.extract().ok()),
            after_move_delay_ms: dict_get(d, "after_move_delay_ms").and_then(|v| v.extract().ok()),
            after_input_delay_ms: dict_get(d, "after_input_delay_ms").and_then(|v| v.extract().ok()),
            press_release_delay_ms: dict_get(d, "press_release_delay_ms").and_then(|v| v.extract().ok()),
            after_click_delay_ms: dict_get(d, "after_click_delay_ms").and_then(|v| v.extract().ok()),
            before_next_click_delay_ms: dict_get(d, "before_next_click_delay_ms").and_then(|v| v.extract().ok()),
            multi_click_delay_ms: dict_get(d, "multi_click_delay_ms").and_then(|v| v.extract().ok()),
            ensure_move_threshold: dict_get(d, "ensure_move_threshold").and_then(|v| v.extract().ok()),
            ensure_move_timeout_ms: dict_get(d, "ensure_move_timeout_ms").and_then(|v| v.extract().ok()),
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
            PointerOverridesLike::Class(c) => (*c).inner.clone(),
        }
    }
}

pub enum OriginInput {
    Desktop,
    Absolute((f64, f64)),
    Bounds((f64, f64, f64, f64)),
}

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for OriginInput {
    type Error = PyErr;
    fn extract(obj: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
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
        Err(pyo3::exceptions::PyTypeError::new_err("invalid origin: expected 'desktop', core.Point or core.Rect"))
    }
}

impl From<OriginInput> for core_rs::platform::PointOrigin {
    fn from(o: OriginInput) -> Self {
        match o {
            OriginInput::Desktop => Self::Desktop,
            OriginInput::Absolute((x, y)) => Self::Absolute(core_rs::types::Point::new(x, y)),
            OriginInput::Bounds((x, y, w, h)) => Self::Bounds(core_rs::types::Rect::new(x, y, w, h)),
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

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for KeyboardOverridesInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let d_borrowed = ob.cast::<PyDict>()?;
        let d: &Bound<'py, PyDict> = &d_borrowed;
        Ok(Self {
            press_delay_ms: dict_get(d, "press_delay_ms").and_then(|v| v.extract().ok()),
            release_delay_ms: dict_get(d, "release_delay_ms").and_then(|v| v.extract().ok()),
            between_keys_delay_ms: dict_get(d, "between_keys_delay_ms").and_then(|v| v.extract().ok()),
            chord_press_delay_ms: dict_get(d, "chord_press_delay_ms").and_then(|v| v.extract().ok()),
            chord_release_delay_ms: dict_get(d, "chord_release_delay_ms").and_then(|v| v.extract().ok()),
            after_sequence_delay_ms: dict_get(d, "after_sequence_delay_ms").and_then(|v| v.extract().ok()),
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
            KeyboardOverridesLike::Class(c) => (*c).inner.clone(),
        }
    }
}
