#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::useless_conversion)]
use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

use platynui_core as core_rs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ---------- Point ----------

/// Immutable 2D point with ``x``/``y`` coordinates.
///
/// Create it with numeric ``x`` and ``y`` values or any point-like object. Supports
/// tuple semantics (length, indexing) and helpers such as :py:meth:`translate`,
/// :py:meth:`with_x`, and :py:meth:`with_y`.
#[pyclass(name = "Point", module = "platynui_native")]
#[derive(Clone)]
pub struct PyPoint {
    inner: core_rs::types::Point,
}

#[pymethods]
impl PyPoint {
    #[new]
    /// Creates a point at ``(x, y)`` coordinates.
    fn new(x: f64, y: f64) -> Self {
        Self { inner: core_rs::types::Point::new(x, y) }
    }
    #[getter]
    /// Returns the horizontal coordinate.
    fn x(&self) -> f64 {
        self.inner.x()
    }
    #[getter]
    /// Returns the vertical coordinate.
    fn y(&self) -> f64 {
        self.inner.y()
    }
    /// Returns the point as a ``(x, y)`` tuple.
    fn to_tuple(&self) -> (f64, f64) {
        (self.inner.x(), self.inner.y())
    }
    /// Returns ``2`` so the point can be treated like a sequence.
    fn __len__(&self) -> usize {
        2
    }
    /// Returns ``x`` for index ``0`` and ``y`` for index ``1``.
    fn __getitem__(&self, idx: isize) -> PyResult<f64> {
        match idx {
            0 => Ok(self.inner.x()),
            1 => Ok(self.inner.y()),
            _ => Err(PyIndexError::new_err("Point index out of range")),
        }
    }

    /// Returns a new point with the x-coordinate replaced.
    fn with_x(&self, x: f64) -> Self {
        Self { inner: self.inner.with_x(x) }
    }
    /// Returns a new point with the y-coordinate replaced.
    fn with_y(&self, y: f64) -> Self {
        Self { inner: self.inner.with_y(y) }
    }
    /// Returns a translated copy offset by ``(dx, dy)``.
    fn translate(&self, dx: f64, dy: f64) -> Self {
        Self { inner: self.inner.translate(dx, dy) }
    }
    /// Returns ``True`` when the coordinates are finite numbers.
    fn is_finite(&self) -> bool {
        self.inner.is_finite()
    }

    /// Adds another point component-wise and returns the result.
    fn __add__(&self, other: &PyPoint) -> Self {
        Self { inner: self.inner + other.inner }
    }
    /// Subtracts another point component-wise and returns the result.
    fn __sub__(&self, other: &PyPoint) -> Self {
        Self { inner: self.inner - other.inner }
    }
    /// Compares two points for equality.
    fn __eq__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        if let Some(p) = point_from_any(other) { Ok(self.inner == p) } else { Ok(false) }
    }
    /// Returns ``False`` when two points are equal.
    fn __ne__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }
    #[classmethod]
    #[pyo3(text_signature = "(value)")]
    /// Converts any point-like object into a :class:`Point` instance.
    fn from_like(_cls: &Bound<'_, pyo3::types::PyType>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Some(p) = point_from_any(value) {
            Ok(Self { inner: p })
        } else {
            Err(PyTypeError::new_err("Point.from_like(): expected Point | (x, y) | {x, y}"))
        }
    }
    /// Returns a ``Point(x, y)`` representation.
    fn __repr__(&self) -> String {
        format!("Point({}, {})", self.inner.x(), self.inner.y())
    }
    /// Returns a hash compatible with the tuple representation.
    fn __hash__(&self) -> PyResult<isize> {
        let mut s = DefaultHasher::new();
        self.inner.x().to_bits().hash(&mut s);
        self.inner.y().to_bits().hash(&mut s);
        Ok(s.finish() as isize)
    }
}

impl From<core_rs::types::Point> for PyPoint {
    fn from(p: core_rs::types::Point) -> Self {
        Self { inner: p }
    }
}

// ---------- Size ----------

/// Immutable size with ``width``/``height`` components.
///
/// Create it with numeric ``width`` and ``height`` or any size-like object.
/// Supports tuple semantics (length, indexing) and helpers such as
/// :py:meth:`area`, :py:meth:`is_empty`, and arithmetic operators.
#[pyclass(name = "Size", module = "platynui_native")]
#[derive(Clone)]
pub struct PySize {
    inner: core_rs::types::Size,
}

#[pymethods]
impl PySize {
    #[new]
    /// Creates a size with the given ``width`` and ``height``.
    fn new(width: f64, height: f64) -> Self {
        Self { inner: core_rs::types::Size::new(width, height) }
    }
    #[getter]
    /// Returns the width component.
    fn width(&self) -> f64 {
        self.inner.width()
    }
    #[getter]
    /// Returns the height component.
    fn height(&self) -> f64 {
        self.inner.height()
    }
    /// Returns ``(width, height)``.
    fn to_tuple(&self) -> (f64, f64) {
        (self.inner.width(), self.inner.height())
    }
    /// Returns ``2`` so the size behaves like a sequence.
    fn __len__(&self) -> usize {
        2
    }
    /// Returns ``width`` for index ``0`` and ``height`` for index ``1``.
    fn __getitem__(&self, idx: isize) -> PyResult<f64> {
        match idx {
            0 => Ok(self.inner.width()),
            1 => Ok(self.inner.height()),
            _ => Err(PyIndexError::new_err("Size index out of range")),
        }
    }

    /// Returns the area ``width * height``.
    fn area(&self) -> f64 {
        self.inner.area()
    }
    /// Returns ``True`` when width or height is zero.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Returns ``True`` when both dimensions are finite numbers.
    fn is_finite(&self) -> bool {
        self.inner.is_finite()
    }

    /// Adds another size component-wise.
    fn __add__(&self, other: &PySize) -> Self {
        Self { inner: self.inner + other.inner }
    }
    /// Subtracts another size component-wise.
    fn __sub__(&self, other: &PySize) -> Self {
        Self { inner: self.inner - other.inner }
    }
    /// Scales the size by ``scalar``.
    fn __mul__(&self, scalar: f64) -> Self {
        Self { inner: self.inner * scalar }
    }
    /// Divides the size by ``scalar``.
    fn __truediv__(&self, scalar: f64) -> Self {
        Self { inner: self.inner / scalar }
    }
    /// Compares two sizes for equality.
    fn __eq__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        if let Some(s) = size_from_any(other) { Ok(self.inner == s) } else { Ok(false) }
    }
    /// Returns ``False`` when two sizes are equal.
    fn __ne__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }
    #[classmethod]
    #[pyo3(text_signature = "(value)")]
    /// Converts any size-like object into a :class:`Size` instance.
    fn from_like(_cls: &Bound<'_, pyo3::types::PyType>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Some(s) = size_from_any(value) {
            Ok(Self { inner: s })
        } else {
            Err(PyTypeError::new_err("Size.from_like(): expected Size | (width, height) | {width, height}"))
        }
    }
    /// Returns ``Size(width, height)``.
    fn __repr__(&self) -> String {
        format!("Size({}, {})", self.inner.width(), self.inner.height())
    }
    /// Returns a hash compatible with the tuple representation.
    fn __hash__(&self) -> PyResult<isize> {
        let mut s = DefaultHasher::new();
        self.inner.width().to_bits().hash(&mut s);
        self.inner.height().to_bits().hash(&mut s);
        Ok(s.finish() as isize)
    }
}

impl From<core_rs::types::Size> for PySize {
    fn from(s: core_rs::types::Size) -> Self {
        Self { inner: s }
    }
}

impl PySize {
    pub(crate) fn as_inner(&self) -> core_rs::types::Size {
        self.inner
    }
}

// ---------- Rect ----------

/// Axis-aligned rectangle with position and size.
///
/// Construct it with ``x``, ``y``, ``width``, ``height`` values or any rect-like
/// object. Supports tuple semantics and helpers such as :py:meth:`contains`,
/// :py:meth:`intersection`, :py:meth:`union`, and translation/resize methods.
#[pyclass(name = "Rect", module = "platynui_native")]
#[derive(Clone)]
pub struct PyRect {
    inner: core_rs::types::Rect,
}

#[pymethods]
impl PyRect {
    #[new]
    /// Creates a rectangle at ``(x, y)`` with the specified size.
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { inner: core_rs::types::Rect::new(x, y, width, height) }
    }
    #[getter]
    /// Returns the left coordinate.
    fn x(&self) -> f64 {
        self.inner.x()
    }
    #[getter]
    /// Returns the top coordinate.
    fn y(&self) -> f64 {
        self.inner.y()
    }
    #[getter]
    /// Returns the width.
    fn width(&self) -> f64 {
        self.inner.width()
    }
    #[getter]
    /// Returns the height.
    fn height(&self) -> f64 {
        self.inner.height()
    }
    /// Returns ``(x, y, width, height)``.
    fn to_tuple(&self) -> (f64, f64, f64, f64) {
        (self.inner.x(), self.inner.y(), self.inner.width(), self.inner.height())
    }
    /// Returns ``4`` so the rectangle behaves like a sequence.
    fn __len__(&self) -> usize {
        4
    }
    /// Returns the requested component by index.
    fn __getitem__(&self, idx: isize) -> PyResult<f64> {
        match idx {
            0 => Ok(self.inner.x()),
            1 => Ok(self.inner.y()),
            2 => Ok(self.inner.width()),
            3 => Ok(self.inner.height()),
            _ => Err(PyIndexError::new_err("Rect index out of range")),
        }
    }

    /// Returns the left edge coordinate.
    fn left(&self) -> f64 {
        self.inner.left()
    }
    /// Returns the top edge coordinate.
    fn top(&self) -> f64 {
        self.inner.top()
    }
    /// Returns the right edge coordinate.
    fn right(&self) -> f64 {
        self.inner.right()
    }
    /// Returns the bottom edge coordinate.
    fn bottom(&self) -> f64 {
        self.inner.bottom()
    }
    /// Returns the rectangle centre as :class:`Point`.
    fn center(&self) -> PyPoint {
        PyPoint { inner: self.inner.center() }
    }
    /// Returns the size component of the rectangle.
    fn size(&self) -> PySize {
        PySize { inner: self.inner.size() }
    }
    /// Returns the top-left corner as :class:`Point`.
    fn position(&self) -> PyPoint {
        PyPoint { inner: self.inner.position() }
    }

    /// Returns ``True`` if ``point`` lies inside the rectangle.
    fn contains(&self, point: &PyPoint) -> bool {
        self.inner.contains(point.inner)
    }
    /// Returns ``True`` if this rectangle intersects ``other``.
    fn intersects(&self, other: &PyRect) -> bool {
        self.inner.intersects(&other.inner)
    }
    /// Returns the overlapping area with ``other`` or ``None``.
    fn intersection(&self, other: &PyRect) -> Option<PyRect> {
        self.inner.intersection(&other.inner).map(|r| PyRect { inner: r })
    }
    /// Returns the smallest rectangle covering both rectangles.
    fn union(&self, other: &PyRect) -> PyRect {
        PyRect { inner: self.inner.union(&other.inner) }
    }

    /// Returns a translated copy offset by ``(dx, dy)``.
    fn translate(&self, dx: f64, dy: f64) -> PyRect {
        PyRect { inner: self.inner.translate(dx, dy) }
    }
    /// Expands the rectangle by ``dw`` horizontally and ``dh`` vertically.
    fn inflate(&self, dw: f64, dh: f64) -> PyRect {
        PyRect { inner: self.inner.inflate(dw, dh) }
    }
    /// Shrinks the rectangle by ``dw`` horizontally and ``dh`` vertically.
    fn deflate(&self, dw: f64, dh: f64) -> PyRect {
        PyRect { inner: self.inner.deflate(dw, dh) }
    }
    /// Returns ``True`` when width or height is zero.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a rectangle translated by ``point``.
    fn __add__(&self, point: &PyPoint) -> PyRect {
        PyRect { inner: self.inner + point.inner }
    }
    /// Returns a rectangle translated by ``-point``.
    fn __sub__(&self, point: &PyPoint) -> PyRect {
        PyRect { inner: self.inner - point.inner }
    }
    /// Compares two rectangles for equality.
    fn __eq__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        if let Some(r) = rect_from_any(other) { Ok(self.inner == r) } else { Ok(false) }
    }
    /// Returns ``False`` when two rectangles are equal.
    fn __ne__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }
    #[classmethod]
    #[pyo3(text_signature = "(value)")]
    /// Converts any rect-like object into a :class:`Rect` instance.
    fn from_like(_cls: &Bound<'_, pyo3::types::PyType>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Some(r) = rect_from_any(value) {
            Ok(Self { inner: r })
        } else {
            Err(PyTypeError::new_err("Rect.from_like(): expected Rect | (x, y, width, height) | {x, y, width, height}"))
        }
    }
    /// Returns ``Rect(x, y, width, height)``.
    fn __repr__(&self) -> String {
        format!("Rect({}, {}, {}, {})", self.inner.x(), self.inner.y(), self.inner.width(), self.inner.height())
    }
    /// Returns a hash compatible with the tuple representation.
    fn __hash__(&self) -> PyResult<isize> {
        let mut s = DefaultHasher::new();
        self.inner.x().to_bits().hash(&mut s);
        self.inner.y().to_bits().hash(&mut s);
        self.inner.width().to_bits().hash(&mut s);
        self.inner.height().to_bits().hash(&mut s);
        Ok(s.finish() as isize)
    }
}

impl From<core_rs::types::Rect> for PyRect {
    fn from(r: core_rs::types::Rect) -> Self {
        Self { inner: r }
    }
}

// ---------- IDs ----------

macro_rules! define_id {
    ($name:ident, $rust:ty, $desc:expr) => {
        #[doc = concat!(
                    "Lightweight identifier for ",
                    $desc,
                    " exposed as :class:`",
                    stringify!($name),
                    "`.",
                    "\n\n",
                    "Behaves like an immutable string returned from runtime APIs."
                )]
        #[pyclass(module = "platynui_native", frozen)]
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
            fn __eq__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
                if let Ok(o) = other.extract::<PyRef<$name>>() { Ok(self.inner == o.inner) } else { Ok(false) }
            }
            fn __ne__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
                self.__eq__(other).map(|eq| !eq)
            }
            fn __hash__(&self) -> PyResult<isize> {
                let mut s = DefaultHasher::new();
                self.inner.as_str().hash(&mut s);
                Ok(s.finish() as isize)
            }
        }
    };
}

define_id!(PatternId, core_rs::ui::identifiers::PatternId, "pattern handles");
define_id!(RuntimeId, core_rs::ui::identifiers::RuntimeId, "runtime instances");
define_id!(TechnologyId, core_rs::ui::identifiers::TechnologyId, "accessibility technologies");

// ---------- Namespace ----------

/// Identifier for attribute/pattern namespaces such as ``control`` or ``native``.
///
/// Exposes the common namespace constants and behaves like an immutable string
/// for comparisons and repr output.
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
    fn __eq__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        if let Ok(o) = other.extract::<PyRef<PyNamespace>>() { Ok(self.inner == o.inner) } else { Ok(false) }
    }
    fn __ne__(&self, other: &Bound<'_, pyo3::types::PyAny>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }
    fn __hash__(&self) -> PyResult<isize> {
        let mut s = DefaultHasher::new();
        self.inner.as_str().hash(&mut s);
        Ok(s.finish() as isize)
    }
}

pub(crate) fn py_namespace_from_inner(ns: core_rs::ui::namespace::Namespace) -> PyNamespace {
    PyNamespace { inner: ns }
}

/// Register all core types directly into the module (no submodule).
pub fn register_types(m: &Bound<'_, PyModule>) -> PyResult<()> {
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

// ---------- Like-type helpers (private) ----------

fn as_f64(obj: &Bound<'_, PyAny>) -> Option<f64> {
    obj.extract::<f64>().ok()
}

fn dict_get_ci<'py>(d: &Bound<'py, PyDict>, k: &str) -> Option<Bound<'py, PyAny>> {
    if let Ok(v) = d.get_item(k)
        && v.is_some()
    {
        return v;
    }
    let k2 = k.to_ascii_uppercase();
    if let Ok(v) = d.get_item(k2.as_str())
        && v.is_some()
    {
        return v;
    }
    None
}

pub(crate) fn point_from_any(obj: &Bound<'_, PyAny>) -> Option<core_rs::types::Point> {
    // Point instance
    if let Ok(p) = obj.extract::<PyRef<PyPoint>>() {
        return Some(p.inner);
    }
    // Tuple or list
    if let Ok((x, y)) = obj.extract::<(f64, f64)>() {
        return Some(core_rs::types::Point::new(x, y));
    }
    if let Ok(seq) = obj.extract::<Vec<f64>>()
        && seq.len() == 2
    {
        return Some(core_rs::types::Point::new(seq[0], seq[1]));
    }
    // Dict {x, y}
    if let Ok(d) = obj.cast::<PyDict>() {
        let x = dict_get_ci(d, "x").and_then(|v| as_f64(&v));
        let y = dict_get_ci(d, "y").and_then(|v| as_f64(&v));
        if let (Some(x), Some(y)) = (x, y) {
            return Some(core_rs::types::Point::new(x, y));
        }
    }
    None
}

pub(crate) fn size_from_any(obj: &Bound<'_, PyAny>) -> Option<core_rs::types::Size> {
    if let Ok(s) = obj.extract::<PyRef<PySize>>() {
        return Some(s.inner);
    }
    if let Ok((w, h)) = obj.extract::<(f64, f64)>() {
        return Some(core_rs::types::Size::new(w, h));
    }
    if let Ok(seq) = obj.extract::<Vec<f64>>()
        && seq.len() == 2
    {
        return Some(core_rs::types::Size::new(seq[0], seq[1]));
    }
    if let Ok(d) = obj.cast::<PyDict>() {
        // prefer width/height, tolerate w/h
        let w = dict_get_ci(d, "width").or_else(|| dict_get_ci(d, "w")).and_then(|v| as_f64(&v));
        let h = dict_get_ci(d, "height").or_else(|| dict_get_ci(d, "h")).and_then(|v| as_f64(&v));
        if let (Some(w), Some(h)) = (w, h) {
            return Some(core_rs::types::Size::new(w, h));
        }
    }
    None
}

pub(crate) fn rect_from_any(obj: &Bound<'_, PyAny>) -> Option<core_rs::types::Rect> {
    if let Ok(r) = obj.extract::<PyRef<PyRect>>() {
        return Some(r.inner);
    }
    if let Ok((x, y, w, h)) = obj.extract::<(f64, f64, f64, f64)>() {
        return Some(core_rs::types::Rect::new(x, y, w, h));
    }
    if let Ok(seq) = obj.extract::<Vec<f64>>()
        && seq.len() == 4
    {
        return Some(core_rs::types::Rect::new(seq[0], seq[1], seq[2], seq[3]));
    }
    if let Ok(d) = obj.cast::<PyDict>() {
        let x = dict_get_ci(d, "x").and_then(|v| as_f64(&v));
        let y = dict_get_ci(d, "y").and_then(|v| as_f64(&v));
        let w = dict_get_ci(d, "width").and_then(|v| as_f64(&v));
        let h = dict_get_ci(d, "height").and_then(|v| as_f64(&v));
        if let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) {
            return Some(core_rs::types::Rect::new(x, y, w, h));
        }
    }
    None
}
