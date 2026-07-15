//! RAII ownership of JVM-side object references (`JOBJECT64`).

use crate::ffi::{JObject64, VmId};
use std::sync::mpsc;

/// Sink for deferred releases: `Drop` can run on any thread, but
/// `releaseJavaObject` may only run on the pump thread, so drops enqueue the
/// raw handle and the pump drains the queue between requests.
pub(crate) type ReleaseSender = mpsc::Sender<(VmId, JObject64)>;

/// Owning wrapper around a `(vmID, JOBJECT64)` pair obtained from the bridge.
///
/// Every context handed out by JAB must be released via `releaseJavaObject`,
/// or the **target JVM** leaks the reference. Dropping a `JabObject` enqueues
/// exactly that release. Raw handle values are meaningless for identity —
/// two lookups of the same Java object routinely return different raw values
/// (`isSameObject` is the only valid comparison; see `JabClient::is_same`).
#[derive(Debug)]
pub(crate) struct JabObject {
    vm: VmId,
    handle: JObject64,
    release: ReleaseSender,
}

impl JabObject {
    pub(crate) fn new(vm: VmId, handle: JObject64, release: ReleaseSender) -> Self {
        Self { vm, handle, release }
    }

    pub(crate) fn vm(&self) -> VmId {
        self.vm
    }

    pub(crate) fn handle(&self) -> JObject64 {
        self.handle
    }
}

impl Drop for JabObject {
    fn drop(&mut self) {
        // If the pump is already gone the JVM-side reference cannot be
        // released any more; the JVM reclaims it when the bridge connection
        // dies, so a failed send is deliberately ignored.
        let _ = self.release.send((self.vm, self.handle));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_enqueues_release() {
        let (tx, rx) = mpsc::channel();
        {
            let _obj = JabObject::new(7, 0x1234, tx);
        }
        assert_eq!(rx.try_recv().expect("release queued"), (7, 0x1234));
    }

    #[test]
    fn drop_with_closed_queue_is_silent() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let obj = JabObject::new(1, 2, tx);
        drop(obj); // must not panic
    }

    #[test]
    fn accessors_expose_raw_parts() {
        let (tx, _rx) = mpsc::channel();
        let obj = JabObject::new(42, -5, tx);
        assert_eq!(obj.vm(), 42);
        assert_eq!(obj.handle(), -5);
    }
}
