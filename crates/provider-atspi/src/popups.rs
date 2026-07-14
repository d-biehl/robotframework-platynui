//! Event-driven surfacing of transient popups (context menus, popup menus,
//! transient tool windows) in the AT-SPI tree.
//!
//! Some toolkits expose transient popups *asymmetrically*: the popup accessible
//! is on the bus and walks back to an in-tree owner via `parent()` (or is
//! announced by a `children-changed:add` on the owner), but the owner's own
//! `GetChildren` never lists it, so a pure top-down walk misses it. Verified
//! with the `atspi_focus_watch` example:
//!
//! - Qt context menu: `state-changed:showing=true` on a `PopupMenu` whose
//!   `parent()` is the `Application`; no `children-changed` is emitted on the
//!   Application, and its `GetChildren` never lists the popup.
//! - GTK4 context menu: `children-changed:add` on the owning widget (e.g. an
//!   entry) with the popup `Menu` as child, yet the owner's `GetChildren` stays
//!   empty (an `Invalid` intermediate node breaks the downward link).
//!
//! Toolkits also only *create* the popup accessible while an AT client is
//! registered for events (the observer effect — a screen reader sees the menu,
//! a cold tree dump does not), so the provider must keep an event registration
//! alive, not just query on demand.
//!
//! The mechanism: a background worker ([`PopupWatcher`]) owns a **dedicated**
//! event-stream connection and records `popup → owner` in a shared
//! [`PopupRegistry`]; tree enumeration merges the recorded popups into the
//! owner's children ([`PopupRegistry::merge_into`]). Everything the worker
//! resolves (roles, parents, PIDs) goes through a **second** query connection:
//! making blocking proxy calls on the connection whose stream is being awaited
//! deadlocks the stream (the bug `atspi_focus_watch` originally had).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atspi_common::events::object::{ChildrenChangedEvent, StateChangedEvent};
use atspi_common::events::{Event, ObjectEvents};
use atspi_common::{ObjectRefOwned, Operation, Role, State};
use atspi_connection::AccessibilityConnection;
use futures_lite::StreamExt;
use tracing::{debug, trace};

use crate::SELF_PID;
use crate::connection::connect_a11y_bus_with;
use crate::error::AtspiError;
use crate::node::{AtspiNode, accessible_proxy};
use crate::timeout::{block_on_timeout_call, block_on_timeout_init};

/// Roles a transient popup can carry (locked by the toolkit spike): Qt context
/// menus are `PopupMenu`, GTK4 popover menus are `Menu`; `Window`/`Dialog`/
/// `ToolTip` cover toolkits that expose popups as transient windows. Kept
/// deliberately narrow — generic containers (Qt's combo-dropdown container is a
/// bare `Panel`) must NOT match, and in-tree popups are filtered by the
/// reachability check at registration time anyway.
const POPUP_ROLES: &[Role] = &[Role::PopupMenu, Role::Menu, Role::Window, Role::Dialog, Role::ToolTip];

/// Upper bound on simultaneously recorded popups. Registrations beyond it evict
/// the oldest entry, so missed hide events cannot grow the registry unbounded
/// (stale entries are additionally pruned during [`PopupRegistry::merge_into`]).
const MAX_ENTRIES: usize = 32;

/// Per-call timeout for the worker's D-Bus resolution (role, parent, children,
/// PID). Bounds how long one unresponsive application can stall event handling.
const WORKER_CALL_TIMEOUT: Duration = Duration::from_secs(1);

/// One recorded transient popup: the popup accessible and the in-tree owner it
/// is grafted under during enumeration.
#[derive(Clone, Debug)]
pub(crate) struct PopupEntry {
    popup: ObjectRefOwned,
    owner: ObjectRefOwned,
}

/// Shared, mutex-guarded map of currently visible transient popups, written by
/// the [`PopupWatcher`] worker and read by tree enumeration.
#[derive(Default)]
pub(crate) struct PopupRegistry {
    entries: Mutex<Vec<PopupEntry>>,
}

impl PopupRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<PopupEntry>> {
        self.entries.lock().expect("popup registry mutex poisoned")
    }

    /// Record `popup` under `owner`, replacing any previous entry for the same
    /// popup (a re-shown popup may have moved to a different owner).
    pub(crate) fn insert(&self, popup: ObjectRefOwned, owner: ObjectRefOwned) {
        let mut entries = self.lock();
        entries.retain(|entry| entry.popup != popup);
        if entries.len() >= MAX_ENTRIES {
            entries.remove(0);
        }
        entries.push(PopupEntry { popup, owner });
    }

    /// Drop the entry for `popup` (hidden, removed, or defunct). Unknown popups
    /// are a no-op, so hide events for never-recorded nodes are harmless.
    pub(crate) fn remove(&self, popup: &ObjectRefOwned) {
        self.lock().retain(|entry| entry.popup != *popup);
    }

    pub(crate) fn clear(&self) {
        self.lock().clear();
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Append the recorded popups owned by `owner` to `children` (an owner's
    /// `GetChildren` result), skipping popups the toolkit already lists there.
    /// Each candidate is checked with `popup_is_live` first; dead ones (dismissed
    /// without a hide event, crashed app) are pruned from the registry instead of
    /// surfaced. With no recorded popups `children` is left exactly as-is.
    pub(crate) fn merge_into(
        &self,
        owner: &ObjectRefOwned,
        children: &mut Vec<ObjectRefOwned>,
        mut popup_is_live: impl FnMut(&ObjectRefOwned) -> bool,
    ) {
        let candidates: Vec<ObjectRefOwned> = {
            let entries = self.lock();
            entries.iter().filter(|entry| entry.owner == *owner).map(|entry| entry.popup.clone()).collect()
        };
        for popup in candidates {
            if children.contains(&popup) {
                continue;
            }
            if !popup_is_live(&popup) {
                trace!(popup = %popup.path_as_str(), "pruning dead popup from registry");
                self.remove(&popup);
                continue;
            }
            children.push(popup);
        }
    }
}

/// Whether a recorded popup is still shown on screen — the merge-time liveness
/// probe for [`PopupRegistry::merge_into`] on the synchronous query connection.
pub(crate) fn popup_is_live(conn: &AccessibilityConnection, popup: &ObjectRefOwned) -> bool {
    let Some(proxy) = accessible_proxy(conn, popup) else {
        return false;
    };
    block_on_timeout_call(proxy.get_state())
        .and_then(|result| result.ok())
        .is_some_and(|state| state.contains(State::Showing))
}

/// Background worker that keeps a [`PopupRegistry`] current from AT-SPI
/// structural events. Owns two dedicated bus connections (stream + query; see
/// the module docs for why they must be separate) on a named thread.
pub(crate) struct PopupWatcher {
    registry: Arc<PopupRegistry>,
    /// Handle to the event stream's zbus connection; closing it ends the
    /// stream, which is how [`Self::stop`] terminates the worker thread.
    events_conn: zbus::Connection,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PopupWatcher {
    /// Connect the two bus connections, register for `object:state-changed` and
    /// `object:children-changed`, and start the worker thread. Registering for
    /// events is itself load-bearing: it is what makes toolkits create popup
    /// accessibles at all (observer effect), so it must happen before a popup
    /// opens — the provider starts the watcher together with its connection.
    pub(crate) fn spawn(bus_address: Option<&str>, registry: Arc<PopupRegistry>) -> Result<Self, AtspiError> {
        let events = connect_a11y_bus_with(bus_address)?;
        let query = connect_a11y_bus_with(bus_address)?;
        block_on_timeout_init(async {
            events.register_event::<StateChangedEvent>().await?;
            events.register_event::<ChildrenChangedEvent>().await
        })
        .ok_or_else(|| AtspiError::timeout("popup event registration"))?
        .map_err(|err| AtspiError::ConnectionFailed(format!("popup event registration: {err}")))?;

        let events_conn = events.connection().clone();
        let worker_registry = Arc::clone(&registry);
        let thread = std::thread::Builder::new()
            .name("atspi-popup-watch".into())
            .spawn(move || zbus::block_on(worker_loop(events, query, worker_registry)))
            .map_err(|err| AtspiError::ConnectionFailed(format!("popup watcher thread: {err}")))?;

        Ok(Self { registry, events_conn, thread: Some(thread) })
    }

    /// Stop the worker: closing the stream's connection ends the event stream,
    /// the thread exits, and the registry is cleared so no stale popup outlives
    /// the watcher.
    pub(crate) fn stop(&mut self) {
        let _ = block_on_timeout_call(self.events_conn.clone().close());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.registry.clear();
    }
}

impl Drop for PopupWatcher {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop();
        }
    }
}

/// Await `future`, giving up after [`WORKER_CALL_TIMEOUT`]. The async
/// counterpart of [`block_on_timeout_call`] for use inside the worker.
async fn with_timeout<T>(future: impl std::future::Future<Output = T>) -> Option<T> {
    futures_lite::future::or(async { Some(future.await) }, async {
        async_io::Timer::after(WORKER_CALL_TIMEOUT).await;
        None
    })
    .await
}

async fn worker_loop(events: AccessibilityConnection, query: AccessibilityConnection, registry: Arc<PopupRegistry>) {
    // PID per application bus name; stable for a connection's lifetime, so one
    // D-Bus lookup per application suffices for the own-process filter.
    let mut pid_cache: HashMap<String, Option<u32>> = HashMap::new();
    let stream = events.event_stream();
    let mut stream = std::pin::pin!(stream);
    debug!("popup watcher: event stream running");
    while let Some(item) = stream.next().await {
        let Ok(event) = item else { continue };
        match event {
            Event::Object(ObjectEvents::StateChanged(ev)) => match ev.state {
                // A popup opening announces itself with showing=true (Qt); the
                // owner is one `parent()` hop away (verified: PopupMenu → Application).
                State::Showing if ev.enabled => {
                    on_popup_candidate(&query, &registry, &mut pid_cache, ev.item, None).await;
                }
                State::Showing => registry.remove(&ev.item),
                State::Defunct if ev.enabled => registry.remove(&ev.item),
                _ => {}
            },
            Event::Object(ObjectEvents::ChildrenChanged(ev)) => match ev.operation {
                // GTK announces popups as children-changed:add on the owning
                // widget without actually listing them in its GetChildren; the
                // event source IS the owner, no parent() hop needed.
                Operation::Insert => {
                    on_popup_candidate(&query, &registry, &mut pid_cache, ev.child, Some(ev.item)).await;
                }
                Operation::Delete => registry.remove(&ev.child),
            },
            _ => {}
        }
    }
    debug!("popup watcher: event stream ended");
}

/// Decide whether an announced accessible is a transient popup worth grafting,
/// and if so record it: popup-class role, not our own process, and NOT already
/// reachable through the owner's `GetChildren` (real windows and in-tree
/// popovers stay out — they need no graft).
async fn on_popup_candidate(
    query: &AccessibilityConnection,
    registry: &Arc<PopupRegistry>,
    pid_cache: &mut HashMap<String, Option<u32>>,
    popup: ObjectRefOwned,
    owner_hint: Option<ObjectRefOwned>,
) {
    if AtspiNode::is_null_object(&popup) {
        return;
    }
    let Some(bus_name) = popup.name_as_str().map(str::to_owned) else {
        return;
    };

    // Own-process filter, consistent with the SELF_PID skip in get_nodes.
    let pid = match pid_cache.get(&bus_name) {
        Some(pid) => *pid,
        None => {
            let resolved = resolve_pid(query, &bus_name).await;
            pid_cache.insert(bus_name.clone(), resolved);
            resolved
        }
    };
    if pid == Some(*SELF_PID) {
        return;
    }

    let Some(proxy) = accessible_proxy(query, &popup) else {
        return;
    };
    let Some(Ok(role)) = with_timeout(proxy.get_role()).await else {
        return;
    };
    if !POPUP_ROLES.contains(&role) {
        return;
    }

    let owner = match owner_hint {
        Some(owner) => owner,
        None => match with_timeout(proxy.parent()).await {
            Some(Ok(parent)) => parent,
            _ => return,
        },
    };
    if AtspiNode::is_null_object(&owner) || owner.name_as_str().is_none() {
        return;
    }

    // Reachability check: if the owner already lists the popup, a top-down walk
    // finds it without help. Only the asymmetric case needs the registry.
    let Some(owner_proxy) = accessible_proxy(query, &owner) else {
        return;
    };
    if let Some(Ok(children)) = with_timeout(owner_proxy.get_children()).await
        && children.contains(&popup)
    {
        trace!(popup = %popup.path_as_str(), ?role, "popup already reachable top-down; not grafting");
        return;
    }

    debug!(
        popup = %popup.path_as_str(),
        owner = %owner.path_as_str(),
        app = %bus_name,
        ?role,
        "recording transient popup",
    );
    registry.insert(popup, owner);
}

/// Unix PID of the application owning `bus_name`, via the bus daemon.
async fn resolve_pid(query: &AccessibilityConnection, bus_name: &str) -> Option<u32> {
    let conn = query.connection();
    with_timeout(async {
        let dbus = zbus::fdo::DBusProxy::new(conn).await.ok()?;
        dbus.get_connection_unix_process_id(zbus::names::BusName::try_from(bus_name).ok()?).await.ok()
    })
    .await
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use atspi_common::ObjectRef;
    use zbus::names::UniqueName;
    use zbus::zvariant::ObjectPath;

    fn obj(name: &'static str, path: &'static str) -> ObjectRefOwned {
        ObjectRef::new_owned(UniqueName::from_static_str_unchecked(name), ObjectPath::from_static_str_unchecked(path))
    }

    #[test]
    fn merge_appends_registered_popup_under_its_owner_only() {
        let registry = PopupRegistry::new();
        let owner = obj(":1.5", "/org/a11y/atspi/accessible/root");
        let popup = obj(":1.5", "/org/a11y/atspi/accessible/popup");
        registry.insert(popup.clone(), owner.clone());

        let mut children = vec![obj(":1.5", "/org/a11y/atspi/accessible/frame")];
        registry.merge_into(&owner, &mut children, |_| true);
        assert_eq!(children.len(), 2);
        assert_eq!(children[1], popup);

        // A different node enumerating its children is unaffected.
        let other = obj(":1.9", "/org/a11y/atspi/accessible/root");
        let mut other_children = Vec::new();
        registry.merge_into(&other, &mut other_children, |_| true);
        assert!(other_children.is_empty());
    }

    #[test]
    fn merge_dedupes_popup_the_toolkit_already_lists() {
        let registry = PopupRegistry::new();
        let owner = obj(":1.5", "/org/a11y/atspi/accessible/root");
        let popup = obj(":1.5", "/org/a11y/atspi/accessible/popup");
        registry.insert(popup.clone(), owner.clone());

        let mut children = vec![popup.clone()];
        registry.merge_into(&owner, &mut children, |_| true);
        assert_eq!(children, vec![popup]);
    }

    #[test]
    fn merge_prunes_dead_popups_from_result_and_registry() {
        let registry = PopupRegistry::new();
        let owner = obj(":1.5", "/org/a11y/atspi/accessible/root");
        let popup = obj(":1.5", "/org/a11y/atspi/accessible/popup");
        registry.insert(popup, owner.clone());

        let mut children = Vec::new();
        registry.merge_into(&owner, &mut children, |_| false);
        assert!(children.is_empty(), "a dismissed/defunct popup must not be surfaced");
        assert!(registry.is_empty(), "a dead popup must be dropped, not retried forever");
    }

    #[test]
    fn merge_with_empty_registry_leaves_children_untouched() {
        let registry = PopupRegistry::new();
        let owner = obj(":1.5", "/org/a11y/atspi/accessible/root");
        let mut children = vec![obj(":1.5", "/org/a11y/atspi/accessible/frame")];
        let before = children.clone();
        registry.merge_into(&owner, &mut children, |_| panic!("no liveness probe without recorded popups"));
        assert_eq!(children, before);
    }

    #[test]
    fn reinserting_a_popup_replaces_its_entry() {
        let registry = PopupRegistry::new();
        let popup = obj(":1.5", "/org/a11y/atspi/accessible/popup");
        let old_owner = obj(":1.5", "/org/a11y/atspi/accessible/old");
        let new_owner = obj(":1.5", "/org/a11y/atspi/accessible/new");
        registry.insert(popup.clone(), old_owner.clone());
        registry.insert(popup.clone(), new_owner.clone());

        let mut old_children = Vec::new();
        registry.merge_into(&old_owner, &mut old_children, |_| true);
        assert!(old_children.is_empty(), "the popup moved to a new owner");

        let mut new_children = Vec::new();
        registry.merge_into(&new_owner, &mut new_children, |_| true);
        assert_eq!(new_children, vec![popup]);
    }

    #[test]
    fn remove_and_clear_drop_entries() {
        let registry = PopupRegistry::new();
        let owner = obj(":1.5", "/org/a11y/atspi/accessible/root");
        let popup_a = obj(":1.5", "/org/a11y/atspi/accessible/a");
        let popup_b = obj(":1.5", "/org/a11y/atspi/accessible/b");
        registry.insert(popup_a.clone(), owner.clone());
        registry.insert(popup_b, owner.clone());

        registry.remove(&popup_a);
        let mut children = Vec::new();
        registry.merge_into(&owner, &mut children, |_| true);
        assert_eq!(children.len(), 1);

        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn insert_evicts_the_oldest_entry_beyond_the_cap() {
        let registry = PopupRegistry::new();
        let owner = obj(":1.5", "/org/a11y/atspi/accessible/root");
        let first = obj(":1.5", "/org/a11y/atspi/accessible/first");
        registry.insert(first.clone(), owner.clone());
        for i in 0..MAX_ENTRIES {
            // Distinct paths; leaked once per test run, bounded by MAX_ENTRIES.
            let path: &'static str = format!("/org/a11y/atspi/accessible/n{i}").leak();
            registry.insert(obj(":1.5", path), owner.clone());
        }
        let mut children = Vec::new();
        registry.merge_into(&owner, &mut children, |_| true);
        assert_eq!(children.len(), MAX_ENTRIES);
        assert!(!children.contains(&first), "the oldest entry is evicted first");
    }
}
