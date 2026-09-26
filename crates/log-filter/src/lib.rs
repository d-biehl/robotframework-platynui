//! The log filter shared by `PlatynUI`'s entry points: the command-line tool,
//! the Inspector and the Python extension.
//!
//! The filter comes from the first of these sources that is present:
//!
//! 1. `RUST_LOG`, used verbatim: the only source of filter directives, and the
//!    only way to make third-party modules more verbose;
//! 2. the level the entry point was given (`--log-level`, `native_log_level`);
//! 3. `PLATYNUI_LOG_LEVEL`, a single level;
//! 4. `warn`.
//!
//! A single level of `warn`, `info`, `debug` or `trace` lowers only
//! `PlatynUI`'s own modules (`warn,platynui=<level>`); `error` and `off` apply
//! to every module. A value that is rejected counts as absent, so the next
//! source applies, and is reported once per process. See `dev-docs/logging.md`.

use std::collections::HashSet;
use std::fmt;
use std::sync::{LazyLock, Mutex, PoisonError};

use tracing_subscriber::EnvFilter;
pub use tracing_subscriber::filter::LevelFilter;

// Used by the `log_records` integration test only.
#[cfg(test)]
use log as _;

/// The level names [`parse_level`] accepts, as its error lists them.
pub const LEVEL_NAMES: &str = "off, error, warn (warning), info, debug, trace, critical, fatal";

/// A level name that [`parse_level`] does not know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownLevel(String);

impl fmt::Display for UnknownLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown log level '{}'; expected one of {LEVEL_NAMES}", self.0)
    }
}

impl std::error::Error for UnknownLevel {}

/// Parses a single level, case-insensitively.
///
/// Accepts `off`, `error`, `warn`, `info`, `debug` and `trace`, the Python
/// spelling `warning` (warn), and `critical` and `fatal` (error). Serves
/// directly as a clap `value_parser`.
///
/// # Errors
///
/// [`UnknownLevel`] for anything else, filter directives included.
pub fn parse_level(value: &str) -> Result<LevelFilter, UnknownLevel> {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" => Ok(LevelFilter::OFF),
        "error" | "critical" | "fatal" => Ok(LevelFilter::ERROR),
        "warn" | "warning" => Ok(LevelFilter::WARN),
        "info" => Ok(LevelFilter::INFO),
        "debug" => Ok(LevelFilter::DEBUG),
        "trace" => Ok(LevelFilter::TRACE),
        _ => Err(UnknownLevel(value.to_owned())),
    }
}

/// The directives for a single level: `PlatynUI`'s modules only, except for
/// `error` and `off`, which apply to every module.
#[must_use]
pub fn level_directives(level: LevelFilter) -> String {
    let name = level.to_string().to_ascii_lowercase();
    if level <= LevelFilter::ERROR { name } else { format!("warn,platynui={name}") }
}

/// An environment value that was rejected and skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    /// The environment variable.
    pub variable: &'static str,
    /// Its value.
    pub value: String,
    reason: String,
}

impl Rejection {
    /// Why the value was rejected.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={:?} is ignored: {}", self.variable, self.value, self.reason)
    }
}

/// The filter directives to install, and the rejected values not reported before.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterSpec {
    /// Directives in `EnvFilter` syntax.
    pub directives: String,
    /// Rejected environment values that this process has not reported yet.
    /// Each (variable, value) pair appears once per process, because an entry
    /// point may rebuild the filter many times.
    pub rejected: Vec<Rejection>,
}

impl FilterSpec {
    /// The filter for these directives.
    #[must_use]
    pub fn env_filter(&self) -> EnvFilter {
        EnvFilter::builder().parse_lossy(&self.directives)
    }
}

/// The (variable, value) pairs this process has reported.
static REPORTED: LazyLock<Mutex<HashSet<(&'static str, String)>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Builds the filter from the environment (`env` looks up a variable) and the
/// level the entry point was given.
pub fn filter_spec(env: impl Fn(&str) -> Option<String>, requested: Option<LevelFilter>) -> FilterSpec {
    filter_spec_reporting(env, requested, &REPORTED)
}

fn filter_spec_reporting(
    env: impl Fn(&str) -> Option<String>,
    requested: Option<LevelFilter>,
    reported: &Mutex<HashSet<(&'static str, String)>>,
) -> FilterSpec {
    let mut rejected = Vec::new();
    let mut reject = |variable: &'static str, value: String, reason: String| {
        let first = reported.lock().unwrap_or_else(PoisonError::into_inner).insert((variable, value.clone()));
        if first {
            rejected.push(Rejection { variable, value, reason });
        }
    };
    let present = |variable: &str| env(variable).filter(|value| !value.trim().is_empty());

    let from_rust_log = present("RUST_LOG").and_then(|value| match EnvFilter::builder().parse(&value) {
        Ok(_) => Some(value),
        Err(err) => {
            reject("RUST_LOG", value, format!("not a valid filter ({err})"));
            None
        }
    });
    let directives = from_rust_log
        .or_else(|| requested.map(level_directives))
        .or_else(|| {
            present("PLATYNUI_LOG_LEVEL").and_then(|value| {
                if let Ok(level) = parse_level(&value) {
                    Some(level_directives(level))
                } else {
                    let reason =
                        format!("not a single log level (one of {LEVEL_NAMES}); filter directives belong in RUST_LOG");
                    reject("PLATYNUI_LOG_LEVEL", value, reason);
                    None
                }
            })
        })
        .unwrap_or_else(|| "warn".to_owned());
    FilterSpec { directives, rejected }
}

/// Installs a `fmt` subscriber that writes to stderr, filtered as this crate
/// describes, and reports rejected environment values through it.
///
/// Never panics: when a subscriber is already installed, it says so on
/// stderr. With the `log` feature, `log` records pass through the same filter.
#[cfg(feature = "fmt")]
pub fn init_stderr(requested: Option<LevelFilter>) {
    init_with_writer(requested, std::io::stderr);
}

/// [`init_stderr`] with another writer, for tests.
#[cfg(feature = "fmt")]
#[doc(hidden)]
pub fn init_with_writer<W>(requested: Option<LevelFilter>, make_writer: W)
where
    W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
    let spec = filter_spec(|variable| std::env::var(variable).ok(), requested);
    let installed = tracing_subscriber::fmt()
        .with_env_filter(spec.env_filter())
        .with_target(true)
        .with_writer(make_writer)
        .try_init();
    match installed {
        Ok(()) => {
            for rejection in &spec.rejected {
                tracing::warn!(
                    variable = rejection.variable,
                    value = %rejection.value,
                    reason = rejection.reason(),
                    "log setting ignored; the next source applies"
                );
            }
        }
        Err(err) => {
            eprintln!("platynui: logging is not set up: {err}");
            for rejection in &spec.rejected {
                eprintln!("platynui: {rejection}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    use tracing::Subscriber;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned())
    }

    fn spec(pairs: &'static [(&'static str, &'static str)], requested: Option<LevelFilter>) -> FilterSpec {
        filter_spec_reporting(env(pairs), requested, &Mutex::new(HashSet::new()))
    }

    fn rejected(spec: &FilterSpec) -> Vec<(&'static str, &str)> {
        spec.rejected.iter().map(|r| (r.variable, r.value.as_str())).collect()
    }

    #[test]
    fn levels_are_case_insensitive_and_know_the_python_spellings() {
        assert_eq!(parse_level("DEBUG").unwrap(), LevelFilter::DEBUG);
        assert_eq!(parse_level("Trace").unwrap(), LevelFilter::TRACE);
        assert_eq!(parse_level("info").unwrap(), LevelFilter::INFO);
        assert_eq!(parse_level("off").unwrap(), LevelFilter::OFF);
        assert_eq!(parse_level("warn").unwrap(), LevelFilter::WARN);
        assert_eq!(parse_level("WARNING").unwrap(), LevelFilter::WARN);
        assert_eq!(parse_level("error").unwrap(), LevelFilter::ERROR);
        assert_eq!(parse_level("critical").unwrap(), LevelFilter::ERROR);
        assert_eq!(parse_level("Fatal").unwrap(), LevelFilter::ERROR);
    }

    #[test]
    fn an_unknown_level_is_an_error_that_lists_the_accepted_names() {
        let err = parse_level("verbose").unwrap_err();
        assert_eq!(
            err.to_string(),
            "unknown log level 'verbose'; expected one of off, error, warn (warning), info, debug, trace, critical, fatal"
        );
        let _: &dyn std::error::Error = &err;
        assert!(parse_level("platynui=debug").is_err());
        assert!(parse_level("").is_err());
    }

    #[test]
    fn rust_log_is_used_verbatim() {
        assert_eq!(spec(&[("RUST_LOG", "zbus")], None).directives, "zbus");
        let s = spec(&[("RUST_LOG", "platynui_runtime=trace,warn")], Some(LevelFilter::DEBUG));
        assert_eq!(s.directives, "platynui_runtime=trace,warn");
        assert!(s.rejected.is_empty());
    }

    #[test]
    fn a_rust_log_that_does_not_parse_is_rejected_and_falls_through() {
        let s = spec(&[("RUST_LOG", "zbus=loud")], Some(LevelFilter::DEBUG));
        assert_eq!(s.directives, "warn,platynui=debug");
        assert_eq!(rejected(&s), [("RUST_LOG", "zbus=loud")]);
        assert!(s.rejected[0].to_string().contains("RUST_LOG"), "{}", s.rejected[0]);
        assert!(s.rejected[0].to_string().contains("zbus=loud"), "{}", s.rejected[0]);
    }

    #[test]
    fn blank_variables_count_as_absent_without_a_report() {
        let s = spec(&[("RUST_LOG", ""), ("PLATYNUI_LOG_LEVEL", "  ")], None);
        assert_eq!(s.directives, "warn");
        assert!(s.rejected.is_empty());
    }

    #[test]
    fn a_requested_level_lowers_platynui_only_and_error_and_off_apply_everywhere() {
        assert_eq!(spec(&[], Some(LevelFilter::DEBUG)).directives, "warn,platynui=debug");
        assert_eq!(spec(&[], Some(LevelFilter::WARN)).directives, "warn,platynui=warn");
        assert_eq!(spec(&[], Some(LevelFilter::ERROR)).directives, "error");
        assert_eq!(spec(&[], Some(LevelFilter::OFF)).directives, "off");
    }

    #[test]
    fn the_environment_level_means_the_same_as_a_requested_one() {
        assert_eq!(spec(&[("PLATYNUI_LOG_LEVEL", "debug")], None).directives, "warn,platynui=debug");
        assert_eq!(spec(&[("PLATYNUI_LOG_LEVEL", "WARNING")], None).directives, "warn,platynui=warn");
        assert_eq!(spec(&[("PLATYNUI_LOG_LEVEL", "off")], None).directives, "off");
    }

    #[test]
    fn directives_and_unknown_words_in_the_environment_level_are_rejected() {
        for value in ["platynui_runtime=trace", "verbose"] {
            let pairs: &'static [(&'static str, &'static str)] =
                Box::leak(vec![("PLATYNUI_LOG_LEVEL", value)].into_boxed_slice());
            let s = spec(pairs, None);
            assert_eq!(s.directives, "warn", "{value}");
            assert_eq!(rejected(&s), [("PLATYNUI_LOG_LEVEL", value)]);
            let message = s.rejected[0].to_string();
            assert!(message.contains("PLATYNUI_LOG_LEVEL") && message.contains(value), "{message}");
            assert!(message.contains("RUST_LOG"), "the rejection points to RUST_LOG for directives: {message}");
        }
    }

    #[test]
    fn the_sources_keep_their_precedence() {
        let debug = Some(LevelFilter::DEBUG);
        assert_eq!(spec(&[("RUST_LOG", "zbus=debug"), ("PLATYNUI_LOG_LEVEL", "info")], debug).directives, "zbus=debug");
        assert_eq!(spec(&[("PLATYNUI_LOG_LEVEL", "info")], debug).directives, "warn,platynui=debug");
        assert_eq!(spec(&[("PLATYNUI_LOG_LEVEL", "info")], None).directives, "warn,platynui=info");
    }

    #[test]
    fn nothing_set_means_warn() {
        let s = spec(&[], None);
        assert_eq!(s.directives, "warn");
        assert!(s.rejected.is_empty());
    }

    #[test]
    fn a_rejected_pair_is_reported_once() {
        let reported = Mutex::new(HashSet::new());
        let pairs: &'static [(&'static str, &'static str)] = &[("PLATYNUI_LOG_LEVEL", "verbose")];
        let first = filter_spec_reporting(env(pairs), None, &reported);
        let second = filter_spec_reporting(env(pairs), Some(LevelFilter::DEBUG), &reported);
        assert_eq!(rejected(&first), [("PLATYNUI_LOG_LEVEL", "verbose")]);
        assert!(second.rejected.is_empty(), "the same pair is not reported again");
        assert_eq!(second.directives, "warn,platynui=debug", "but it is still skipped");
        let other = filter_spec_reporting(env(&[("PLATYNUI_LOG_LEVEL", "loud")]), None, &reported);
        assert_eq!(rejected(&other), [("PLATYNUI_LOG_LEVEL", "loud")], "another value is reported");
    }

    #[test]
    fn the_process_wide_report_happens_once() {
        let pairs: &'static [(&'static str, &'static str)] = &[("PLATYNUI_LOG_LEVEL", "only-once-per-process")];
        assert_eq!(filter_spec(env(pairs), None).rejected.len(), 1);
        assert!(filter_spec(env(pairs), None).rejected.is_empty());
    }

    /// Collects the targets of the events that pass the filter.
    #[derive(Clone, Default)]
    struct Seen(Arc<Mutex<Vec<String>>>);

    impl<S: Subscriber> Layer<S> for Seen {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            self.0.lock().unwrap().push(format!("{} {}", event.metadata().level(), event.metadata().target()));
        }
    }

    fn seen_with(directives: &str, emit: impl FnOnce()) -> Vec<String> {
        let seen = Seen::default();
        let subscriber = tracing_subscriber::registry().with(EnvFilter::new(directives)).with(seen.clone());
        tracing::subscriber::with_default(subscriber, emit);
        seen.0.lock().unwrap().clone()
    }

    #[test]
    fn a_single_level_opens_platynui_and_not_third_party_modules() {
        let directives = spec(&[], Some(LevelFilter::DEBUG)).directives;
        let events = seen_with(&directives, || {
            tracing::debug!(target: "platynui_provider_atspi::node", "ours");
            tracing::debug!(target: "zbus::connection", "theirs");
            tracing::warn!(target: "zbus::connection", "their warning");
        });
        assert_eq!(events, ["DEBUG platynui_provider_atspi::node", "WARN zbus::connection"]);
    }

    #[test]
    fn error_hides_third_party_warnings() {
        let directives = spec(&[], Some(LevelFilter::ERROR)).directives;
        let events = seen_with(&directives, || {
            tracing::warn!(target: "zbus::connection", "their warning");
            tracing::warn!(target: "platynui_runtime", "our warning");
            tracing::error!(target: "zbus::connection", "their error");
        });
        assert_eq!(events, ["ERROR zbus::connection"]);
    }
}
