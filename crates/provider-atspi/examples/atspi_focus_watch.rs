//! `atspi_focus_watch` — behave like a screen reader: register for AT-SPI events
//! and print each one with its source's role/name and full **parent chain** up to
//! the application root.
//!
//! Purpose: transient popups (a Qt context menu) do not appear in a cold
//! top-down `GetChildren` tree walk, yet Orca reads them. This tool shows *how*:
//! the toolkit delivers them via **events** (children-changed on open,
//! state-changed:showing/focused, focus, selection-changed). If an event's source
//! (or a children-changed `child`) walks back up to the application through
//! `parent()`, that is the hook by which the picker/provider could graft the
//! popup into our own tree.
//!
//! Run inside a session whose `AT_SPI_BUS_ADDRESS` points at the a11y bus:
//!   cargo run -p platynui-provider-atspi --example atspi_focus_watch
//! It streams until killed.

use std::io::Write;

use atspi_common::events::{Event, EventProperties, EventTypeProperties, FocusEvents, ObjectEvents, WindowEvents};
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use futures_lite::StreamExt;
use zbus::Address;
use zbus::proxy::CacheProperties;

const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
const NULL_PATH: &str = "/org/a11y/atspi/null";

fn main() {
    zbus::block_on(run());
}

async fn build<'a>(conn: &'a zbus::Connection, dest: &str, path: &str) -> Option<AccessibleProxy<'a>> {
    AccessibleProxy::builder(conn)
        .cache_properties(CacheProperties::No)
        .destination(dest.to_owned())
        .ok()?
        .path(path.to_owned())
        .ok()?
        .build()
        .await
        .ok()
}

/// `Role 'Name'` for one accessible.
async fn label(conn: &zbus::Connection, dest: &str, path: &str) -> String {
    match build(conn, dest, path).await {
        Some(p) => {
            let role = p.get_role().await.map(|r| format!("{r:?}")).unwrap_or_else(|_| "?".to_owned());
            let name = p.name().await.unwrap_or_default();
            format!("{role} '{name}'")
        }
        None => "<gone>".to_owned(),
    }
}

/// Recursive `getChildren` dump DOWNWARD from a node — to check whether a
/// transient popup's items are reachable top-down from the popup itself (even
/// though the popup is not in its parent's `getChildren`).
fn subtree<'a>(
    conn: &'a zbus::Connection,
    dest: String,
    path: String,
    depth: usize,
    lines: &'a mut Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        lines.push(format!("{}{}", "  ".repeat(depth), label(conn, &dest, &path).await));
        if depth >= 6 {
            return;
        }
        if let Some(proxy) = build(conn, &dest, &path).await
            && let Ok(children) = proxy.get_children().await
        {
            for c in children {
                if let Some(cd) = c.name_as_str() {
                    subtree(conn, cd.to_owned(), c.path_as_str().to_owned(), depth + 1, lines).await;
                }
            }
        }
    })
}

/// `Role 'Name'  <-  Parent  <-  ...  <-  Application` via `parent()`.
async fn chain(conn: &zbus::Connection, dest: &str, path: &str) -> String {
    let mut parts = vec![label(conn, dest, path).await];
    let (mut d, mut p) = (dest.to_owned(), path.to_owned());
    for _ in 0..15 {
        let Some(proxy) = build(conn, &d, &p).await else { break };
        let Ok(parent) = proxy.parent().await else { break };
        let Some(pd) = parent.name_as_str().map(str::to_owned) else { break };
        let pp = parent.path_as_str().to_owned();
        if pp == NULL_PATH || pp.is_empty() {
            break;
        }
        parts.push(label(conn, &pd, &pp).await);
        if pp == ROOT_PATH {
            break;
        }
        d = pd;
        p = pp;
    }
    parts.join("  <-  ")
}

async fn connect() -> AccessibilityConnection {
    match std::env::var("AT_SPI_BUS_ADDRESS") {
        Ok(addr) if !addr.is_empty() => {
            AccessibilityConnection::from_address(addr.parse::<Address>().expect("addr")).await
        }
        _ => AccessibilityConnection::new().await,
    }
    .expect("connect to a11y bus")
}

async fn run() {
    // Optional first argument: only show events whose parent chain contains this
    // substring (e.g. an app id / window name), to cut noise on a busy desktop.
    let filter = std::env::args().nth(1);

    // TWO connections on purpose. The event stream is consumed on `events_conn`;
    // resolving each event's role/name/parent-chain is done on a SEPARATE
    // `query_conn`. Making synchronous proxy calls on the *same* connection whose
    // stream we are awaiting deadlocks the stream (no events are then delivered) —
    // the bug this tool originally had.
    let events_conn = connect().await;
    events_conn.register_event::<ObjectEvents>().await.expect("register object events");
    events_conn.register_event::<WindowEvents>().await.expect("register window events");
    events_conn.register_event::<FocusEvents>().await.expect("register focus events");
    let query = connect().await;
    let qconn = query.connection();

    let mut out = std::io::stdout();
    let _ = writeln!(out, "atspi_focus_watch: subscribed; filter={filter:?}. Streaming until killed.");
    let _ = out.flush();

    let events = events_conn.event_stream();
    let mut events = std::pin::pin!(events);
    while let Some(res) = events.next().await {
        let Ok(ev) = res else { continue };
        // Curate to navigation-relevant events; skip high-frequency noise
        // (property/bounds/text/caret changes) that would flood a real desktop.
        let (detail, child) = match &ev {
            Event::Object(ObjectEvents::StateChanged(e)) => {
                let s = format!("{:?}", e.state);
                if !matches!(s.as_str(), "Showing" | "Focused" | "Selected" | "Active") {
                    continue;
                }
                (format!("state:{s}={}", e.enabled), None)
            }
            Event::Object(ObjectEvents::ChildrenChanged(e)) => {
                let child = (e.child.name_as_str().unwrap_or_default().to_owned(), e.child.path_as_str().to_owned());
                (format!("children:{:?}", e.operation), Some(child))
            }
            Event::Object(ObjectEvents::ActiveDescendantChanged(_)) => ("active-descendant".to_owned(), None),
            Event::Object(ObjectEvents::SelectionChanged(_)) => ("selection".to_owned(), None),
            Event::Focus(_) => ("focus".to_owned(), None),
            Event::Window(w) => (format!("window:{}", w.member()), None),
            _ => continue,
        };
        let obj = ev.object_ref();
        let dest = obj.name().map(|n| n.to_string()).unwrap_or_default();
        let path = obj.path().to_string();
        let src_chain = chain(qconn, &dest, &path).await;
        let child_chain = match &child {
            Some((cd, cp)) => Some(chain(qconn, cd, cp).await),
            None => None,
        };
        if let Some(f) = &filter
            && !src_chain.contains(f.as_str())
            && !child_chain.as_deref().is_some_and(|c| c.contains(f.as_str()))
        {
            continue;
        }
        match &child_chain {
            Some(cc) => {
                let _ = writeln!(out, "{detail}\n    on:    {src_chain}\n    child: {cc}");
            }
            None => {
                let _ = writeln!(out, "{detail}  {src_chain}");
            }
        }
        // For a shown/added popup-ish node, dump its subtree DOWNWARD to see
        // whether its items are reachable top-down from the popup itself.
        let target = child.as_ref().map_or((dest.as_str(), path.as_str()), |(d, p)| (d.as_str(), p.as_str()));
        let target_chain = child_chain.as_deref().unwrap_or(&src_chain);
        let is_popupish = ["PopupMenu", "Menu", "Window", "Dialog", "ToolTip", "Combo"]
            .iter()
            .any(|k| target_chain.split("  <-  ").next().is_some_and(|first| first.contains(k)));
        if is_popupish && matches!(detail.as_str(), d if d.contains("Showing=true") || d.contains("children:Insert")) {
            let mut lines = Vec::new();
            subtree(qconn, target.0.to_owned(), target.1.to_owned(), 0, &mut lines).await;
            let _ = writeln!(out, "    subtree-down:\n        {}", lines.join("\n        "));
        }
        let _ = out.flush();
    }
}
