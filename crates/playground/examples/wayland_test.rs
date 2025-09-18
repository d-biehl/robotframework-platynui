#[cfg(target_os = "linux")]
mod linux {
    use anyhow::{Context, Result};
    use serde::Serialize;
    use std::{env, fs};

    use wayland_client::{
        globals::{registry_queue_init, GlobalListContents},
        protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
        Connection, Dispatch, Proxy, QueueHandle,
    };
    use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
        zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
    };
    use wayland_protocols_wlr::virtual_pointer::v1::client::{
        zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
        zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
    };

    #[derive(Default)]
    struct AppState;

    impl Dispatch<WlRegistry, GlobalListContents> for AppState {
        fn event(
            _state: &mut Self,
            _proxy: &WlRegistry,
            _event: wayland_client::protocol::wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlSeat, ()> for AppState {
        fn event(
            _state: &mut Self,
            _proxy: &WlSeat,
            _event: wayland_client::protocol::wl_seat::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for AppState {
        fn event(
            _state: &mut Self,
            _proxy: &ZwpVirtualKeyboardManagerV1,
            _event: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<ZwpVirtualKeyboardV1, ()> for AppState {
        fn event(
            _state: &mut Self,
            _proxy: &ZwpVirtualKeyboardV1,
            _event: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for AppState {
        fn event(
            _state: &mut Self,
            _proxy: &ZwlrVirtualPointerManagerV1,
            _event: wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<ZwlrVirtualPointerV1, ()> for AppState {
        fn event(
            _state: &mut Self,
            _proxy: &ZwlrVirtualPointerV1,
            _event: wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    #[derive(Serialize, Default)]
    struct Capability {
        present: bool,
        version: Option<u32>,
        creatable: bool,
    }

    #[derive(Serialize)]
    struct Report {
        session: SessionInfo,
        seats: usize,
        virtual_keyboard_v1: Capability,
        virtual_pointer_v1: Capability,
        fallbacks: Fallbacks,
    }

    #[derive(Serialize, Default)]
    struct SessionInfo {
        wayland_display: Option<String>,
        xdg_session_desktop: Option<String>,
        xdg_session_type: Option<String>,
    }

    #[derive(Serialize, Default)]
    struct Fallbacks {
        uinput_available: bool,
    }

    pub fn run() -> Result<()> {
        // Connect to Wayland and initialise the registry helper
        let conn = Connection::connect_to_env().context("Wayland connect failed")?;
        let (globals, mut queue) =
            registry_queue_init::<AppState>(&conn).context("registry init failed")?;
        let mut state = AppState::default();
        queue
            .roundtrip(&mut state)
            .context("initial registry roundtrip failed")?;
        let qh = queue.handle();

        // Enumerate seats; at least one seat is required for the feature checks
        let global_snapshot = globals.contents().clone_list();
        let seat_globals = global_snapshot
            .iter()
            .filter(|g| g.interface == "wl_seat")
            .cloned()
            .collect::<Vec<_>>();

        let seats: Vec<WlSeat> = seat_globals
            .iter()
            .map(|g| {
                let version = g.version.min(WlSeat::interface().version);
                globals
                    .registry()
                    .bind::<WlSeat, _, _>(g.name, version, &qh, ())
            })
            .collect();

        // Discover manager globals and remember their advertised version
        let vk_mgr_global = global_snapshot
            .iter()
            .find(|g| g.interface == "zwp_virtual_keyboard_manager_v1")
            .cloned();
        let vp_mgr_global = global_snapshot
            .iter()
            .find(|g| g.interface == "zwlr_virtual_pointer_manager_v1")
            .cloned();

        // Try to create one object per capability if a seat is available
        let mut vk_cap = Capability::default();
        if let (Some(g), Some(seat)) = (vk_mgr_global.as_ref(), seats.get(0)) {
            vk_cap.present = true;
            vk_cap.version = Some(g.version);
            let version = g
                .version
                .min(ZwpVirtualKeyboardManagerV1::interface().version);
            if let Ok(vk_mgr) = globals.bind::<ZwpVirtualKeyboardManagerV1, _, _>(&qh, 1..=version, ()) {
                let _vk = vk_mgr.create_virtual_keyboard(seat, &qh, ());
                queue
                    .roundtrip(&mut state)
                    .context("virtual keyboard roundtrip failed")?;
                vk_cap.creatable = true;
            }
        }

        let mut vp_cap = Capability::default();
        if let (Some(g), Some(seat)) = (vp_mgr_global.as_ref(), seats.get(0)) {
            vp_cap.present = true;
            vp_cap.version = Some(g.version);
            let version = g
                .version
                .min(ZwlrVirtualPointerManagerV1::interface().version);
            if let Ok(vp_mgr) = globals.bind::<ZwlrVirtualPointerManagerV1, _, _>(&qh, 1..=version, ()) {
                let _vp = vp_mgr.create_virtual_pointer(Some(seat), &qh, ());
                queue
                    .roundtrip(&mut state)
                    .context("virtual pointer roundtrip failed")?;
                vp_cap.creatable = true;
            }
        }

        // Fallbacks (kernel uinput device availability)
        let uinput_available = fs::metadata("/dev/uinput").is_ok();

        // Session environment snapshot
        let session = SessionInfo {
            wayland_display: env::var("WAYLAND_DISPLAY").ok(),
            xdg_session_desktop: env::var("XDG_SESSION_DESKTOP").ok(),
            xdg_session_type: env::var("XDG_SESSION_TYPE").ok(),
        };

        let report = Report {
            session,
            seats: seats.len(),
            virtual_keyboard_v1: vk_cap,
            virtual_pointer_v1: vp_cap,
            fallbacks: Fallbacks { uinput_available },
        };

        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    linux::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("wayland_test example is only available on Linux targets.");
}
