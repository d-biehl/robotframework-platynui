#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
use pyo3::prelude::*;

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

pub fn init_submodule(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPoint>()?;
    m.add_class::<PySize>()?;
    m.add_class::<PyRect>()?;
    m.add_class::<PatternId>()?;
    m.add_class::<RuntimeId>()?;
    m.add_class::<TechnologyId>()?;
    m.add_class::<PyNamespace>()?;
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
