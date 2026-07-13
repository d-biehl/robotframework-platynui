//! Raw AT-SPI tree inspector: dump the accessibility tree straight from the
//! registry root over our own atspi stack, with **no provider logic** — no
//! `window_at_point`, no SELF_PID / empty-app filtering, no caching. Pure
//! `Accessible.GetChildren` recursion, so it shows exactly what the toolkits
//! publish on the bus. A small, maintained alternative to Accerciser for
//! debugging what is (or isn't) exposed — e.g. confirming that a given popup or
//! widget actually reaches AT-SPI.
//!
//! Run inside a session whose `AT_SPI_BUS_ADDRESS` points at the a11y bus:
//!   cargo run -p platynui-provider-atspi --example dump_registry_tree

use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use zbus::Address;
use zbus::proxy::CacheProperties;

const REGISTRY_BUS: &str = "org.a11y.atspi.Registry";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

fn main() {
    zbus::block_on(run());
}

async fn build_proxy<'a>(conn: &'a zbus::Connection, dest: &str, path: &str) -> Option<AccessibleProxy<'a>> {
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

async fn run() {
    let a11y = match std::env::var("AT_SPI_BUS_ADDRESS") {
        Ok(addr) if !addr.is_empty() => {
            let addr: Address = addr.parse().expect("valid AT_SPI_BUS_ADDRESS");
            AccessibilityConnection::from_address(addr).await
        }
        _ => AccessibilityConnection::new().await,
    }
    .expect("connect to a11y bus");
    let conn = a11y.connection();

    // (bus name, object path, depth); DFS via an explicit stack (no async recursion).
    let mut stack: Vec<(String, String, usize)> = vec![(REGISTRY_BUS.to_owned(), ROOT_PATH.to_owned(), 0)];
    let mut visited = 0u32;
    while let Some((dest, path, depth)) = stack.pop() {
        visited += 1;
        if visited > 8000 {
            println!("... node budget exhausted");
            break;
        }
        let Some(proxy) = build_proxy(conn, &dest, &path).await else {
            continue;
        };
        let role = proxy.get_role().await.map(|r| format!("{r:?}")).unwrap_or_else(|_| "?".to_owned());
        let name = proxy.name().await.unwrap_or_default();
        println!("{:indent$}{role} '{name}'  [{dest} {path}]", "", indent = depth * 2);
        if let Ok(children) = proxy.get_children().await {
            for child in children.into_iter().rev() {
                let Some(cdest) = child.name_as_str() else { continue };
                stack.push((cdest.to_owned(), child.path_as_str().to_owned(), depth + 1));
            }
        }
    }
    println!("--- {visited} nodes visited ---");
}
