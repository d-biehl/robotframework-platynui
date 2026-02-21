//! Unified timeout helpers for blocking on async D-Bus futures.
//!
//! Every async D-Bus call in this crate runs through [`block_on_timeout`] which
//! races the future against an [`async_io::Timer`].  Three pre-defined
//! durations cover the typical call-site categories:
//!
//! | Constant            | Duration | Use case                                   |
//! |---------------------|----------|--------------------------------------------|
//! | [`TIMEOUT_CALL`]    | 1 s      | Per-node property reads during tree walks  |
//! | [`TIMEOUT_INIT`]    | 5 s      | One-off calls during provider startup      |
//! | [`TIMEOUT_CONNECT`] | 10 s     | A11y bus connection establishment           |

use futures_lite::future::block_on;
use std::time::Duration;
use tracing::warn;

/// Timeout for individual D-Bus property reads (per-node calls).
pub(crate) const TIMEOUT_CALL: Duration = Duration::from_secs(1);

/// Timeout for one-off D-Bus calls during provider initialisation (e.g.
/// building the registry proxy, fetching the application list).
pub(crate) const TIMEOUT_INIT: Duration = Duration::from_secs(5);

/// Generous timeout for the initial accessibility bus connection.
pub(crate) const TIMEOUT_CONNECT: Duration = Duration::from_secs(10);

/// Execute a future with a timeout.
///
/// Returns `Some(output)` on success or `None` if the future does not complete
/// within `timeout`.  A `warn!` is emitted on every timeout so slow or
/// unresponsive applications are visible in logs.
pub(crate) fn block_on_timeout<F: std::future::Future>(future: F, timeout: Duration) -> Option<F::Output> {
    let start = std::time::Instant::now();
    let result = block_on(async {
        futures_lite::future::or(async { Some(future.await) }, async {
            async_io::Timer::after(timeout).await;
            None
        })
        .await
    });
    if result.is_none() {
        warn!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            timeout_ms = timeout.as_millis() as u64,
            "D-Bus call timed out",
        );
    }
    result
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
