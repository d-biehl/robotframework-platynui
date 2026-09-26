//! Unified timeout helpers for blocking on async D-Bus futures.
//!
//! Every async D-Bus call in this crate runs through [`block_on_timeout`] which
//! polls the future in a loop with a deadline enforced by
//! [`std::thread::park_timeout`].  Three pre-defined durations cover the
//! typical call-site categories:
//!
//! | Constant            | Duration | Use case                                   |
//! |---------------------|----------|--------------------------------------------|
//! | [`TIMEOUT_CALL`]    | 1 s      | Per-node property reads during tree walks  |
//! | [`TIMEOUT_INIT`]    | 5 s      | One-off calls during provider startup      |
//! | [`TIMEOUT_CONNECT`] | 10 s     | A11y bus connection establishment           |
//!
//! How a timeout is logged depends on who pays for it:
//!
//! - A call to an application whose timeout is swallowed goes through
//!   [`block_on_timeout_call`] or [`block_on_app_call`], keyed by the
//!   application's bus name in the provider's [`AppTimeouts`]: the first timeout
//!   of an application warns, naming it, the further ones are debug.
//! - A call without an application subject (a proxy build, the registry, the
//!   bus daemon) goes through [`block_on_timeout_unlatched`] (and
//!   [`block_on_timeout_init`]), which record a timeout at debug, whether the
//!   caller swallows or returns it.
//! - Any other caller that returns its timeout uses [`block_on_timeout`] or
//!   [`block_on_timeout_connect`], which log nothing: the layer that swallows
//!   the returned failure reports it.

use std::collections::HashMap;
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Wake};
use std::time::{Duration, Instant};

use platynui_core::diagnostics::{Episode, Transitions};
use tracing::{debug, warn};

/// Timeout for individual D-Bus property reads (per-node calls).
pub(crate) const TIMEOUT_CALL: Duration = Duration::from_secs(1);

/// Timeout for one-off D-Bus calls during provider initialisation (e.g.
/// building the registry proxy, fetching the application list).
pub(crate) const TIMEOUT_INIT: Duration = Duration::from_secs(5);

/// Generous timeout for the initial accessibility bus connection.
pub(crate) const TIMEOUT_CONNECT: Duration = Duration::from_secs(10);

/// Waker that unparks a specific thread.
struct ThreadWake(std::thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Execute a future with a timeout.
///
/// Returns `Some(output)` on success or `None` if the future does not complete
/// within `timeout`. Logs nothing: the caller decides what a timeout means.
///
/// The timeout is enforced by [`std::thread::park_timeout`], making it
/// independent of any async reactor.
pub(crate) fn block_on_timeout<F: Future>(future: F, timeout: Duration) -> Option<F::Output> {
    let start = Instant::now();
    let waker = Arc::new(ThreadWake(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(val) => return Some(val),
            std::task::Poll::Pending => {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return None;
                }
                std::thread::park_timeout(timeout.checked_sub(elapsed).unwrap());
            }
        }
    }
}

/// A timeout's milliseconds as a log field. Call timeouts are seconds, so
/// their milliseconds fit in `u64`.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn millis(timeout: Duration) -> u64 {
    timeout.as_millis() as u64
}

/// [`block_on_timeout`] for a call that has no application as its subject, such
/// as a proxy build or a call to the registry or the bus daemon: a timeout is
/// recorded at debug, naming the call.
pub(crate) fn block_on_timeout_unlatched<F: Future>(
    call: &'static str,
    timeout: Duration,
    future: F,
) -> Option<F::Output> {
    let output = block_on_timeout(future, timeout);
    if output.is_none() {
        debug!(call, timeout_ms = millis(timeout), "AT-SPI call timed out");
    }
    output
}

/// Convenience wrapper: [`block_on_timeout_unlatched`] with [`TIMEOUT_INIT`] (5 s).
///
/// Use this for one-off D-Bus calls to the registry (registry proxy,
/// application list).
#[inline]
pub(crate) fn block_on_timeout_init<F: Future>(call: &'static str, future: F) -> Option<F::Output> {
    block_on_timeout_unlatched(call, TIMEOUT_INIT, future)
}

/// Convenience wrapper: [`block_on_timeout`] with [`TIMEOUT_CONNECT`] (10 s).
///
/// Use this for the initial accessibility bus connection. It logs nothing,
/// because the connection code returns the timeout.
#[inline]
pub(crate) fn block_on_timeout_connect<F: Future>(future: F) -> Option<F::Output> {
    block_on_timeout(future, TIMEOUT_CONNECT)
}

/// What a caller loses when an application's call times out, as the warning
/// states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Loss {
    /// A value, a state or a child list of the application's elements.
    Values,
    /// The whole application: enumeration leaves it out.
    Application,
}

/// What enumeration learned about the application behind a bus name, so that
/// its timeout warning names more than `:1.57`.
#[derive(Clone, Debug, Default)]
struct Application {
    name: Option<String>,
    pid: Option<u32>,
}

/// The applications of one provider whose calls time out, keyed by bus name.
///
/// One episode is one application instance. A successful call does not end
/// it: an application's calls have independent budgets, so one slow handler
/// times out while the next call answers, and re-arming on success would warn
/// on every poll. D-Bus unique names are not reused, so [`Self::retain`]
/// against the registry's applications at each enumeration ends the episode
/// of an instance that has gone, and a restarted application warns again.
#[derive(Default)]
pub(crate) struct AppTimeouts {
    latch: Transitions<String>,
    applications: Mutex<HashMap<String, Application>>,
}

impl AppTimeouts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Remember what enumeration learned about the application behind
    /// `bus_name`. A part that is `None` keeps what was learned before.
    pub(crate) fn learned(&self, bus_name: &str, name: Option<&str>, pid: Option<u32>) {
        let mut applications = self.applications.lock().unwrap_or_else(PoisonError::into_inner);
        let application = applications.entry(bus_name.to_owned()).or_default();
        if let Some(name) = name {
            application.name = Some(name.to_owned());
        }
        if pid.is_some() {
            application.pid = pid;
        }
    }

    /// Forget every application for which `registered` returns `false`, and
    /// record the end of the timeout episode of each forgotten one at debug.
    pub(crate) fn retain(&self, registered: impl Fn(&str) -> bool) {
        let ended = self.latch.retain(|bus_name| registered(bus_name));
        let mut applications = self.applications.lock().unwrap_or_else(PoisonError::into_inner);
        for bus_name in &ended {
            let application = applications.get(bus_name).cloned().unwrap_or_default();
            debug!(
                application = application.name.as_deref(),
                pid = application.pid,
                bus_name = %bus_name,
                "application is no longer registered on the accessibility bus; its timeout episode ends"
            );
        }
        applications.retain(|bus_name, _| registered(bus_name));
    }

    /// Record a timeout of `call` to the application behind `bus_name`: a
    /// warning when it starts the application's episode, debug after that.
    fn timed_out(&self, bus_name: &str, call: &'static str, timeout: Duration, loss: Loss) {
        let episode = self.latch.failed(bus_name);
        let application =
            self.applications.lock().unwrap_or_else(PoisonError::into_inner).get(bus_name).cloned().unwrap_or_default();
        let application_name = application.name.as_deref();
        let timeout_ms = millis(timeout);
        match (episode, loss) {
            (Episode::Started, Loss::Values) => warn!(
                application = application_name,
                pid = application.pid,
                bus_name = %bus_name,
                call,
                timeout_ms,
                "application does not answer AT-SPI calls in time; its elements may be missing or incomplete in \
                 query results"
            ),
            (Episode::Started, Loss::Application) => warn!(
                application = application_name,
                pid = application.pid,
                bus_name = %bus_name,
                call,
                timeout_ms,
                "application does not answer AT-SPI calls in time; its elements are missing from query results"
            ),
            (Episode::Continuing, Loss::Values) => debug!(
                application = application_name,
                pid = application.pid,
                bus_name = %bus_name,
                call,
                timeout_ms,
                "application still does not answer AT-SPI calls in time"
            ),
            (Episode::Continuing, Loss::Application) => debug!(
                application = application_name,
                pid = application.pid,
                bus_name = %bus_name,
                call,
                timeout_ms,
                "application still does not answer AT-SPI calls in time; it is left out of this enumeration"
            ),
        }
    }
}

/// [`block_on_timeout`] for `call` to the application behind `bus_name`, whose
/// timeout the caller swallows: the application's first timeout warns, naming
/// it and saying what the caller loses (`loss`); later ones are debug. A call
/// that answers, with a value or an error, logs nothing and does not end the
/// episode.
pub(crate) fn block_on_app_call<F: Future>(
    timeouts: &AppTimeouts,
    bus_name: &str,
    call: &'static str,
    timeout: Duration,
    loss: Loss,
    future: F,
) -> Option<F::Output> {
    let output = block_on_timeout(future, timeout);
    if output.is_none() {
        timeouts.timed_out(bus_name, call, timeout, loss);
    }
    output
}

/// Convenience wrapper: [`block_on_app_call`] with [`TIMEOUT_CALL`] (1 s), for
/// a per-node read whose value is missing when it times out.
///
/// Use this for regular per-node D-Bus property reads during tree evaluation.
#[inline]
pub(crate) fn block_on_timeout_call<F: Future>(
    timeouts: &AppTimeouts,
    bus_name: &str,
    call: &'static str,
    future: F,
) -> Option<F::Output> {
    block_on_app_call(timeouts, bus_name, call, TIMEOUT_CALL, Loss::Values, future)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_log::{at_level, logged, warnings};
    use std::future::{pending, ready};

    // Spec: *An application that stops answering is named once*.

    const SHORT: Duration = Duration::from_millis(10);

    fn time_out(timeouts: &AppTimeouts, bus_name: &str) -> Option<()> {
        block_on_app_call(timeouts, bus_name, "Accessible.GetState", SHORT, Loss::Values, pending::<()>())
    }

    fn gedit() -> AppTimeouts {
        let timeouts = AppTimeouts::new();
        timeouts.learned(":1.7", Some("gedit"), Some(4711));
        timeouts
    }

    #[test]
    fn an_application_that_keeps_timing_out_is_warned_about_once_by_name() {
        let timeouts = gedit();
        let (results, log) = logged(|| (0..10).map(|_| time_out(&timeouts, ":1.7")).collect::<Vec<_>>());

        assert!(results.iter().all(Option::is_none), "every call timed out");
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in [
            "application=\"gedit\"",
            "pid=4711",
            "bus_name=:1.7",
            "call=\"Accessible.GetState\"",
            "timeout_ms=10",
            "missing or incomplete in query results",
        ] {
            assert!(warnings[0].contains(expected), "the warning names `{expected}`: {}", warnings[0]);
        }
        assert_eq!(at_level(&log, "DEBUG").len(), 9, "the further timeouts are debug: {log}");
    }

    #[test]
    fn neither_an_answer_nor_an_error_ends_the_episode() {
        let timeouts = gedit();
        let (answers, log) = logged(|| {
            time_out(&timeouts, ":1.7");
            let value = block_on_timeout_call(&timeouts, ":1.7", "Accessible.Name", ready(Ok::<_, ()>("gedit")));
            time_out(&timeouts, ":1.7");
            let error =
                block_on_timeout_call(&timeouts, ":1.7", "Accessible.Name", ready(Err::<(), _>("no such object")));
            time_out(&timeouts, ":1.7");
            (value, error)
        });

        assert_eq!(answers, (Some(Ok("gedit")), Some(Err("no such object"))));
        assert_eq!(warnings(&log).len(), 1, "{log}");
        assert_eq!(at_level(&log, "DEBUG").len(), 2, "{log}");
    }

    #[test]
    fn each_application_has_its_own_episode() {
        let timeouts = gedit();
        let ((), log) = logged(|| {
            time_out(&timeouts, ":1.7");
            time_out(&timeouts, ":1.8");
            time_out(&timeouts, ":1.7");
        });

        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 2, "{log}");
        assert!(warnings[0].contains(":1.7") && warnings[1].contains(":1.8"), "{log}");
        assert!(!warnings[1].contains("application="), "an application enumeration has not named yet: {log}");
    }

    #[test]
    fn an_application_that_left_the_bus_ends_its_episode_and_a_new_instance_warns_again() {
        let timeouts = gedit();
        let ((), log) = logged(|| {
            time_out(&timeouts, ":1.7");
            timeouts.retain(|bus_name| bus_name == ":1.8");
            time_out(&timeouts, ":1.7");
        });

        let debug = at_level(&log, "DEBUG");
        assert_eq!(debug.len(), 1, "{log}");
        for expected in ["application=\"gedit\"", "bus_name=:1.7", "episode ends"] {
            assert!(debug[0].contains(expected), "the end of the episode names `{expected}`: {}", debug[0]);
        }
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 2, "{log}");
        assert!(!warnings[1].contains("gedit"), "what was learned about the instance is forgotten too: {log}");
    }

    #[test]
    fn retain_keeps_registered_applications_quietly() {
        let timeouts = gedit();
        let ((), log) = logged(|| {
            time_out(&timeouts, ":1.7");
            timeouts.retain(|bus_name| bus_name == ":1.7");
            time_out(&timeouts, ":1.7");
        });

        assert_eq!(warnings(&log).len(), 1, "{log}");
        assert!(!log.contains("episode ends"), "{log}");
        assert!(at_level(&log, "DEBUG")[0].contains("application=\"gedit\""), "{log}");
    }

    #[test]
    fn a_skipped_application_is_said_to_be_missing_from_query_results() {
        let timeouts = gedit();
        let (result, log) = logged(|| {
            block_on_app_call(&timeouts, ":1.7", "Accessible.ChildCount", SHORT, Loss::Application, pending::<()>())
        });

        assert_eq!(result, None);
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("its elements are missing from query results"), "{}", warnings[0]);
    }

    #[test]
    fn a_call_without_an_application_is_recorded_at_debug_only() {
        let (result, log) = logged(|| block_on_timeout_unlatched("Accessible.GetChildren", SHORT, pending::<()>()));

        assert_eq!(result, None);
        assert!(warnings(&log).is_empty(), "{log}");
        let debug = at_level(&log, "DEBUG");
        assert_eq!(debug.len(), 1, "{log}");
        assert!(debug[0].contains("call=\"Accessible.GetChildren\"") && debug[0].contains("timeout_ms=10"), "{log}");
    }

    #[test]
    fn a_returned_timeout_is_not_logged() {
        let (result, log) = logged(|| block_on_timeout(pending::<()>(), SHORT));

        assert_eq!(result, None);
        assert!(log.is_empty(), "the caller returns the timeout, so the helper logs nothing: {log}");
    }

    #[test]
    fn an_answer_in_time_is_returned_without_a_record() {
        let timeouts = gedit();
        let (result, log) = logged(|| block_on_timeout_call(&timeouts, ":1.7", "Accessible.Name", ready(1)));

        assert_eq!(result, Some(1));
        assert!(log.is_empty(), "{log}");
    }
}
