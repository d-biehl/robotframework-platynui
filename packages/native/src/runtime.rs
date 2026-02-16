use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods, PyDict, PyIterator, PyList, PyModule, PyTuple, PyType};
use std::str::FromStr;

use core_rs::ui::UiNodeExt;
use platynui_core as core_rs;
use platynui_core::platform::{HighlightRequest, PixelFormat, ScreenshotRequest};
use platynui_runtime as runtime_rs;

use crate::core::{PyNamespace, PyPoint, PyRect, PySize, py_namespace_from_inner};
use platynui_core::ui::FocusablePattern as _;

use pyo3::prelude::PyRef;

static NEXT_CACHE_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static CACHE_MAP: RefCell<HashMap<u64, runtime_rs::XdmCache>> = RefCell::new(HashMap::new());
}

fn with_cache<R>(cache_id: u64, f: impl FnOnce(&runtime_rs::XdmCache) -> R) -> R {
    CACHE_MAP.with(|map| {
        let mut map = map.borrow_mut();
        let cache = map.entry(cache_id).or_insert_with(runtime_rs::XdmCache::new);
        f(cache)
    })
}

fn remove_cache(cache_id: u64) {
    CACHE_MAP.with(|map| {
        map.borrow_mut().remove(&cache_id);
    });
}

// ---------------- Node wrapper ----------------

/// Represents a single UI element that was discovered through the runtime.
///
/// The object lets Python code inspect metadata (identifiers, attributes),
/// traverse the accessibility tree, and invoke supported interaction patterns
/// such as focus or window management.
#[pyclass(name = "UiNode", module = "platynui_native")]
pub struct PyNode {
    pub(crate) inner: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyNode {
    /// Returns the provider-stable identifier for this node.
    #[getter]
    fn runtime_id(&self) -> String {
        self.inner.runtime_id().as_str().to_string()
    }
    /// Returns the optional, human-readable identifier if the platform exposes one.
    #[getter]
    fn id(&self) -> Option<String> {
        self.inner.id()
    }
    /// Returns the localized name announced for this node.
    #[getter]
    fn name(&self) -> String {
        self.inner.name()
    }
    /// Returns the semantic role/type of the node (for example, "Button").
    #[getter]
    fn role(&self) -> &str {
        self.inner.role()
    }
    /// Returns the attribute namespace this node lives in.
    #[getter]
    fn namespace(&self) -> PyNamespace {
        py_namespace_from_inner(self.inner.namespace())
    }

    /// Looks up an attribute and returns its value as a native Python object.
    ///
    /// The namespace parameter accepts a string such as ``"control"``; when omitted
    /// the default namespace for the node is used.
    #[pyo3(signature = (name, namespace=None), text_signature = "(self, name, namespace=None)")]
    fn attribute(&self, name: &str, namespace: Option<&str>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ns = core_rs::ui::resolve_namespace(namespace);
        match self.inner.attribute(ns, name) {
            Some(attr) => ui_value_to_py(py, &attr.value()),
            None => Err(AttributeNotFoundError::new_err(format!(
                "attribute not found: {}:{} on {}",
                ns.as_str(),
                name,
                self.inner.runtime_id().as_str()
            ))),
        }
    }

    /// Returns the immediate parent node, or ``None`` for the desktop root.
    fn parent(&self, py: Python<'_>) -> Option<Py<PyNode>> {
        self.inner.parent().and_then(|w| w.upgrade()).and_then(|arc| Py::new(py, PyNode { inner: arc }).ok())
    }

    /// Returns a list of ancestors beginning with the closest parent.
    ///
    /// The desktop root is omitted to avoid duplicated top-level entries.
    fn ancestors(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for node in self.inner.ancestors() {
            list.append(Py::new(py, PyNode { inner: node })?)?;
        }
        Ok(list.unbind())
    }

    /// Returns the node followed by its ancestors (closest first).
    ///
    /// This is useful when walking upwards until a predicate is met.
    fn ancestors_including_self(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for node in self.inner.ancestors_including_self() {
            list.append(Py::new(py, PyNode { inner: node })?)?;
        }
        Ok(list.unbind())
    }

    /// Returns the outermost ancestor above the node.
    ///
    /// If the node already represents a top-level element, the node itself is
    /// returned.
    fn top_level_or_self(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        let node = self.inner.top_level_or_self();
        Py::new(py, PyNode { inner: node })
    }

    /// Returns the first ancestor (including ``self``) that supports the given pattern.
    ///
    /// ``id`` accepts names such as ``"Focusable"`` or ``"WindowSurface"`` and
    /// returns the Python pattern object when available.
    fn ancestor_pattern(&self, py: Python<'_>, id: &str) -> Option<Py<PyAny>> {
        let pid = core_rs::ui::identifiers::PatternId::from(id);
        for node in self.inner.ancestors_including_self() {
            if node.pattern_by_id(&pid).is_some() {
                return pattern_object(py, &node, id);
            }
        }
        None
    }

    /// Returns the requested pattern object from the top-level ancestor, if supported.
    fn top_level_pattern(&self, py: Python<'_>, id: &str) -> Option<Py<PyAny>> {
        let top = self.inner.top_level_or_self();
        let pid = core_rs::ui::identifiers::PatternId::from(id);
        if top.pattern_by_id(&pid).is_some() {
            return pattern_object(py, &top, id);
        }
        None
    }

    /// Returns an iterator that yields the direct children as ``UiNode`` objects.
    fn children(&self, py: Python<'_>) -> PyResult<Py<PyNodeChildrenIterator>> {
        let iter = self.inner.children();
        Py::new(py, PyNodeChildrenIterator { iter: Some(iter) })
    }

    /// Returns an iterator that yields attribute handles for the node.
    ///
    /// Each item is a ``UiAttribute`` exposing ``namespace``, ``name`` and
    /// ``value()``.
    fn attributes(&self, py: Python<'_>) -> PyResult<Py<PyNodeAttributesIterator>> {
        let iter = self.inner.attributes();
        let owner = self.inner.clone();
        Py::new(py, PyNodeAttributesIterator { iter: Some(iter), owner })
    }

    /// Returns a list of pattern identifiers supported by the node.
    fn supported_patterns(&self) -> Vec<String> {
        self.inner.supported_patterns().into_iter().map(|p| p.as_str().to_string()).collect()
    }

    /// Returns a stable ordering key when the provider assigns one.
    fn doc_order_key(&self) -> Option<u64> {
        self.inner.doc_order_key()
    }

    /// Returns ``True`` when the node still refers to a live platform element.
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Tells the provider to refresh any cached information for this node.
    fn invalidate(&self) {
        self.inner.invalidate();
    }

    /// Returns the requested interaction pattern object or ``None`` when unsupported.
    ///
    /// Known pattern ids include ``"Focusable"`` and ``"WindowSurface"``.
    fn pattern_by_id(&self, py: Python<'_>, id: &str) -> Option<Py<PyAny>> {
        match id {
            "Focusable" => Py::new(py, PyFocusable { node: self.inner.clone() }).ok().map(|p| p.into_any()),
            "WindowSurface" => Py::new(py, PyWindowSurface { node: self.inner.clone() }).ok().map(|p| p.into_any()),
            _ => None,
        }
    }

    /// Returns ``True`` when the node advertises support for the given pattern id.
    fn has_pattern(&self, id: &str) -> bool {
        self.inner.supported_patterns().iter().any(|p| p.as_str() == id)
    }
}

// ---------------- Iterator for UiNode children ----------------

/// Iterator returned by ``UiNode.children()``.
///
/// Each iteration yields another ``UiNode`` instance.
#[pyclass(name = "NodeChildrenIterator", module = "platynui_native", unsendable)]
pub struct PyNodeChildrenIterator {
    iter: Option<Box<dyn Iterator<Item = Arc<dyn core_rs::ui::UiNode>> + Send + 'static>>,
}

#[pymethods]
impl PyNodeChildrenIterator {
    /// Part of the Python iterator protocol; returns ``self``.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next child node or ``None`` when the sequence is exhausted.
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

/// Iterator returned by ``UiNode.attributes()``.
///
/// Each iteration yields a ``UiAttribute`` bound to the originating node.
#[pyclass(name = "NodeAttributesIterator", module = "platynui_native", unsendable)]
pub struct PyNodeAttributesIterator {
    iter: Option<Box<dyn Iterator<Item = Arc<dyn core_rs::ui::UiAttribute>> + Send + 'static>>,
    owner: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyNodeAttributesIterator {
    /// Part of the Python iterator protocol; returns ``self``.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next ``UiAttribute`` or ``None`` when no attributes remain.
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

/// Iterator returned by :py:meth:`Runtime.evaluate_iter`.
///
/// Each iteration yields either a ``UiNode``, an ``EvaluatedAttribute`` or a
/// primitive value depending on the query.
#[pyclass(name = "EvaluationIterator", module = "platynui_native", unsendable)]
pub struct PyEvaluationIterator {
    iter: Option<Box<dyn Iterator<Item = runtime_rs::EvaluationItem>>>,
}

#[pymethods]
impl PyEvaluationIterator {
    /// Part of the Python iterator protocol; returns ``self``.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next evaluation result or ``None`` when the iterator is exhausted.
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

/// Provides focus-related actions for nodes that advertise the ``Focusable`` pattern.
#[pyclass(module = "platynui_native", name = "Focusable")]
pub struct PyFocusable {
    node: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyFocusable {
    /// Returns the pattern identifier ``"Focusable"``.
    fn id(&self) -> &'static str {
        "Focusable"
    }
    /// Requests focus for the associated node.
    fn focus(&self) -> PyResult<()> {
        if let Some(p) = self.node.pattern::<core_rs::ui::pattern::FocusableAction>() {
            p.focus().map_err(|e| PatternError::new_err(e.to_string()))
        } else {
            Err(PatternError::new_err("Focusable pattern not available"))
        }
    }
}

/// Exposes window-management operations for nodes with the ``WindowSurface`` pattern.
#[pyclass(module = "platynui_native", name = "WindowSurface")]
pub struct PyWindowSurface {
    node: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyWindowSurface {
    /// Returns the pattern identifier ``"WindowSurface"``.
    fn id(&self) -> &'static str {
        "WindowSurface"
    }

    /// Brings the window to the foreground and activates it.
    fn activate(&self) -> PyResult<()> {
        self.call(|p| p.activate())
    }
    /// Minimizes the window if the platform supports the action.
    fn minimize(&self) -> PyResult<()> {
        self.call(|p| p.minimize())
    }
    /// Maximizes the window if the platform supports the action.
    fn maximize(&self) -> PyResult<()> {
        self.call(|p| p.maximize())
    }
    /// Restores the window to its previous size/state after minimize or maximize.
    fn restore(&self) -> PyResult<()> {
        self.call(|p| p.restore())
    }
    /// Closes the window.
    fn close(&self) -> PyResult<()> {
        self.call(|p| p.close())
    }

    /// Moves the window's top-left corner to ``(x, y)`` screen coordinates.
    fn move_to(&self, x: f64, y: f64) -> PyResult<()> {
        self.call(|p| p.move_to(core_rs::types::Point::new(x, y)))
    }
    /// Resizes the window to ``width`` × ``height``.
    fn resize(&self, width: f64, height: f64) -> PyResult<()> {
        self.call(|p| p.resize(core_rs::types::Size::new(width, height)))
    }
    /// Moves and resizes the window in a single operation.
    fn move_and_resize(&self, x: f64, y: f64, width: f64, height: f64) -> PyResult<()> {
        self.call(|p| p.move_and_resize(core_rs::types::Rect::new(x, y, width, height)))
    }
    /// Returns whether the window is currently able to receive user input, if known.
    fn accepts_user_input(&self) -> PyResult<Option<bool>> {
        self.with_pattern(|p| p.accepts_user_input())
    }
}

// ---------------- UiAttribute wrapper ----------------

/// Represents a node attribute that can be resolved on demand.
#[pyclass(module = "platynui_native", name = "UiAttribute", subclass)]
pub struct PyAttribute {
    namespace: String,
    name: String,
    owner: Arc<dyn core_rs::ui::UiNode>,
}

#[pymethods]
impl PyAttribute {
    /// Returns the namespace label (for example ``"control"``).
    #[getter]
    fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Returns the attribute name within the namespace.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    /// Resolves the current attribute value.
    ///
    /// ``None`` is returned when the provider reports that the attribute no longer exists.
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ns = core_rs::ui::namespace::Namespace::from_str(self.namespace.as_str()).unwrap_or_default();
        match self.owner.attribute(ns, &self.name) {
            Some(attr) => ui_value_to_py(py, &attr.value()),
            None => Ok(py.None()),
        }
    }
    /// Returns a concise string representation useful for debugging.
    fn __repr__(&self) -> String {
        format!("Attribute(namespace='{}', name='{}')", self.namespace, self.name)
    }
}

/// Holds an attribute that was fully evaluated during an XPath query.
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
    /// Creates a new evaluated attribute with a pre-resolved value.
    fn new(namespace: String, name: String, value: Py<PyAny>, owner: Option<Py<PyNode>>) -> Self {
        Self { namespace, name, value, owner }
    }
    /// Returns the namespace label of the attribute.
    #[getter]
    fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Returns the attribute name within the namespace.
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }
    /// Returns the captured value.
    #[getter]
    fn value(&self, py: Python<'_>) -> Py<PyAny> {
        self.value.clone_ref(py)
    }
    /// Returns the originating node if it was provided during evaluation.
    fn owner(&self, py: Python<'_>) -> Option<Py<PyNode>> {
        self.owner.as_ref().map(|o| o.clone_ref(py))
    }
    /// Returns a concise string representation useful for debugging.
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
        Err(PatternError::new_err("WindowSurface pattern not available"))
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

/// High-level automation runtime for exploring UI trees and driving input devices.
///
/// A :class:`Runtime` instance is responsible for
///
/// - discovering the available platform providers (Windows UIA, AT-SPI, mock, …),
/// - evaluating XPath queries into :class:`UiNode` objects or primitive values,
/// - exposing helper APIs for pointer and keyboard input, and
/// - exposing utilities such as focus, highlight overlays, and screenshots.
///
/// The binding offers an ergonomic API: call :py:meth:`evaluate` / :py:meth:`evaluate_single`
/// to obtain :class:`UiNode` objects, inspect their attributes, then invoke pointer or keyboard
/// actions using regular tuples or small helper classes (:class:`Point`, :class:`PointerOverrides`, …).
///
/// Example::
///
///     from platynui_native import Runtime
///
///     rt = Runtime()
///     button = rt.evaluate_single("//Button[@Name='Sign in']")
///     if button:
///         rt.bring_to_front(button)
///         rt.pointer_click()
///
#[pyclass(name = "Runtime", module = "platynui_native")]
pub struct PyRuntime {
    inner: runtime_rs::Runtime,
    cache_id: u64,
}

#[pymethods]
impl PyRuntime {
    #[new]
    /// Creates a runtime that discovers platform providers automatically.
    fn new() -> PyResult<Self> {
        runtime_rs::Runtime::new()
            .map(|inner| Self { inner, cache_id: NEXT_CACHE_ID.fetch_add(1, Ordering::Relaxed) })
            .map_err(map_provider_err)
    }

    // ---------------- Static builder (mock only) ----------------

    /// Creates a runtime that talks to the bundled mock provider and devices.
    ///
    /// Available only when the native extension is compiled with the
    /// ``mock-provider`` feature. Useful for unit testing on any host.
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
                .map(|inner| Self { inner, cache_id: NEXT_CACHE_ID.fetch_add(1, Ordering::Relaxed) })
                .map_err(map_provider_err);
        }
        #[cfg(not(feature = "mock-provider"))]
        {
            Err(ProviderError::new_err("Runtime.new_with_mock() requires building with feature 'mock-provider'"))
        }
    }

    /// Evaluates an XPath expression and returns all matching results as a list.
    ///
    /// Items are converted into ``UiNode`` instances, ``EvaluatedAttribute``
    /// objects, or native Python values (``None``, ``bool``, ``int``, ``float``,
    /// ``str``, ``list``, ``dict``, :class:`Point`, :class:`Size`, :class:`Rect`).
    /// ``node`` restricts the search to the subtree when provided.
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
        let items = with_cache(self.cache_id, |cache| {
            self.inner.evaluate_cached(node_arc, xpath, cache)
        })
        .map_err(map_eval_err)?;
        let out = PyList::empty(py);
        for item in items {
            out.append(evaluation_item_to_py(py, &item)?)?;
        }
        Ok(out.into())
    }

    /// Evaluates an XPath expression and returns the first match.
    ///
    /// ``None`` is returned when the query produced no results. Items are converted
    /// in the same way as :py:meth:`Runtime.evaluate`.
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

        let item = with_cache(self.cache_id, |cache| {
            self.inner.evaluate_single_cached(node_arc, xpath, cache)
        })
        .map_err(map_eval_err)?;

        match item {
            Some(it) => evaluation_item_to_py(py, &it),
            None => Ok(py.None()),
        }
    }

    /// Immediately releases provider resources and device handles.
    ///
    /// Calling this ensures deterministic cleanup; otherwise the runtime will
    /// dispose its resources later when Python garbage collection drops the
    /// last reference.
    fn shutdown(&mut self) {
        self.inner.shutdown();
    }

    fn clear_cache(&self) {
        CACHE_MAP.with(|map| {
            if let Some(cache) = map.borrow().get(&self.cache_id) {
                cache.clear();
            }
        });
    }

    /// Evaluates an XPath expression and returns a lazy iterator over the results.
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

        let stream = with_cache(self.cache_id, |cache| {
            self.inner.evaluate_iter_owned_cached(node_arc, xpath, cache)
        })
        .map_err(map_eval_err)?;
        Py::new(py, PyEvaluationIterator { iter: Some(Box::new(stream)) })
    }

    /// Returns a list of dictionaries describing the active providers.
    ///
    /// Each dictionary exposes the provider ``id``, human-readable ``display_name``,
    /// associated ``technology`` identifier, and the provider ``kind``.
    fn providers(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let list = PyList::empty(py);
        for provider in self.inner.providers() {
            let desc = provider.descriptor();
            let dict = PyDict::new(py);
            dict.set_item("id", desc.id)?;
            dict.set_item("display_name", desc.display_name)?;
            dict.set_item("technology", desc.technology.as_str())?;
            let kind = match desc.kind {
                core_rs::provider::ProviderKind::Native => "Native",
                core_rs::provider::ProviderKind::External => "External",
            };
            dict.set_item("kind", kind)?;
            list.append(dict)?;
        }
        Ok(list.unbind())
    }

    /// Returns the current pointer defaults as a :class:`PointerSettings` instance.
    #[pyo3(text_signature = "(self)")]
    fn pointer_settings(&self, py: Python<'_>) -> PyResult<Py<PyPointerSettings>> {
        Py::new(py, PyPointerSettings::from(self.inner.pointer_settings()))
    }

    /// Replaces the pointer defaults that future actions will use.
    #[pyo3(signature = (settings), text_signature = "(self, settings)")]
    fn set_pointer_settings(&self, settings: PointerSettingsLike) -> PyResult<()> {
        self.inner.set_pointer_settings(settings.into());
        Ok(())
    }

    /// Returns the active pointer movement profile as :class:`PointerProfile`.
    #[pyo3(text_signature = "(self)")]
    fn pointer_profile(&self, py: Python<'_>) -> PyResult<Py<PyPointerProfile>> {
        Py::new(py, PyPointerProfile::from(self.inner.pointer_profile()))
    }

    /// Sets the pointer movement profile that subsequent pointer operations will use.
    #[pyo3(signature = (profile), text_signature = "(self, profile)")]
    fn set_pointer_profile(&self, profile: PointerProfileLike) -> PyResult<()> {
        self.inner.set_pointer_profile(profile.into());
        Ok(())
    }

    /// Returns the keyboard timing defaults as :class:`KeyboardSettings`.
    #[pyo3(text_signature = "(self)")]
    fn keyboard_settings(&self, py: Python<'_>) -> PyResult<Py<PyKeyboardSettings>> {
        Py::new(py, PyKeyboardSettings::from(self.inner.keyboard_settings()))
    }

    /// Replaces the keyboard timing defaults for subsequent keyboard input.
    #[pyo3(signature = (settings), text_signature = "(self, settings)")]
    fn set_keyboard_settings(&self, settings: KeyboardSettingsLike) -> PyResult<()> {
        self.inner.set_keyboard_settings(settings.into());
        Ok(())
    }

    /// Returns the top-level window that contains ``node``.
    #[pyo3(signature = (node), text_signature = "(self, node)")]
    fn top_level_window_for(&self, py: Python<'_>, node: PyRef<'_, PyNode>) -> PyResult<Option<Py<PyNode>>> {
        match self.inner.top_level_window_for(&node.inner) {
            Some(window) => Ok(Some(Py::new(py, PyNode { inner: window })?)),
            None => Ok(None),
        }
    }

    // ---------------- Pointer minimal API ----------------

    /// Returns the current pointer position as a :class:`Point`.
    #[pyo3(text_signature = "(self)")]
    fn pointer_position(&self, py: Python<'_>) -> PyResult<Py<PyPoint>> {
        let p = self.inner.pointer_position().map_err(map_pointer_err)?;
        Py::new(py, PyPoint::from(p))
    }

    /// Moves the pointer to ``point`` and returns the final position.
    #[pyo3(signature = (point, overrides=None), text_signature = "(self, point, overrides=None)")]
    fn pointer_move_to(
        &self,
        py: Python<'_>,
        point: PointInput,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<Py<PyPoint>> {
        let p: core_rs::types::Point = point.0;
        let ov = overrides.map(Into::into);
        let new_pos = self.inner.pointer_move_to(p, ov).map_err(map_pointer_err)?;
        Py::new(py, PyPoint::from(new_pos))
    }

    /// Performs a single click.
    ///
    /// ``point`` defaults to the current pointer location. ``button`` selects the
    /// button to use and ``overrides`` customises timing for this call.
    #[pyo3(signature = (point, button=None, overrides=None), text_signature = "(self, point, button=None, overrides=None)")]
    fn pointer_click(
        &self,
        point: Option<PointInput>,
        button: Option<PointerButtonLike>,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<()> {
        let p: Option<core_rs::types::Point> = point.map(|r| r.0);
        let btn = button.map(|b| b.into());
        let ov = overrides.map(Into::into);
        self.inner.pointer_click(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Performs ``clicks`` consecutive clicks at ``point``.
    #[pyo3(signature = (point=None, clicks=2, button=None, overrides=None), text_signature = "(self, point=None, clicks=2, button=None, overrides=None)")]
    fn pointer_multi_click(
        &self,
        point: Option<PointInput>,
        clicks: u32,
        button: Option<PointerButtonLike>,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<()> {
        let p: Option<core_rs::types::Point> = point.map(|r| r.0);
        let btn = button.map(|b| b.into());
        let ov = overrides.map(Into::into);
        self.inner.pointer_multi_click(p, btn, clicks, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Performs a drag gesture from ``start`` to ``end``.
    #[pyo3(signature = (start, end, button=None, overrides=None), text_signature = "(self, start, end, button=None, overrides=None)")]
    fn pointer_drag(
        &self,
        start: PointInput,
        end: PointInput,
        button: Option<PointerButtonLike>,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<()> {
        let s: core_rs::types::Point = start.0;
        let e: core_rs::types::Point = end.0;
        let btn = button.map(|b| b.into());
        let ov = overrides.map(Into::into);
        self.inner.pointer_drag(s, e, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Presses the selected pointer button, optionally moving first.
    #[pyo3(signature = (point=None, button=None, overrides=None), text_signature = "(self, point=None, button=None, overrides=None)")]
    fn pointer_press(
        &self,
        point: Option<PointInput>,
        button: Option<PointerButtonLike>,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<()> {
        let p = point.map(|r| r.0);
        let btn = button.map(|b| b.into());
        let ov = overrides.map(Into::into);
        self.inner.pointer_press(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Releases the selected pointer button, optionally moving first.
    #[pyo3(signature = (point=None, button=None, overrides=None), text_signature = "(self, point=None, button=None, overrides=None)")]
    fn pointer_release(
        &self,
        point: Option<PointInput>,
        button: Option<PointerButtonLike>,
        overrides: Option<PointerOverridesLike>,
    ) -> PyResult<()> {
        let p: Option<core_rs::types::Point> = point.map(|r| r.0);
        let btn = button.map(|b| b.into());
        let ov = overrides.map(Into::into);
        self.inner.pointer_release(p, btn, ov).map_err(map_pointer_err)?;
        Ok(())
    }

    /// Scrolls by the horizontal/vertical deltas specified in ``delta``.
    #[pyo3(signature = (delta, overrides=None), text_signature = "(self, delta, overrides=None)")]
    fn pointer_scroll(&self, delta: ScrollLike, overrides: Option<PointerOverridesLike>) -> PyResult<()> {
        let (h, v) = match delta {
            ScrollLike::Tuple((h, v)) => (h, v),
        };
        let ov = overrides.map(Into::into);
        self.inner.pointer_scroll(core_rs::platform::ScrollDelta::new(h, v), ov).map_err(map_pointer_err)?;
        Ok(())
    }

    // ---------------- Keyboard minimal API ----------------

    /// Types the provided sequence using the runtime keyboard DSL.
    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_type(&self, sequence: &str, overrides: Option<KeyboardOverridesLike>) -> PyResult<()> {
        let ov = overrides.map(Into::into);
        self.inner.keyboard_type(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    /// Presses all keys from ``sequence`` without releasing them.
    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_press(&self, sequence: &str, overrides: Option<KeyboardOverridesLike>) -> PyResult<()> {
        let ov = overrides.map(Into::into);
        self.inner.keyboard_press(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    /// Releases the keys listed in ``sequence``.
    #[pyo3(signature = (sequence, overrides=None), text_signature = "(self, sequence, overrides=None)")]
    fn keyboard_release(&self, sequence: &str, overrides: Option<KeyboardOverridesLike>) -> PyResult<()> {
        let ov = overrides.map(Into::into);
        self.inner.keyboard_release(sequence, ov).map_err(map_keyboard_err)?;
        Ok(())
    }

    /// Returns the list of key names recognised by the active keyboard device.
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

    /// Returns a dictionary describing the desktop (bounds, monitors, platform names).
    fn desktop_info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let info = self.inner.desktop_info();
        desktop_info_to_py(py, info)
    }

    /// Sets focus to ``node``.
    fn focus(&self, node: PyRef<'_, PyNode>) -> PyResult<()> {
        self.inner.focus(&node.inner).map_err(map_focus_err)?;
        Ok(())
    }

    /// Brings the window associated with ``node`` to the foreground.
    ///
    /// When ``wait_ms`` is provided the call waits up to that many milliseconds
    /// for the window to become input ready if the platform reports readiness.
    #[pyo3(signature = (node, wait_ms=None), text_signature = "(self, node, wait_ms=None)")]
    fn bring_to_front(&self, node: PyRef<'_, PyNode>, wait_ms: Option<f64>) -> PyResult<()> {
        match wait_ms {
            Some(ms) => {
                let dur = std::time::Duration::from_millis(ms.max(0.0) as u64);
                self.inner.bring_to_front_and_wait(&node.inner, dur).map_err(map_bring_err)?;
            }
            None => {
                self.inner.bring_to_front(&node.inner).map_err(map_bring_err)?;
            }
        }
        Ok(())
    }

    // ---------------- Highlight & Screenshot ----------------

    /// Highlights one or more rectangles for ``duration_ms`` milliseconds.
    ///
    /// ``rects`` may be a single :class:`Rect` or any iterable producing rectangles.
    #[pyo3(signature = (rects, duration_ms=None), text_signature = "(self, rects, duration_ms=None)")]
    fn highlight(&self, rects: Bound<'_, PyAny>, duration_ms: Option<f64>) -> PyResult<()> {
        let mut all: Vec<platynui_core::types::Rect> = Vec::new();
        // Fast path: single Rect passed directly
        if let Ok(inp) = rects.extract::<RectInput>() {
            all.push(inp.0);
        } else {
            // Fallback: consume any iterable of Rects
            let iter = PyIterator::from_object(&rects)?;
            for item in iter {
                let any = item?;
                if let Ok(inp) = any.extract::<RectInput>() {
                    all.push(inp.0);
                } else {
                    let r: PyRef<PyRect> = any.extract()?;
                    all.push(r.as_inner());
                }
            }
        }
        let mut req = HighlightRequest::from_rects(all);
        if let Some(ms) = duration_ms {
            req = req.with_duration(std::time::Duration::from_millis(ms as u64));
        }
        self.inner.highlight(&req).map_err(map_platform_err)?;
        Ok(())
    }

    /// Clears a previously shown highlight overlay when supported by the platform.
    fn clear_highlight(&self) -> PyResult<()> {
        self.inner.clear_highlight().map_err(map_platform_err)?;
        Ok(())
    }

    /// Captures a screenshot and returns the encoded image bytes.
    ///
    /// ``rect`` limits the capture area; ``mime_type`` currently accepts only
    /// ``"image/png"``.
    #[pyo3(signature = (rect=None, mime_type=None), text_signature = "(self, rect=None, mime_type=None)")]
    fn screenshot(&self, py: Python<'_>, rect: Option<RectInput>, mime_type: Option<&str>) -> PyResult<Py<PyAny>> {
        let effective_mime = mime_type.unwrap_or("image/png");
        if !effective_mime.eq_ignore_ascii_case("image/png") {
            return Err(PyTypeError::new_err("unsupported mime_type; only 'image/png' is supported"));
        }
        let request =
            rect.map(|r| ScreenshotRequest::with_region(r.0)).unwrap_or_else(ScreenshotRequest::entire_display);
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

fn map_bring_err(err: runtime_rs::runtime::BringToFrontError) -> PyErr {
    // Reuse PatternError for simplicity; include the runtime id and message
    PatternError::new_err(err.to_string())
}

// ---------------- Internal helpers ----------------

fn pattern_object(py: Python<'_>, node: &Arc<dyn core_rs::ui::UiNode>, id: &str) -> Option<Py<PyAny>> {
    match id {
        "Focusable" => Py::new(py, PyFocusable { node: node.clone() }).ok().map(|p| p.into_any()),
        "WindowSurface" => Py::new(py, PyWindowSurface { node: node.clone() }).ok().map(|p| p.into_any()),
        _ => None,
    }
}

// ---------------- Module init ----------------

// ---------------- Exceptions ----------------

// Base error for all PlatynUI-related exceptions
pyo3::create_exception!(runtime, PlatynUiError, PyException);
// Specific errors deriving from PlatynUiError for finer-grained handling
pyo3::create_exception!(runtime, EvaluationError, PlatynUiError);
pyo3::create_exception!(runtime, ProviderError, PlatynUiError);
pyo3::create_exception!(runtime, PointerError, PlatynUiError);
pyo3::create_exception!(runtime, KeyboardError, PlatynUiError);
pyo3::create_exception!(runtime, PatternError, PlatynUiError);
pyo3::create_exception!(runtime, AttributeNotFoundError, PlatynUiError);

/// Register all runtime types and functions directly into the module (no submodule).
pub fn register_types(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRuntime>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyNodeChildrenIterator>()?;
    m.add_class::<PyNodeAttributesIterator>()?;
    m.add_class::<PyEvaluationIterator>()?;
    m.add_class::<PyAttribute>()?;
    m.add_class::<PyEvaluatedAttribute>()?;
    m.add_class::<PyFocusable>()?;
    m.add_class::<PyWindowSurface>()?;
    m.add_class::<PyPointerOverrides>()?;
    m.add_class::<PyPointerSettings>()?;
    m.add_class::<PyPointerProfile>()?;
    m.add_class::<PyKeyboardOverrides>()?;
    m.add_class::<PyKeyboardSettings>()?;
    // Pointer motion mode enum (IntEnum)
    {
        let enum_mod = PyModule::import(py, "enum")?;
        let int_enum = enum_mod.getattr("IntEnum")?;
        let dict = PyDict::new(py);
        dict.set_item("DIRECT", 0)?;
        dict.set_item("LINEAR", 1)?;
        dict.set_item("BEZIER", 2)?;
        dict.set_item("OVERSHOOT", 3)?;
        dict.set_item("JITTER", 4)?;
        let args = ("PointerMotionMode", dict);
        let py_enum = int_enum.call1(args)?;
        m.add("PointerMotionMode", py_enum)?;
    }
    {
        let enum_mod = PyModule::import(py, "enum")?;
        let int_enum = enum_mod.getattr("IntEnum")?;
        let dict = PyDict::new(py);
        dict.set_item("CONSTANT", 0)?;
        dict.set_item("EASE_IN", 1)?;
        dict.set_item("EASE_OUT", 2)?;
        dict.set_item("SMOOTH_STEP", 3)?;
        let args = ("PointerAccelerationProfile", dict);
        let py_enum = int_enum.call1(args)?;
        m.add("PointerAccelerationProfile", py_enum)?;
    }
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
    m.add("PlatynUiError", py.get_type::<PlatynUiError>())?;
    m.add("AttributeNotFoundError", py.get_type::<AttributeNotFoundError>())?;

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

// -------- Like helpers for Point/Rect (tuple/list/dict/instances) --------

fn duration_from_millis(ms: f64) -> std::time::Duration {
    std::time::Duration::from_millis(ms.max(0.0) as u64)
}

fn duration_from_micros(us: f64) -> std::time::Duration {
    std::time::Duration::from_micros(us.max(0.0) as u64)
}

fn pointer_button_to_int(button: core_rs::platform::PointerButton) -> u16 {
    match button {
        core_rs::platform::PointerButton::Left => 1,
        core_rs::platform::PointerButton::Middle => 2,
        core_rs::platform::PointerButton::Right => 3,
        core_rs::platform::PointerButton::Other(code) => code,
    }
}

fn pointer_motion_mode_from_int(value: i32) -> Option<core_rs::platform::PointerMotionMode> {
    match value {
        0 => Some(core_rs::platform::PointerMotionMode::Direct),
        1 => Some(core_rs::platform::PointerMotionMode::Linear),
        2 => Some(core_rs::platform::PointerMotionMode::Bezier),
        3 => Some(core_rs::platform::PointerMotionMode::Overshoot),
        4 => Some(core_rs::platform::PointerMotionMode::Jitter),
        _ => None,
    }
}

fn pointer_motion_mode_to_str(mode: core_rs::platform::PointerMotionMode) -> &'static str {
    match mode {
        core_rs::platform::PointerMotionMode::Direct => "direct",
        core_rs::platform::PointerMotionMode::Linear => "linear",
        core_rs::platform::PointerMotionMode::Bezier => "bezier",
        core_rs::platform::PointerMotionMode::Overshoot => "overshoot",
        core_rs::platform::PointerMotionMode::Jitter => "jitter",
    }
}

fn pointer_motion_mode_to_int(mode: core_rs::platform::PointerMotionMode) -> i32 {
    match mode {
        core_rs::platform::PointerMotionMode::Direct => 0,
        core_rs::platform::PointerMotionMode::Linear => 1,
        core_rs::platform::PointerMotionMode::Bezier => 2,
        core_rs::platform::PointerMotionMode::Overshoot => 3,
        core_rs::platform::PointerMotionMode::Jitter => 4,
    }
}

fn pointer_motion_mode_to_py(py: Python<'_>, mode: core_rs::platform::PointerMotionMode) -> PyResult<Py<PyAny>> {
    let module = PyModule::import(py, "platynui_native")?;
    let enum_cls = module.getattr("PointerMotionMode")?;
    let value = pointer_motion_mode_to_int(mode);
    Ok(enum_cls.call1((value,))?.unbind().into_any())
}

fn pointer_acceleration_from_int(value: i32) -> Option<core_rs::platform::PointerAccelerationProfile> {
    match value {
        0 => Some(core_rs::platform::PointerAccelerationProfile::Constant),
        1 => Some(core_rs::platform::PointerAccelerationProfile::EaseIn),
        2 => Some(core_rs::platform::PointerAccelerationProfile::EaseOut),
        3 => Some(core_rs::platform::PointerAccelerationProfile::SmoothStep),
        _ => None,
    }
}

fn pointer_acceleration_to_int(profile: core_rs::platform::PointerAccelerationProfile) -> i32 {
    match profile {
        core_rs::platform::PointerAccelerationProfile::Constant => 0,
        core_rs::platform::PointerAccelerationProfile::EaseIn => 1,
        core_rs::platform::PointerAccelerationProfile::EaseOut => 2,
        core_rs::platform::PointerAccelerationProfile::SmoothStep => 3,
    }
}

fn pointer_acceleration_to_py(
    py: Python<'_>,
    profile: core_rs::platform::PointerAccelerationProfile,
) -> PyResult<Py<PyAny>> {
    let module = PyModule::import(py, "platynui_native")?;
    let enum_cls = module.getattr("PointerAccelerationProfile")?;
    let value = pointer_acceleration_to_int(profile);
    Ok(enum_cls.call1((value,))?.unbind().into_any())
}

fn ci_get<'py>(d: &Bound<'py, PyDict>, k: &str) -> Option<Bound<'py, PyAny>> {
    if let Some(v) = dict_get(d, k) {
        return Some(v);
    }
    let k2 = k.to_ascii_uppercase();
    dict_get(d, k2.as_str())
}

pub struct PointInput(pub core_rs::types::Point);

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for PointInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(p) = ob.extract::<PyRef<PyPoint>>() {
            return Ok(PointInput(p.as_inner()));
        }
        if let Ok((x, y)) = ob.extract::<(f64, f64)>() {
            return Ok(PointInput(core_rs::types::Point::new(x, y)));
        }
        if let Ok(seq) = ob.extract::<Vec<f64>>()
            && seq.len() == 2
        {
            return Ok(PointInput(core_rs::types::Point::new(seq[0], seq[1])));
        }
        if let Ok(d) = ob.cast::<PyDict>() {
            let x = ci_get(&d, "x").and_then(|v| v.extract::<f64>().ok());
            let y = ci_get(&d, "y").and_then(|v| v.extract::<f64>().ok());
            if let (Some(x), Some(y)) = (x, y) {
                return Ok(PointInput(core_rs::types::Point::new(x, y)));
            }
        }
        Err(PyTypeError::new_err("invalid point: expected Point | (x, y) | {x, y}"))
    }
}

pub struct RectInput(pub core_rs::types::Rect);

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for RectInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(r) = ob.extract::<PyRef<PyRect>>() {
            return Ok(RectInput(r.as_inner()));
        }
        if let Ok((x, y, w, h)) = ob.extract::<(f64, f64, f64, f64)>() {
            return Ok(RectInput(core_rs::types::Rect::new(x, y, w, h)));
        }
        if let Ok(seq) = ob.extract::<Vec<f64>>()
            && seq.len() == 4
        {
            return Ok(RectInput(core_rs::types::Rect::new(seq[0], seq[1], seq[2], seq[3])));
        }
        if let Ok(d) = ob.cast::<PyDict>() {
            let x = ci_get(&d, "x").and_then(|v| v.extract::<f64>().ok());
            let y = ci_get(&d, "y").and_then(|v| v.extract::<f64>().ok());
            let w = ci_get(&d, "width").and_then(|v| v.extract::<f64>().ok());
            let h = ci_get(&d, "height").and_then(|v| v.extract::<f64>().ok());
            if let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) {
                return Ok(RectInput(core_rs::types::Rect::new(x, y, w, h)));
            }
        }
        Err(PyTypeError::new_err("invalid rect: expected Rect | (x, y, w, h) | {x, y, width, height}"))
    }
}

pub struct SizeInput(pub core_rs::types::Size);

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for SizeInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(s) = ob.extract::<PyRef<PySize>>() {
            return Ok(SizeInput(s.as_inner()));
        }
        if let Ok((w, h)) = ob.extract::<(f64, f64)>() {
            return Ok(SizeInput(core_rs::types::Size::new(w, h)));
        }
        if let Ok(seq) = ob.extract::<Vec<f64>>()
            && seq.len() == 2
        {
            return Ok(SizeInput(core_rs::types::Size::new(seq[0], seq[1])));
        }
        if let Ok(d) = ob.cast::<PyDict>() {
            let w = ci_get(&d, "width").or_else(|| ci_get(&d, "w")).and_then(|v| v.extract::<f64>().ok());
            let h = ci_get(&d, "height").or_else(|| ci_get(&d, "h")).and_then(|v| v.extract::<f64>().ok());
            if let (Some(w), Some(h)) = (w, h) {
                return Ok(SizeInput(core_rs::types::Size::new(w, h)));
            }
        }
        Err(PyTypeError::new_err("invalid size: expected Size | (width, height) | {width, height}"))
    }
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

#[derive(Clone, Copy)]
pub struct PointerMotionModeInput(pub core_rs::platform::PointerMotionMode);

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for PointerMotionModeInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(value) = ob.extract::<i32>() {
            return pointer_motion_mode_from_int(value)
                .map(PointerMotionModeInput)
                .ok_or_else(|| PyTypeError::new_err(format!("unknown pointer motion mode value {value}")));
        }

        if let Ok(attr) = ob.getattr("value")
            && let Ok(value) = attr.extract::<i32>()
        {
            return pointer_motion_mode_from_int(value)
                .map(PointerMotionModeInput)
                .ok_or_else(|| PyTypeError::new_err(format!("unknown pointer motion mode value {value}")));
        }

        Err(PyTypeError::new_err("pointer motion mode must be PointerMotionMode enum value"))
    }
}

impl From<PointerMotionModeInput> for core_rs::platform::PointerMotionMode {
    fn from(value: PointerMotionModeInput) -> Self {
        value.0
    }
}

#[derive(Clone, Copy)]
pub struct PointerAccelerationInput(pub core_rs::platform::PointerAccelerationProfile);

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for PointerAccelerationInput {
    type Error = PyErr;

    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if let Ok(value) = ob.extract::<i32>() {
            return pointer_acceleration_from_int(value)
                .map(PointerAccelerationInput)
                .ok_or_else(|| PyTypeError::new_err(format!("unknown pointer acceleration value {value}")));
        }

        if let Ok(attr) = ob.getattr("value")
            && let Ok(value) = attr.extract::<i32>()
        {
            return pointer_acceleration_from_int(value)
                .map(PointerAccelerationInput)
                .ok_or_else(|| PyTypeError::new_err(format!("unknown pointer acceleration value {value}")));
        }

        Err(PyTypeError::new_err("pointer acceleration profile must be PointerAccelerationProfile enum value"))
    }
}

impl From<PointerAccelerationInput> for core_rs::platform::PointerAccelerationProfile {
    fn from(value: PointerAccelerationInput) -> Self {
        value.0
    }
}

impl Drop for PyRuntime {
    fn drop(&mut self) {
        remove_cache(self.cache_id);
    }
}

// ---------------- Concrete overrides classes (Python-visible) ----------------

/// Per-action pointer overrides used to adjust movement, timing, and behaviour.
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
        motion=None,
        steps_per_pixel=None,
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
        overshoot_ratio=None,
        overshoot_settle_steps=None,
        curve_amplitude=None,
        jitter_amplitude=None,
        ensure_move_position=None,
        ensure_move_threshold=None,
        ensure_move_timeout_ms=None,
        scroll_step=None,
        scroll_delay_ms=None,
    ))]
    /// Creates overrides that can be passed to pointer-related runtime calls.
    fn new(
        origin: Option<OriginInput>,
        motion: Option<PointerMotionModeInput>,
        steps_per_pixel: Option<f64>,
        speed_factor: Option<f64>,
        acceleration_profile: Option<PointerAccelerationInput>,
        max_move_duration_ms: Option<f64>,
        move_time_per_pixel_us: Option<f64>,
        after_move_delay_ms: Option<f64>,
        after_input_delay_ms: Option<f64>,
        press_release_delay_ms: Option<f64>,
        after_click_delay_ms: Option<f64>,
        before_next_click_delay_ms: Option<f64>,
        multi_click_delay_ms: Option<f64>,
        overshoot_ratio: Option<f64>,
        overshoot_settle_steps: Option<u32>,
        curve_amplitude: Option<f64>,
        jitter_amplitude: Option<f64>,
        ensure_move_position: Option<bool>,
        ensure_move_threshold: Option<f64>,
        ensure_move_timeout_ms: Option<f64>,
        scroll_step: Option<(f64, f64)>,
        scroll_delay_ms: Option<f64>,
    ) -> Self {
        let input = PointerOverridesInput {
            origin,
            motion,
            steps_per_pixel,
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
            overshoot_ratio,
            overshoot_settle_steps,
            curve_amplitude,
            jitter_amplitude,
            ensure_move_position,
            ensure_move_threshold,
            ensure_move_timeout_ms,
            scroll_step,
            scroll_delay_ms,
        };
        Self { inner: input.into() }
    }

    /// Returns a readable representation for debugging.
    fn __repr__(&self) -> String {
        "PointerOverrides(...)".to_string()
    }

    #[classmethod]
    /// Accepts any object recognised by the ``PointerOverridesLike`` helper and creates overrides.
    fn from_like(_cls: &Bound<'_, PyType>, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let like = value.extract::<PointerOverridesLike>()?;
        Ok(Self { inner: like.into() })
    }

    // ----- getters (read-only properties) -----
    #[getter]
    /// Returns the origin reference used when interpreting pointer coordinates.
    fn origin(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        use core_rs::platform::PointOrigin as O;
        self.inner.origin.as_ref().and_then(|o| match o {
            O::Desktop => "desktop".into_pyobject(py).ok().map(|v| v.unbind().into_any()),
            O::Absolute(p) => Py::new(py, PyPoint::from(*p)).ok().map(|v| v.into_any()),
            O::Bounds(r) => Py::new(py, PyRect::from(*r)).ok().map(|v| v.into_any()),
        })
    }
    #[getter]
    /// Returns the motion profile used for pointer moves when provided.
    fn motion(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner.motion_mode.map(|m| pointer_motion_mode_to_py(py, m)).transpose()
    }
    #[getter]
    /// Returns the number of motion steps performed per pixel moved.
    fn steps_per_pixel(&self) -> Option<f64> {
        self.inner.steps_per_pixel
    }
    #[getter]
    /// Returns the multiplier applied to pointer speed.
    fn speed_factor(&self) -> Option<f64> {
        self.inner.speed_factor
    }
    #[getter]
    /// Returns the acceleration profile used for pointer moves when set.
    fn acceleration_profile(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.inner.acceleration_profile.map(|p| pointer_acceleration_to_py(py, p)).transpose()
    }
    #[getter]
    /// Returns the maximum duration, in milliseconds, of a pointer move.
    fn max_move_duration_ms(&self) -> Option<f64> {
        self.inner.max_move_duration.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the microseconds spent per pixel when timing is based on distance.
    fn move_time_per_pixel_us(&self) -> Option<f64> {
        self.inner.move_time_per_pixel.map(|d| d.as_micros() as f64)
    }
    #[getter]
    /// Returns the delay after completing a pointer move in milliseconds.
    fn after_move_delay_ms(&self) -> Option<f64> {
        self.inner.after_move_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the delay applied after any pointer input action in milliseconds.
    fn after_input_delay_ms(&self) -> Option<f64> {
        self.inner.after_input_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the press/release delay in milliseconds when both happen together.
    fn press_release_delay_ms(&self) -> Option<f64> {
        self.inner.press_release_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the pause after completing a click, in milliseconds.
    fn after_click_delay_ms(&self) -> Option<f64> {
        self.inner.after_click_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the pause inserted before moving on to the next click.
    fn before_next_click_delay_ms(&self) -> Option<f64> {
        self.inner.before_next_click_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the time window used to group a sequence of clicks.
    fn multi_click_delay_ms(&self) -> Option<f64> {
        self.inner.multi_click_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the overshoot ratio applied to pointer moves.
    fn overshoot_ratio(&self) -> Option<f64> {
        self.inner.overshoot_ratio
    }
    #[getter]
    /// Returns the number of easing steps used to settle after overshooting.
    fn overshoot_settle_steps(&self) -> Option<u32> {
        self.inner.overshoot_settle_steps
    }
    #[getter]
    /// Returns the amplitude for curve-based motion shapes.
    fn curve_amplitude(&self) -> Option<f64> {
        self.inner.curve_amplitude
    }
    #[getter]
    /// Returns the amplitude of random jitter applied during motion.
    fn jitter_amplitude(&self) -> Option<f64> {
        self.inner.jitter_amplitude
    }
    #[getter]
    /// Returns whether the runtime verifies that pointer moves end at the requested position.
    fn ensure_move_position(&self) -> Option<bool> {
        self.inner.ensure_move_position
    }
    #[getter]
    /// Returns the acceptable error threshold, in pixels, when verifying pointer moves.
    fn ensure_move_threshold(&self) -> Option<f64> {
        self.inner.ensure_move_threshold
    }
    #[getter]
    /// Returns the timeout used when verifying pointer moves.
    fn ensure_move_timeout_ms(&self) -> Option<f64> {
        self.inner.ensure_move_timeout.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the scroll delta that is applied per scroll step when set.
    fn scroll_step(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.inner
            .scroll_step
            .and_then(|d| PyTuple::new(py, [d.horizontal, d.vertical]).ok().map(|t| t.unbind().into_any()))
    }
    #[getter]
    /// Returns the delay between scroll steps in milliseconds.
    fn scroll_delay_ms(&self) -> Option<f64> {
        self.inner.scroll_delay.map(|d| d.as_millis() as f64)
    }
}

/// Temporary keyboard timing overrides applied to individual actions.
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
    /// Creates overrides that can be supplied to keyboard-related runtime calls.
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

    /// Returns a readable representation for debugging.
    fn __repr__(&self) -> String {
        "KeyboardOverrides(...)".to_string()
    }

    #[classmethod]
    /// Accepts any object recognised by ``KeyboardOverridesLike`` and creates overrides.
    fn from_like(_cls: &Bound<'_, PyType>, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let like = value.extract::<KeyboardOverridesLike>()?;
        Ok(Self { inner: like.into() })
    }

    // ----- getters (read-only properties) -----
    #[getter]
    /// Returns the delay between key press and release.
    fn press_delay_ms(&self) -> Option<f64> {
        self.inner.press_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the delay after releasing a key.
    fn release_delay_ms(&self) -> Option<f64> {
        self.inner.release_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the delay inserted between consecutive keys.
    fn between_keys_delay_ms(&self) -> Option<f64> {
        self.inner.between_keys_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the delay between pressing keys in a chord.
    fn chord_press_delay_ms(&self) -> Option<f64> {
        self.inner.chord_press_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the delay between releasing keys in a chord.
    fn chord_release_delay_ms(&self) -> Option<f64> {
        self.inner.chord_release_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the pause applied after finishing a sequence.
    fn after_sequence_delay_ms(&self) -> Option<f64> {
        self.inner.after_sequence_delay.map(|d| d.as_millis() as f64)
    }
    #[getter]
    /// Returns the pause applied after text input operations.
    fn after_text_delay_ms(&self) -> Option<f64> {
        self.inner.after_text_delay.map(|d| d.as_millis() as f64)
    }
}

/// Persistent pointer defaults fetched from or applied to the active runtime.
#[pyclass(module = "platynui_native", name = "PointerSettings")]
pub struct PyPointerSettings {
    pub(crate) inner: runtime_rs::PointerSettings,
}

#[pymethods]
impl PyPointerSettings {
    #[new]
    #[pyo3(signature = (*, double_click_time_ms=None, double_click_size=None, default_button=None))]
    /// Creates a settings object that can be applied to the runtime.
    fn new(
        double_click_time_ms: Option<f64>,
        double_click_size: Option<SizeInput>,
        default_button: Option<PointerButtonLike>,
    ) -> PyResult<Self> {
        let mut inner = runtime_rs::PointerSettings::default();
        if let Some(ms) = double_click_time_ms {
            inner.double_click_time = duration_from_millis(ms);
        }
        if let Some(SizeInput(size)) = double_click_size {
            inner.double_click_size = size;
        }
        if let Some(button) = default_button {
            inner.default_button = button.into();
        }
        Ok(Self { inner })
    }

    /// Returns a readable representation for debugging.
    fn __repr__(&self) -> String {
        format!(
            "PointerSettings(double_click_time_ms={}, default_button={})",
            self.double_click_time_ms(),
            self.default_button()
        )
    }

    #[classmethod]
    /// Accepts any object recognised by ``PointerSettingsLike`` and returns settings.
    fn from_like(_cls: &Bound<'_, PyType>, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let like = value.extract::<PointerSettingsLike>()?;
        Ok(Self { inner: like.into() })
    }

    #[getter]
    /// Returns the double-click time in milliseconds.
    fn double_click_time_ms(&self) -> f64 {
        self.inner.double_click_time.as_millis() as f64
    }

    #[getter]
    /// Returns the double-click bounding box as :class:`Size`.
    fn double_click_size(&self, py: Python<'_>) -> PyResult<Py<PySize>> {
        Py::new(py, PySize::from(self.inner.double_click_size))
    }

    #[getter]
    /// Returns the default pointer button as the numeric button id.
    fn default_button(&self) -> u16 {
        pointer_button_to_int(self.inner.default_button)
    }
}

impl From<runtime_rs::PointerSettings> for PyPointerSettings {
    fn from(inner: runtime_rs::PointerSettings) -> Self {
        Self { inner }
    }
}

/// Named pointer movement profile that can be swapped at runtime.
#[pyclass(module = "platynui_native", name = "PointerProfile")]
pub struct PyPointerProfile {
    pub(crate) inner: runtime_rs::PointerProfile,
}

#[pymethods]
impl PyPointerProfile {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (*,
        motion=None,
        steps_per_pixel=None,
        max_move_duration_ms=None,
        speed_factor=None,
        acceleration_profile=None,
        overshoot_ratio=None,
        overshoot_settle_steps=None,
        curve_amplitude=None,
        jitter_amplitude=None,
        after_move_delay_ms=None,
        after_input_delay_ms=None,
        press_release_delay_ms=None,
        after_click_delay_ms=None,
        before_next_click_delay_ms=None,
        multi_click_delay_ms=None,
        ensure_move_position=None,
        ensure_move_threshold=None,
        ensure_move_timeout_ms=None,
        scroll_step=None,
        scroll_delay_ms=None,
        move_time_per_pixel_us=None,
    ))]
    /// Creates a profile describing long-lived pointer behaviour.
    fn new(
        motion: Option<PointerMotionModeInput>,
        steps_per_pixel: Option<f64>,
        max_move_duration_ms: Option<f64>,
        speed_factor: Option<f64>,
        acceleration_profile: Option<PointerAccelerationInput>,
        overshoot_ratio: Option<f64>,
        overshoot_settle_steps: Option<u32>,
        curve_amplitude: Option<f64>,
        jitter_amplitude: Option<f64>,
        after_move_delay_ms: Option<f64>,
        after_input_delay_ms: Option<f64>,
        press_release_delay_ms: Option<f64>,
        after_click_delay_ms: Option<f64>,
        before_next_click_delay_ms: Option<f64>,
        multi_click_delay_ms: Option<f64>,
        ensure_move_position: Option<bool>,
        ensure_move_threshold: Option<f64>,
        ensure_move_timeout_ms: Option<f64>,
        scroll_step: Option<(f64, f64)>,
        scroll_delay_ms: Option<f64>,
        move_time_per_pixel_us: Option<f64>,
    ) -> PyResult<Self> {
        let mut inner = runtime_rs::PointerProfile::named_default();
        if let Some(mode) = motion {
            inner.mode = mode.into();
        }
        if let Some(v) = steps_per_pixel {
            inner.steps_per_pixel = v;
        }
        if let Some(ms) = max_move_duration_ms {
            inner.max_move_duration = duration_from_millis(ms);
        }
        if let Some(v) = speed_factor {
            inner.speed_factor = v;
        }
        if let Some(accel) = acceleration_profile {
            inner.acceleration_profile = accel.into();
        }
        if let Some(v) = overshoot_ratio {
            inner.overshoot_ratio = v;
        }
        if let Some(v) = overshoot_settle_steps {
            inner.overshoot_settle_steps = v;
        }
        if let Some(v) = curve_amplitude {
            inner.curve_amplitude = v;
        }
        if let Some(v) = jitter_amplitude {
            inner.jitter_amplitude = v;
        }
        if let Some(ms) = after_move_delay_ms {
            inner.after_move_delay = duration_from_millis(ms);
        }
        if let Some(ms) = after_input_delay_ms {
            inner.after_input_delay = duration_from_millis(ms);
        }
        if let Some(ms) = press_release_delay_ms {
            inner.press_release_delay = duration_from_millis(ms);
        }
        if let Some(ms) = after_click_delay_ms {
            inner.after_click_delay = duration_from_millis(ms);
        }
        if let Some(ms) = before_next_click_delay_ms {
            inner.before_next_click_delay = duration_from_millis(ms);
        }
        if let Some(ms) = multi_click_delay_ms {
            inner.multi_click_delay = duration_from_millis(ms);
        }
        if let Some(flag) = ensure_move_position {
            inner.ensure_move_position = flag;
        }
        if let Some(v) = ensure_move_threshold {
            inner.ensure_move_threshold = v;
        }
        if let Some(ms) = ensure_move_timeout_ms {
            inner.ensure_move_timeout = duration_from_millis(ms);
        }
        if let Some((h, v)) = scroll_step {
            inner.scroll_step = core_rs::platform::ScrollDelta::new(h, v);
        }
        if let Some(ms) = scroll_delay_ms {
            inner.scroll_delay = duration_from_millis(ms);
        }
        if let Some(us) = move_time_per_pixel_us {
            inner.move_time_per_pixel = duration_from_micros(us);
        }
        Ok(Self { inner })
    }

    /// Returns a readable representation for debugging.
    fn __repr__(&self) -> String {
        format!(
            "PointerProfile(mode='{}', speed_factor={})",
            pointer_motion_mode_to_str(self.inner.mode),
            self.inner.speed_factor
        )
    }

    #[classmethod]
    /// Accepts any object recognised by ``PointerProfileLike`` and creates a profile.
    fn from_like(_cls: &Bound<'_, PyType>, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let like = value.extract::<PointerProfileLike>()?;
        Ok(Self { inner: like.into() })
    }

    #[getter]
    /// Returns the pointer motion mode as a Python enum instance.
    fn motion(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        pointer_motion_mode_to_py(py, self.inner.mode)
    }

    #[getter]
    /// Returns the number of movement steps performed per pixel.
    fn steps_per_pixel(&self) -> f64 {
        self.inner.steps_per_pixel
    }

    #[getter]
    /// Returns the maximum pointer move duration in milliseconds.
    fn max_move_duration_ms(&self) -> f64 {
        self.inner.max_move_duration.as_millis() as f64
    }

    #[getter]
    /// Returns the pointer speed multiplier.
    fn speed_factor(&self) -> f64 {
        self.inner.speed_factor
    }

    #[getter]
    /// Returns the acceleration profile used for pointer moves.
    fn acceleration_profile(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        pointer_acceleration_to_py(py, self.inner.acceleration_profile)
    }

    #[getter]
    /// Returns the overshoot ratio applied to pointer motion targets.
    fn overshoot_ratio(&self) -> f64 {
        self.inner.overshoot_ratio
    }

    #[getter]
    /// Returns the number of easing steps performed after overshooting.
    fn overshoot_settle_steps(&self) -> u32 {
        self.inner.overshoot_settle_steps
    }

    #[getter]
    /// Returns the amplitude for curved pointer paths.
    fn curve_amplitude(&self) -> f64 {
        self.inner.curve_amplitude
    }

    #[getter]
    /// Returns the amplitude of random jitter during motion.
    fn jitter_amplitude(&self) -> f64 {
        self.inner.jitter_amplitude
    }

    #[getter]
    /// Returns the delay after finishing a pointer move, in milliseconds.
    fn after_move_delay_ms(&self) -> f64 {
        self.inner.after_move_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the delay after any pointer input, in milliseconds.
    fn after_input_delay_ms(&self) -> f64 {
        self.inner.after_input_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the delay between press and release actions, in milliseconds.
    fn press_release_delay_ms(&self) -> f64 {
        self.inner.press_release_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the pause after a click, in milliseconds.
    fn after_click_delay_ms(&self) -> f64 {
        self.inner.after_click_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the pause before the next click, in milliseconds.
    fn before_next_click_delay_ms(&self) -> f64 {
        self.inner.before_next_click_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the multi-click grouping timeout, in milliseconds.
    fn multi_click_delay_ms(&self) -> f64 {
        self.inner.multi_click_delay.as_millis() as f64
    }

    #[getter]
    /// Returns whether pointer moves must end at the requested position.
    fn ensure_move_position(&self) -> bool {
        self.inner.ensure_move_position
    }

    #[getter]
    /// Returns the allowed deviation, in pixels, when verifying pointer moves.
    fn ensure_move_threshold(&self) -> f64 {
        self.inner.ensure_move_threshold
    }

    #[getter]
    /// Returns the verification timeout, in milliseconds, for pointer moves.
    fn ensure_move_timeout_ms(&self) -> f64 {
        self.inner.ensure_move_timeout.as_millis() as f64
    }

    #[getter]
    /// Returns the scroll delta applied per step as ``(horizontal, vertical)``.
    fn scroll_step(&self) -> (f64, f64) {
        (self.inner.scroll_step.horizontal, self.inner.scroll_step.vertical)
    }

    #[getter]
    /// Returns the delay between scroll steps, in milliseconds.
    fn scroll_delay_ms(&self) -> f64 {
        self.inner.scroll_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the microseconds spent per pixel during pointer moves.
    fn move_time_per_pixel_us(&self) -> f64 {
        self.inner.move_time_per_pixel.as_micros() as f64
    }
}

impl From<runtime_rs::PointerProfile> for PyPointerProfile {
    fn from(inner: runtime_rs::PointerProfile) -> Self {
        Self { inner }
    }
}

/// Runtime keyboard timing defaults such as key press durations and delays.
#[pyclass(module = "platynui_native", name = "KeyboardSettings")]
pub struct PyKeyboardSettings {
    pub(crate) inner: core_rs::platform::KeyboardSettings,
}

#[pymethods]
impl PyKeyboardSettings {
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
    /// Creates keyboard timing defaults that can be applied to the runtime.
    fn new(
        press_delay_ms: Option<f64>,
        release_delay_ms: Option<f64>,
        between_keys_delay_ms: Option<f64>,
        chord_press_delay_ms: Option<f64>,
        chord_release_delay_ms: Option<f64>,
        after_sequence_delay_ms: Option<f64>,
        after_text_delay_ms: Option<f64>,
    ) -> Self {
        let mut inner = core_rs::platform::KeyboardSettings::default();
        if let Some(ms) = press_delay_ms {
            inner.press_delay = duration_from_millis(ms);
        }
        if let Some(ms) = release_delay_ms {
            inner.release_delay = duration_from_millis(ms);
        }
        if let Some(ms) = between_keys_delay_ms {
            inner.between_keys_delay = duration_from_millis(ms);
        }
        if let Some(ms) = chord_press_delay_ms {
            inner.chord_press_delay = duration_from_millis(ms);
        }
        if let Some(ms) = chord_release_delay_ms {
            inner.chord_release_delay = duration_from_millis(ms);
        }
        if let Some(ms) = after_sequence_delay_ms {
            inner.after_sequence_delay = duration_from_millis(ms);
        }
        if let Some(ms) = after_text_delay_ms {
            inner.after_text_delay = duration_from_millis(ms);
        }
        Self { inner }
    }

    /// Returns a readable representation for debugging.
    fn __repr__(&self) -> String {
        "KeyboardSettings(...)".to_string()
    }

    #[classmethod]
    /// Accepts any object recognised by ``KeyboardSettingsLike`` and returns settings.
    fn from_like(_cls: &Bound<'_, PyType>, value: Bound<'_, PyAny>) -> PyResult<Self> {
        let like = value.extract::<KeyboardSettingsLike>()?;
        Ok(Self { inner: like.into() })
    }

    #[getter]
    /// Returns the key press duration in milliseconds.
    fn press_delay_ms(&self) -> f64 {
        self.inner.press_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the key release duration in milliseconds.
    fn release_delay_ms(&self) -> f64 {
        self.inner.release_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the delay between consecutive key strokes.
    fn between_keys_delay_ms(&self) -> f64 {
        self.inner.between_keys_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the delay between pressing keys in a chord.
    fn chord_press_delay_ms(&self) -> f64 {
        self.inner.chord_press_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the delay between releasing keys in a chord.
    fn chord_release_delay_ms(&self) -> f64 {
        self.inner.chord_release_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the pause applied after a key sequence completes.
    fn after_sequence_delay_ms(&self) -> f64 {
        self.inner.after_sequence_delay.as_millis() as f64
    }

    #[getter]
    /// Returns the pause after typing text content.
    fn after_text_delay_ms(&self) -> f64 {
        self.inner.after_text_delay.as_millis() as f64
    }
}

impl From<core_rs::platform::KeyboardSettings> for PyKeyboardSettings {
    fn from(inner: core_rs::platform::KeyboardSettings) -> Self {
        Self { inner }
    }
}

pub struct PointerOverridesInput {
    pub origin: Option<OriginInput>,
    pub motion: Option<PointerMotionModeInput>,
    pub steps_per_pixel: Option<f64>,
    pub speed_factor: Option<f64>,
    pub acceleration_profile: Option<PointerAccelerationInput>,
    pub max_move_duration_ms: Option<f64>,
    pub move_time_per_pixel_us: Option<f64>,
    pub after_move_delay_ms: Option<f64>,
    pub after_input_delay_ms: Option<f64>,
    pub press_release_delay_ms: Option<f64>,
    pub after_click_delay_ms: Option<f64>,
    pub before_next_click_delay_ms: Option<f64>,
    pub multi_click_delay_ms: Option<f64>,
    pub overshoot_ratio: Option<f64>,
    pub overshoot_settle_steps: Option<u32>,
    pub curve_amplitude: Option<f64>,
    pub jitter_amplitude: Option<f64>,
    pub ensure_move_position: Option<bool>,
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
            motion: dict_get(d, "motion").map(|v| PointerMotionModeInput::extract((&v).into())).transpose()?,
            steps_per_pixel: dict_get(d, "steps_per_pixel").and_then(|v| v.extract().ok()),
            speed_factor: dict_get(d, "speed_factor").and_then(|v| v.extract().ok()),
            acceleration_profile: dict_get(d, "acceleration_profile")
                .map(|v| PointerAccelerationInput::extract((&v).into()))
                .transpose()?,
            max_move_duration_ms: dict_get(d, "max_move_duration_ms").and_then(|v| v.extract().ok()),
            move_time_per_pixel_us: dict_get(d, "move_time_per_pixel_us").and_then(|v| v.extract().ok()),
            after_move_delay_ms: dict_get(d, "after_move_delay_ms").and_then(|v| v.extract().ok()),
            after_input_delay_ms: dict_get(d, "after_input_delay_ms").and_then(|v| v.extract().ok()),
            press_release_delay_ms: dict_get(d, "press_release_delay_ms").and_then(|v| v.extract().ok()),
            after_click_delay_ms: dict_get(d, "after_click_delay_ms").and_then(|v| v.extract().ok()),
            before_next_click_delay_ms: dict_get(d, "before_next_click_delay_ms").and_then(|v| v.extract().ok()),
            multi_click_delay_ms: dict_get(d, "multi_click_delay_ms").and_then(|v| v.extract().ok()),
            overshoot_ratio: dict_get(d, "overshoot_ratio").and_then(|v| v.extract().ok()),
            overshoot_settle_steps: dict_get(d, "overshoot_settle_steps").and_then(|v| v.extract().ok()),
            curve_amplitude: dict_get(d, "curve_amplitude").and_then(|v| v.extract().ok()),
            jitter_amplitude: dict_get(d, "jitter_amplitude").and_then(|v| v.extract().ok()),
            ensure_move_position: dict_get(d, "ensure_move_position").and_then(|v| v.extract().ok()),
            ensure_move_threshold: dict_get(d, "ensure_move_threshold").and_then(|v| v.extract().ok()),
            ensure_move_timeout_ms: dict_get(d, "ensure_move_timeout_ms").and_then(|v| v.extract().ok()),
            scroll_step: dict_get(d, "scroll_step").and_then(|v| v.extract().ok()),
            scroll_delay_ms: dict_get(d, "scroll_delay_ms").and_then(|v| v.extract().ok()),
        })
    }
}

impl From<PointerOverridesInput> for runtime_rs::PointerOverrides {
    fn from(s: PointerOverridesInput) -> Self {
        let mut ov = runtime_rs::PointerOverrides::new();
        if let Some(origin) = s.origin {
            ov = ov.origin(origin.into());
        }
        if let Some(mode) = s.motion {
            ov = ov.motion_mode(mode.into());
        }
        if let Some(steps) = s.steps_per_pixel {
            ov = ov.steps_per_pixel(steps);
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
        if let Some(ratio) = s.overshoot_ratio {
            ov = ov.overshoot_ratio(ratio);
        }
        if let Some(steps) = s.overshoot_settle_steps {
            ov = ov.overshoot_settle_steps(steps);
        }
        if let Some(amplitude) = s.curve_amplitude {
            ov = ov.curve_amplitude(amplitude);
        }
        if let Some(amplitude) = s.jitter_amplitude {
            ov = ov.jitter_amplitude(amplitude);
        }
        if let Some(flag) = s.ensure_move_position {
            ov = ov.ensure_move_position(flag);
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
        if let Some(ap) = s.acceleration_profile {
            ov = ov.acceleration_profile(ap.into());
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

#[derive(Default)]
pub struct PointerSettingsInput {
    pub double_click_time_ms: Option<f64>,
    pub double_click_size: Option<SizeInput>,
    pub default_button: Option<PointerButtonLike>,
}

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for PointerSettingsInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let d_borrowed = ob.cast::<PyDict>()?;
        let d: &Bound<'py, PyDict> = &d_borrowed;
        Ok(Self {
            double_click_time_ms: dict_get(d, "double_click_time_ms").and_then(|v| v.extract().ok()),
            double_click_size: dict_get(d, "double_click_size").map(|v| SizeInput::extract((&v).into())).transpose()?,
            default_button: dict_get(d, "default_button").and_then(|v| v.extract().ok()),
        })
    }
}

impl From<PointerSettingsInput> for runtime_rs::PointerSettings {
    fn from(input: PointerSettingsInput) -> Self {
        let mut settings = runtime_rs::PointerSettings::default();
        if let Some(ms) = input.double_click_time_ms {
            settings.double_click_time = duration_from_millis(ms);
        }
        if let Some(SizeInput(size)) = input.double_click_size {
            settings.double_click_size = size;
        }
        if let Some(button) = input.default_button {
            settings.default_button = button.into();
        }
        settings
    }
}

#[derive(FromPyObject)]
pub enum PointerSettingsLike<'py> {
    Dict(PointerSettingsInput),
    Class(PyRef<'py, PyPointerSettings>),
}

impl From<PointerSettingsLike<'_>> for runtime_rs::PointerSettings {
    fn from(value: PointerSettingsLike<'_>) -> Self {
        match value {
            PointerSettingsLike::Dict(d) => d.into(),
            PointerSettingsLike::Class(c) => (*c).inner.clone(),
        }
    }
}

#[derive(Default)]
pub struct PointerProfileInput {
    pub motion: Option<PointerMotionModeInput>,
    pub steps_per_pixel: Option<f64>,
    pub max_move_duration_ms: Option<f64>,
    pub speed_factor: Option<f64>,
    pub acceleration_profile: Option<PointerAccelerationInput>,
    pub overshoot_ratio: Option<f64>,
    pub overshoot_settle_steps: Option<u32>,
    pub curve_amplitude: Option<f64>,
    pub jitter_amplitude: Option<f64>,
    pub after_move_delay_ms: Option<f64>,
    pub after_input_delay_ms: Option<f64>,
    pub press_release_delay_ms: Option<f64>,
    pub after_click_delay_ms: Option<f64>,
    pub before_next_click_delay_ms: Option<f64>,
    pub multi_click_delay_ms: Option<f64>,
    pub ensure_move_position: Option<bool>,
    pub ensure_move_threshold: Option<f64>,
    pub ensure_move_timeout_ms: Option<f64>,
    pub scroll_step: Option<(f64, f64)>,
    pub scroll_delay_ms: Option<f64>,
    pub move_time_per_pixel_us: Option<f64>,
}

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for PointerProfileInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let d_borrowed = ob.cast::<PyDict>()?;
        let d: &Bound<'py, PyDict> = &d_borrowed;
        Ok(Self {
            motion: dict_get(d, "motion").map(|v| PointerMotionModeInput::extract((&v).into())).transpose()?,
            steps_per_pixel: dict_get(d, "steps_per_pixel").and_then(|v| v.extract().ok()),
            max_move_duration_ms: dict_get(d, "max_move_duration_ms").and_then(|v| v.extract().ok()),
            speed_factor: dict_get(d, "speed_factor").and_then(|v| v.extract().ok()),
            acceleration_profile: dict_get(d, "acceleration_profile")
                .map(|v| PointerAccelerationInput::extract((&v).into()))
                .transpose()?,
            overshoot_ratio: dict_get(d, "overshoot_ratio").and_then(|v| v.extract().ok()),
            overshoot_settle_steps: dict_get(d, "overshoot_settle_steps").and_then(|v| v.extract().ok()),
            curve_amplitude: dict_get(d, "curve_amplitude").and_then(|v| v.extract().ok()),
            jitter_amplitude: dict_get(d, "jitter_amplitude").and_then(|v| v.extract().ok()),
            after_move_delay_ms: dict_get(d, "after_move_delay_ms").and_then(|v| v.extract().ok()),
            after_input_delay_ms: dict_get(d, "after_input_delay_ms").and_then(|v| v.extract().ok()),
            press_release_delay_ms: dict_get(d, "press_release_delay_ms").and_then(|v| v.extract().ok()),
            after_click_delay_ms: dict_get(d, "after_click_delay_ms").and_then(|v| v.extract().ok()),
            before_next_click_delay_ms: dict_get(d, "before_next_click_delay_ms").and_then(|v| v.extract().ok()),
            multi_click_delay_ms: dict_get(d, "multi_click_delay_ms").and_then(|v| v.extract().ok()),
            ensure_move_position: dict_get(d, "ensure_move_position").and_then(|v| v.extract().ok()),
            ensure_move_threshold: dict_get(d, "ensure_move_threshold").and_then(|v| v.extract().ok()),
            ensure_move_timeout_ms: dict_get(d, "ensure_move_timeout_ms").and_then(|v| v.extract().ok()),
            scroll_step: dict_get(d, "scroll_step").and_then(|v| v.extract().ok()),
            scroll_delay_ms: dict_get(d, "scroll_delay_ms").and_then(|v| v.extract().ok()),
            move_time_per_pixel_us: dict_get(d, "move_time_per_pixel_us").and_then(|v| v.extract().ok()),
        })
    }
}

impl From<PointerProfileInput> for runtime_rs::PointerProfile {
    fn from(input: PointerProfileInput) -> Self {
        let mut profile = runtime_rs::PointerProfile::named_default();
        if let Some(mode) = input.motion {
            profile.mode = mode.into();
        }
        if let Some(v) = input.steps_per_pixel {
            profile.steps_per_pixel = v;
        }
        if let Some(ms) = input.max_move_duration_ms {
            profile.max_move_duration = duration_from_millis(ms);
        }
        if let Some(v) = input.speed_factor {
            profile.speed_factor = v;
        }
        if let Some(accel) = input.acceleration_profile {
            profile.acceleration_profile = accel.into();
        }
        if let Some(v) = input.overshoot_ratio {
            profile.overshoot_ratio = v;
        }
        if let Some(v) = input.overshoot_settle_steps {
            profile.overshoot_settle_steps = v;
        }
        if let Some(v) = input.curve_amplitude {
            profile.curve_amplitude = v;
        }
        if let Some(v) = input.jitter_amplitude {
            profile.jitter_amplitude = v;
        }
        if let Some(ms) = input.after_move_delay_ms {
            profile.after_move_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.after_input_delay_ms {
            profile.after_input_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.press_release_delay_ms {
            profile.press_release_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.after_click_delay_ms {
            profile.after_click_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.before_next_click_delay_ms {
            profile.before_next_click_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.multi_click_delay_ms {
            profile.multi_click_delay = duration_from_millis(ms);
        }
        if let Some(flag) = input.ensure_move_position {
            profile.ensure_move_position = flag;
        }
        if let Some(v) = input.ensure_move_threshold {
            profile.ensure_move_threshold = v;
        }
        if let Some(ms) = input.ensure_move_timeout_ms {
            profile.ensure_move_timeout = duration_from_millis(ms);
        }
        if let Some((h, v)) = input.scroll_step {
            profile.scroll_step = core_rs::platform::ScrollDelta::new(h, v);
        }
        if let Some(ms) = input.scroll_delay_ms {
            profile.scroll_delay = duration_from_millis(ms);
        }
        if let Some(us) = input.move_time_per_pixel_us {
            profile.move_time_per_pixel = duration_from_micros(us);
        }
        profile
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(FromPyObject)]
pub enum PointerProfileLike<'py> {
    Dict(PointerProfileInput),
    Class(PyRef<'py, PyPointerProfile>),
}

impl From<PointerProfileLike<'_>> for runtime_rs::PointerProfile {
    fn from(value: PointerProfileLike<'_>) -> Self {
        match value {
            PointerProfileLike::Dict(d) => d.into(),
            PointerProfileLike::Class(c) => (*c).inner.clone(),
        }
    }
}

#[derive(Default)]
pub struct KeyboardSettingsInput {
    pub press_delay_ms: Option<f64>,
    pub release_delay_ms: Option<f64>,
    pub between_keys_delay_ms: Option<f64>,
    pub chord_press_delay_ms: Option<f64>,
    pub chord_release_delay_ms: Option<f64>,
    pub after_sequence_delay_ms: Option<f64>,
    pub after_text_delay_ms: Option<f64>,
}

impl<'a, 'py> pyo3::FromPyObject<'a, 'py> for KeyboardSettingsInput {
    type Error = PyErr;
    fn extract(ob: pyo3::Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let dict = ob.cast::<PyDict>()?;
        Ok(Self {
            press_delay_ms: dict_get(&dict, "press_delay_ms").and_then(|v| v.extract().ok()),
            release_delay_ms: dict_get(&dict, "release_delay_ms").and_then(|v| v.extract().ok()),
            between_keys_delay_ms: dict_get(&dict, "between_keys_delay_ms").and_then(|v| v.extract().ok()),
            chord_press_delay_ms: dict_get(&dict, "chord_press_delay_ms").and_then(|v| v.extract().ok()),
            chord_release_delay_ms: dict_get(&dict, "chord_release_delay_ms").and_then(|v| v.extract().ok()),
            after_sequence_delay_ms: dict_get(&dict, "after_sequence_delay_ms").and_then(|v| v.extract().ok()),
            after_text_delay_ms: dict_get(&dict, "after_text_delay_ms").and_then(|v| v.extract().ok()),
        })
    }
}

impl From<KeyboardSettingsInput> for core_rs::platform::KeyboardSettings {
    fn from(input: KeyboardSettingsInput) -> Self {
        let mut settings = core_rs::platform::KeyboardSettings::default();
        if let Some(ms) = input.press_delay_ms {
            settings.press_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.release_delay_ms {
            settings.release_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.between_keys_delay_ms {
            settings.between_keys_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.chord_press_delay_ms {
            settings.chord_press_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.chord_release_delay_ms {
            settings.chord_release_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.after_sequence_delay_ms {
            settings.after_sequence_delay = duration_from_millis(ms);
        }
        if let Some(ms) = input.after_text_delay_ms {
            settings.after_text_delay = duration_from_millis(ms);
        }
        settings
    }
}

#[derive(FromPyObject)]
pub enum KeyboardSettingsLike<'py> {
    Dict(KeyboardSettingsInput),
    Class(PyRef<'py, PyKeyboardSettings>),
}

impl From<KeyboardSettingsLike<'_>> for core_rs::platform::KeyboardSettings {
    fn from(value: KeyboardSettingsLike<'_>) -> Self {
        match value {
            KeyboardSettingsLike::Dict(d) => d.into(),
            KeyboardSettingsLike::Class(c) => (*c).inner.clone(),
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
