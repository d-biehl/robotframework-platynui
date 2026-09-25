//! Native diagnostics into Python's `logging` module.
//!
//! The extension installs one process-wide `tracing` subscriber at import. Its
//! layer turns every enabled event into a [`Record`] and queues it — nothing
//! more: it holds no Python object and never waits for the interpreter, so a
//! native thread can log while the Python caller holds the GIL and waits for
//! that very thread. Queued records reach Python only on a thread that calls
//! into the extension:
//!
//! - when a runtime call returns ([`crate::runtime`]'s runtime guard delivers
//!   after releasing the runtime lock);
//! - on [`flush_logs`], which the Robot Framework library calls around every
//!   keyword, so Robot Framework — which drops messages from any other thread —
//!   records them in the keyword they belong to.
//!
//! A record keeps the time and thread it was emitted on; a record from another
//! thread names both in its message, because Robot Framework stamps a message
//! with its delivery time. By default only `WARN` and `ERROR` are produced;
//! [`set_log_level`] and the environment variables the command-line tool uses
//! (`RUST_LOG`, `PLATYNUI_LOG_LEVEL`) lower that, with the tool's precedence.
//! See `dev-docs/python-bindings.md` for the whole picture.

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};
use std::thread::ThreadId;
use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};
use tracing_subscriber::{EnvFilter, Registry, reload};

/// Records waiting for delivery; beyond this, new records are dropped.
const CAPACITY: usize = 10_000;
/// The Python logger every native logger hangs under.
const ROOT_LOGGER: &str = "platynui.native";
/// Python's level for native `trace` events: below `DEBUG`.
const TRACE_LEVEL: u8 = 5;
/// The levels `set_log_level` accepts, most severe first.
const LEVEL_NAMES: &str = "error, warn, info, debug, trace";
/// Delivery passes per delivery point: the records present, plus what a log
/// handler that calls back into `PlatynUI` queued meanwhile — bounded, so such a
/// handler cannot keep a delivery going forever.
const PASSES: usize = 2;

/// A native event, as it waits for delivery.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Record {
    pub(crate) level: Level,
    pub(crate) target: String,
    pub(crate) message: String,
    /// Structured fields in emission order, values in their `Debug` form.
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<u32>,
    pub(crate) thread: ThreadId,
    pub(crate) thread_name: String,
    pub(crate) time: SystemTime,
}

impl Record {
    fn from_event(event: &Event<'_>) -> Self {
        let metadata = event.metadata();
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        Self::on_this_thread(*metadata.level(), metadata.target(), visitor.message, visitor.fields)
            .with_location(metadata.file(), metadata.line())
    }

    /// A record the bridge itself produces, bypassing the level filter.
    fn from_bridge(message: String) -> Self {
        Self::on_this_thread(Level::WARN, module_path!(), message, Vec::new())
    }

    fn on_this_thread(level: Level, target: &str, message: String, fields: Vec<(String, String)>) -> Self {
        let thread = std::thread::current();
        Self {
            level,
            target: target.to_owned(),
            message,
            fields,
            file: None,
            line: None,
            thread: thread.id(),
            thread_name: thread.name().map_or_else(|| format!("{:?}", thread.id()), str::to_owned),
            time: SystemTime::now(),
        }
    }

    fn with_location(mut self, file: Option<&str>, line: Option<u32>) -> Self {
        self.file = file.map(str::to_owned);
        self.line = line;
        self
    }
}

/// Collects an event's message and fields.
#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.push((field.name().to_owned(), format!("{value:?}")));
        }
    }
}

/// Records waiting for delivery, bounded: when full, new records are dropped
/// and counted, so the beginning of a burst — usually its cause — survives.
#[derive(Debug)]
pub(crate) struct Queue {
    records: VecDeque<Record>,
    dropped: u64,
    capacity: usize,
}

impl Queue {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self { records: VecDeque::new(), dropped: 0, capacity }
    }

    fn push(&mut self, record: Record) {
        if self.records.len() < self.capacity {
            self.records.push_back(record);
        } else {
            self.dropped += 1;
        }
    }

    /// Everything queued so far, and how many records were dropped since the
    /// last take.
    pub(crate) fn take(&mut self) -> (Vec<Record>, u64) {
        (self.records.drain(..).collect(), std::mem::take(&mut self.dropped))
    }
}

/// The subscriber layer: queues each enabled event, nothing else.
pub(crate) struct QueueLayer {
    queue: Arc<Mutex<Queue>>,
}

impl QueueLayer {
    pub(crate) fn new(queue: Arc<Mutex<Queue>>) -> Self {
        Self { queue }
    }
}

impl<S: Subscriber> Layer<S> for QueueLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let record = Record::from_event(event);
        self.queue.lock().unwrap_or_else(PoisonError::into_inner).push(record);
    }
}

/// The emitting module in dotted form, without the `platynui_` crate prefix.
fn module_label(target: &str) -> String {
    target.strip_prefix("platynui_").unwrap_or(target).replace("::", ".")
}

/// The Python logger a record from `target` is emitted under.
pub(crate) fn logger_name(target: &str) -> String {
    format!("{ROOT_LOGGER}.{}", module_label(target))
}

/// The message text Python and Robot Framework see: module, message and
/// fields, and — for a record from another thread than `delivering` — that
/// thread and the time it logged, rendered by `format_time`.
pub(crate) fn render_message(
    record: &Record,
    delivering: ThreadId,
    format_time: impl FnOnce(SystemTime) -> String,
) -> String {
    let mut text = format!("[{}] {}", module_label(&record.target), record.message);
    for (name, value) in &record.fields {
        let _ = write!(text, " {name}={value}");
    }
    if record.thread != delivering {
        let _ = write!(text, " (thread {}, {})", record.thread_name, format_time(record.time));
    }
    text
}

/// The filter directives in effect, and the environment values that were
/// consulted but did not parse.
#[derive(Debug)]
pub(crate) struct FilterSpec {
    pub(crate) directives: String,
    pub(crate) rejected: Vec<(&'static str, String)>,
}

/// Build the filter with the command-line tool's precedence: `RUST_LOG`, then
/// the requested level (`PlatynUI`'s own modules only), then
/// `PLATYNUI_LOG_LEVEL`, then `warn`. An environment value that does not
/// parse counts as absent and is reported.
pub(crate) fn filter_spec(env: impl Fn(&str) -> Option<String>, requested: Option<Level>) -> FilterSpec {
    let mut rejected = Vec::new();
    let mut from_env = |name: &'static str| {
        let value = env(name).filter(|value| !value.trim().is_empty())?;
        if EnvFilter::builder().parse(&value).is_ok() {
            Some(value)
        } else {
            rejected.push((name, value));
            None
        }
    };
    let directives = from_env("RUST_LOG")
        .or_else(|| requested.map(|level| format!("warn,platynui={}", level.as_str().to_ascii_lowercase())))
        .or_else(|| from_env("PLATYNUI_LOG_LEVEL"))
        .unwrap_or_else(|| "warn".to_owned());
    FilterSpec { directives, rejected }
}

/// Records waiting for delivery, process-wide.
static QUEUE: LazyLock<Arc<Mutex<Queue>>> = LazyLock::new(|| Arc::new(Mutex::new(Queue::with_capacity(CAPACITY))));
/// Swaps the installed filter; unset when another subscriber was already installed.
static FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();
/// The level requested through [`set_log_level`].
static REQUESTED: Mutex<Option<Level>> = Mutex::new(None);

thread_local! {
    /// Set while this thread delivers, so a handler that calls back into
    /// `PlatynUI` does not start a nested delivery.
    static DELIVERING: Cell<bool> = const { Cell::new(false) };
}

fn queue_bridge_warning(message: String) {
    QUEUE.lock().unwrap_or_else(PoisonError::into_inner).push(Record::from_bridge(message));
}

/// The filter for the current environment and request; reports rejected
/// environment values into the queue.
fn current_filter() -> EnvFilter {
    let requested = *REQUESTED.lock().unwrap_or_else(PoisonError::into_inner);
    let spec = filter_spec(|name| std::env::var(name).ok(), requested);
    for (name, value) in spec.rejected {
        queue_bridge_warning(format!("ignoring {name}={value:?}: not a valid log filter directive"));
    }
    EnvFilter::builder().parse_lossy(spec.directives)
}

/// Install the queueing subscriber; called once when the extension is imported.
pub(crate) fn install(py: Python<'_>) -> PyResult<()> {
    let logging = py.import("logging")?;
    if logging.call_method1("getLevelName", (TRACE_LEVEL,))?.extract::<String>()? == format!("Level {TRACE_LEVEL}") {
        logging.call_method1("addLevelName", (TRACE_LEVEL, "TRACE"))?;
    }

    let (filter, handle) = reload::Layer::new(current_filter());
    let subscriber = tracing_subscriber::registry().with(filter).with(QueueLayer::new(Arc::clone(&QUEUE)));
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        let _ = FILTER.set(handle);
    } else {
        queue_bridge_warning(
            "another tracing subscriber is already installed in this process; native diagnostics do not reach Python"
                .to_owned(),
        );
    }
    Ok(())
}

/// Marks this thread as delivering for as long as it lives.
struct Delivering;

impl Delivering {
    /// `None` when this thread is already delivering.
    fn start() -> Option<Self> {
        (!DELIVERING.with(|flag| flag.replace(true))).then_some(Self)
    }
}

impl Drop for Delivering {
    fn drop(&mut self) {
        DELIVERING.with(|flag| flag.set(false));
    }
}

/// Deliver every queued record to Python `logging` on this thread.
pub(crate) fn deliver(py: Python<'_>) {
    let Some(_delivering) = Delivering::start() else {
        return;
    };

    for _ in 0..PASSES {
        let (records, dropped) = QUEUE.lock().unwrap_or_else(PoisonError::into_inner).take();
        if records.is_empty() && dropped == 0 {
            break;
        }
        let logging = match py.import("logging") {
            Ok(logging) => logging,
            Err(err) => return err.write_unraisable(py, None),
        };
        let dropped_report = (dropped > 0).then(|| {
            Record::from_bridge(format!(
                "{dropped} native log records were dropped: more were emitted than the queue holds \
                 ({CAPACITY}) before they could be delivered"
            ))
        });
        for record in records.iter().chain(dropped_report.iter()) {
            if let Err(err) = deliver_one(py, &logging, record) {
                err.write_unraisable(py, None);
            }
        }
    }
}

/// Deliver from a place that does not hold a `Python` token — the runtime
/// guard. Does nothing while the interpreter is finalizing.
pub(crate) fn deliver_attached() {
    let _ = Python::try_attach(deliver);
}

fn deliver_one(py: Python<'_>, logging: &Bound<'_, PyModule>, record: &Record) -> PyResult<()> {
    let name = logger_name(&record.target);
    let logger = logging.call_method1("getLogger", (&name,))?;
    let level = python_level(record.level);
    if !logger.call_method1("isEnabledFor", (level,))?.is_truthy()? {
        return Ok(());
    }

    let here = std::thread::current().id();
    let message = render_message(record, here, |time| clock_time(py, time).unwrap_or_else(|_| "?".to_owned()));
    let fields = PyDict::new(py);
    for (field, value) in &record.fields {
        fields.set_item(field, value)?;
    }
    let extra = PyDict::new(py);
    extra.set_item("native_fields", fields)?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("extra", extra)?;
    let args = (
        &name,
        level,
        record.file.as_deref().unwrap_or(""),
        record.line.unwrap_or(0),
        message,
        PyTuple::empty(py),
        py.None(),
    );
    let log_record = logger.call_method("makeRecord", args, Some(&kwargs))?;

    let created = seconds_since_epoch(record.time);
    log_record.setattr("created", created)?;
    log_record.setattr("msecs", ((created - created.floor()) * 1000.0).floor())?;
    if record.thread != here {
        log_record.setattr("threadName", &record.thread_name)?;
        log_record.setattr("thread", thread_number(record.thread))?;
    }
    logger.call_method1("handle", (log_record,))?;
    Ok(())
}

fn python_level(level: Level) -> u8 {
    match level {
        Level::ERROR => 40,
        Level::WARN => 30,
        Level::INFO => 20,
        Level::DEBUG => 10,
        Level::TRACE => TRACE_LEVEL,
    }
}

fn seconds_since_epoch(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH).map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

/// Local wall-clock time with milliseconds, as Robot Framework shows it.
fn clock_time(py: Python<'_>, time: SystemTime) -> PyResult<String> {
    let created = seconds_since_epoch(time);
    let module = py.import("time")?;
    let local = module.call_method1("localtime", (created,))?;
    let hms: String = module.call_method1("strftime", ("%H:%M:%S", local))?.extract()?;
    Ok(format!("{hms}.{:03}", ((created - created.floor()) * 1000.0).floor()))
}

/// A number for a native thread, for `LogRecord.thread`: Rust's thread id.
fn thread_number(thread: ThreadId) -> u64 {
    format!("{thread:?}").trim_start_matches("ThreadId(").trim_end_matches(')').parse().unwrap_or(0)
}

fn parse_level(level: &str) -> PyResult<Level> {
    match level.to_ascii_lowercase().as_str() {
        "error" => Ok(Level::ERROR),
        "warn" => Ok(Level::WARN),
        "info" => Ok(Level::INFO),
        "debug" => Ok(Level::DEBUG),
        "trace" => Ok(Level::TRACE),
        _ => Err(PyValueError::new_err(format!("unknown native log level {level:?}; expected one of {LEVEL_NAMES}"))),
    }
}

/// Deliver every queued native log record to Python `logging` now.
///
/// Records are otherwise delivered when a runtime call returns; call this
/// after working with nodes and patterns, whose calls do not deliver.
#[pyfunction]
fn flush_logs(py: Python<'_>) {
    deliver(py);
}

/// Set the level down to which the extension's own native modules produce log records.
///
/// ``None`` returns to the default, ``warn``. Accepts ``error``, ``warn``,
/// ``info``, ``debug`` and ``trace`` (case-insensitive). ``RUST_LOG`` in the
/// environment takes precedence; ``PLATYNUI_LOG_LEVEL`` applies when no level
/// is set. Third-party modules stay at ``warn`` unless the environment says
/// otherwise.
#[pyfunction]
#[pyo3(signature = (level=None))]
fn set_log_level(level: Option<&str>) -> PyResult<()> {
    let requested = level.map(parse_level).transpose()?;
    *REQUESTED.lock().unwrap_or_else(PoisonError::into_inner) = requested;
    let filter = current_filter();
    if let Some(handle) = FILTER.get() {
        let _ = handle.reload(filter);
    }
    Ok(())
}

/// Test hook: emit `count` events at `level` with the fields ``alpha=1`` and
/// ``beta="two"``, on this thread or on a Rust thread this call joins while it
/// holds the interpreter, then deliver as a runtime call does.
#[cfg(feature = "mock-provider")]
#[pyfunction]
#[pyo3(signature = (level, message, *, on_background_thread=false, count=1))]
fn _emit_log_for_tests(
    py: Python<'_>,
    level: &str,
    message: String,
    on_background_thread: bool,
    count: usize,
) -> PyResult<()> {
    let level = parse_level(level)?;
    let emit = move || {
        for index in 0..count {
            let text = if count > 1 { format!("{message} #{index}") } else { message.clone() };
            match level {
                Level::ERROR => tracing::error!(alpha = 1, beta = "two", "{text}"),
                Level::WARN => tracing::warn!(alpha = 1, beta = "two", "{text}"),
                Level::INFO => tracing::info!(alpha = 1, beta = "two", "{text}"),
                Level::DEBUG => tracing::debug!(alpha = 1, beta = "two", "{text}"),
                Level::TRACE => tracing::trace!(alpha = 1, beta = "two", "{text}"),
            }
        }
    };
    if on_background_thread {
        std::thread::Builder::new()
            .name("platynui-log-test".to_owned())
            .spawn(emit)
            .and_then(|thread| thread.join().map_err(|_| std::io::Error::other("emitter thread panicked")))
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
    } else {
        emit();
    }
    deliver(py);
    Ok(())
}

/// Register the logging functions in the extension module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(flush_logs, m)?)?;
    m.add_function(wrap_pyfunction!(set_log_level, m)?)?;
    #[cfg(feature = "mock-provider")]
    m.add_function(wrap_pyfunction!(_emit_log_for_tests, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    fn queue(capacity: usize) -> Arc<Mutex<Queue>> {
        Arc::new(Mutex::new(Queue::with_capacity(capacity)))
    }

    /// Run `f` with a subscriber that queues into `queue` behind `filter`.
    fn with_queueing<R>(queue: &Arc<Mutex<Queue>>, filter: &str, f: impl FnOnce() -> R) -> R {
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(filter))
            .with(QueueLayer::new(Arc::clone(queue)));
        tracing::subscriber::with_default(subscriber, f)
    }

    fn taken(queue: &Arc<Mutex<Queue>>) -> (Vec<Record>, u64) {
        queue.lock().unwrap().take()
    }

    #[test]
    fn an_event_becomes_a_record_with_everything_it_carries() {
        let q = queue(8);
        with_queueing(&q, "trace", || {
            tracing::warn!(target: "platynui_provider_atspi::extents", alpha = 1, beta = "two", "something {}", "happened");
        });
        let (records, dropped) = taken(&q);
        assert_eq!(dropped, 0);
        let [record] = records.as_slice() else { panic!("expected one record, got {}", records.len()) };
        assert_eq!(record.level, tracing::Level::WARN);
        assert_eq!(record.target, "platynui_provider_atspi::extents");
        assert_eq!(record.message, "something happened");
        assert_eq!(record.fields, [("alpha".to_owned(), "1".to_owned()), ("beta".to_owned(), "\"two\"".to_owned())]);
        assert_eq!(record.thread, std::thread::current().id());
        assert!(record.file.as_deref().is_some_and(|file| file.ends_with("log_bridge.rs")));
        assert!(record.line.is_some());
    }

    #[test]
    fn the_logger_name_follows_the_emitting_module() {
        assert_eq!(logger_name("platynui_provider_atspi::extents"), "platynui.native.provider_atspi.extents");
        assert_eq!(logger_name("zbus::connection"), "platynui.native.zbus.connection");
        assert_eq!(logger_name("platynui_runtime"), "platynui.native.runtime");
    }

    #[test]
    fn the_message_names_the_module_and_every_field() {
        let q = queue(8);
        with_queueing(&q, "trace", || {
            tracing::warn!(target: "platynui_provider_atspi::extents", alpha = 1, beta = "two", "something happened");
        });
        let (records, _) = taken(&q);
        let text = render_message(&records[0], std::thread::current().id(), |_| unreachable!("same thread"));
        assert_eq!(text, "[provider_atspi.extents] something happened alpha=1 beta=\"two\"");
    }

    #[test]
    fn only_a_record_from_another_thread_names_its_thread_and_time() {
        let q = queue(8);
        let emitter = std::thread::Builder::new()
            .name("atspi-popup-watch".into())
            .spawn({
                let q = Arc::clone(&q);
                move || with_queueing(&q, "trace", || tracing::warn!(target: "platynui_runtime", "from elsewhere"))
            })
            .unwrap();
        emitter.join().unwrap();
        with_queueing(&q, "trace", || tracing::warn!(target: "platynui_runtime", "from here"));

        let (records, _) = taken(&q);
        let here = std::thread::current().id();
        let at = |_: SystemTime| "12:34:56.789".to_owned();
        assert_eq!(
            render_message(&records[0], here, at),
            "[runtime] from elsewhere (thread atspi-popup-watch, 12:34:56.789)"
        );
        assert_eq!(render_message(&records[1], here, at), "[runtime] from here");
    }

    #[test]
    fn the_queue_keeps_order_drops_the_newest_and_counts() {
        let q = queue(3);
        with_queueing(&q, "trace", || {
            for i in 0..5 {
                tracing::warn!(target: "platynui_runtime", "record {i}");
            }
        });
        let (records, dropped) = taken(&q);
        let messages: Vec<_> = records.iter().map(|r| r.message.as_str()).collect();
        assert_eq!(messages, ["record 0", "record 1", "record 2"]);
        assert_eq!(dropped, 2);
        assert_eq!(taken(&q), (Vec::new(), 0), "taking resets the queue and the count");
    }

    #[test]
    fn a_requested_level_lowers_platynui_modules_only() {
        let q = queue(8);
        let spec = filter_spec(|_| None, Some(tracing::Level::DEBUG));
        assert_eq!(spec.directives, "warn,platynui=debug");
        with_queueing(&q, &spec.directives, || {
            tracing::debug!(target: "platynui_provider_atspi::extents", "ours");
            tracing::debug!(target: "zbus::connection", "theirs");
            tracing::warn!(target: "zbus::connection", "their warning");
        });
        let (records, _) = taken(&q);
        let messages: Vec<_> = records.iter().map(|r| r.message.as_str()).collect();
        assert_eq!(messages, ["ours", "their warning"]);
    }

    #[test]
    fn the_filter_follows_the_command_line_precedence() {
        let env = |rust_log: Option<&'static str>, platynui: Option<&'static str>| {
            move |name: &str| match name {
                "RUST_LOG" => rust_log.map(str::to_owned),
                "PLATYNUI_LOG_LEVEL" => platynui.map(str::to_owned),
                _ => None,
            }
        };
        let debug = Some(tracing::Level::DEBUG);
        assert_eq!(filter_spec(env(None, None), None).directives, "warn");
        assert_eq!(filter_spec(env(None, Some("info")), None).directives, "info");
        assert_eq!(filter_spec(env(None, Some("info")), debug).directives, "warn,platynui=debug");
        assert_eq!(filter_spec(env(Some("zbus=debug"), Some("info")), debug).directives, "zbus=debug");
    }

    #[test]
    fn an_unparsable_environment_value_is_skipped_and_named() {
        let spec = filter_spec(|name| (name == "PLATYNUI_LOG_LEVEL").then(|| "platynui=loud".to_owned()), None);
        assert_eq!(spec.directives, "warn");
        assert_eq!(spec.rejected, [("PLATYNUI_LOG_LEVEL", "platynui=loud".to_owned())]);

        let spec = filter_spec(|name| (name == "RUST_LOG").then(|| "zbus=loud".to_owned()), Some(tracing::Level::INFO));
        assert_eq!(spec.directives, "warn,platynui=info", "a rejected RUST_LOG falls through to the next source");
        assert_eq!(spec.rejected, [("RUST_LOG", "zbus=loud".to_owned())]);
    }

    #[test]
    fn logging_never_waits_for_the_delivering_side() {
        let q = queue(8);
        with_queueing(&q, "trace", || tracing::warn!(target: "platynui_runtime", "first"));
        // The delivering side has taken its batch and is busy with it (in
        // Python, in the real bridge); another thread logs meanwhile.
        let (batch, _) = taken(&q);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let emitter = std::thread::spawn({
            let q = Arc::clone(&q);
            move || {
                with_queueing(&q, "trace", || tracing::warn!(target: "platynui_runtime", "meanwhile"));
                done_tx.send(()).unwrap();
            }
        });
        done_rx.recv_timeout(Duration::from_secs(5)).expect("logging must not wait for the delivering side");
        emitter.join().unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(taken(&q).0.len(), 1);
    }
}
