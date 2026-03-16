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

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Wake};
use std::time::{Duration, Instant};
use tracing::warn;

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
/// within `timeout`.  A `warn!` is emitted on every timeout so slow or
/// unresponsive applications are visible in logs.
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
                    warn!(
                        elapsed_ms = elapsed.as_millis() as u64,
                        timeout_ms = timeout.as_millis() as u64,
                        "D-Bus call timed out",
                    );
                    return None;
                }
                std::thread::park_timeout(timeout - elapsed);
            }
        }
    }
}

/// Convenience wrapper: [`block_on_timeout`] with [`TIMEOUT_CALL`] (1 s).
///
/// Use this for regular per-node D-Bus property reads during tree evaluation.
#[inline]
pub(crate) fn block_on_timeout_call<F: std::future::Future>(future: F) -> Option<F::Output> {
    block_on_timeout(future, TIMEOUT_CALL)
}

/// Convenience wrapper: [`block_on_timeout`] with [`TIMEOUT_INIT`] (5 s).
///
/// Use this for one-off D-Bus calls during provider startup (registry proxy,
/// application list).
#[inline]
pub(crate) fn block_on_timeout_init<F: std::future::Future>(future: F) -> Option<F::Output> {
    block_on_timeout(future, TIMEOUT_INIT)
}

/// Convenience wrapper: [`block_on_timeout`] with [`TIMEOUT_CONNECT`] (10 s).
///
/// Use this for the initial accessibility bus connection.
#[inline]
pub(crate) fn block_on_timeout_connect<F: std::future::Future>(future: F) -> Option<F::Output> {
    block_on_timeout(future, TIMEOUT_CONNECT)
}
