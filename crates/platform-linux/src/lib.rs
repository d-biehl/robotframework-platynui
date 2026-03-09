//! Linux platform mediator for `PlatynUI`.
//!
//! Detects the display session type (X11 or Wayland) at runtime and delegates
//! all platform trait calls to the appropriate sub-platform crate. The
//! sub-platform crates (`platform-linux-x11`, `platform-linux-wayland`)
//! do **not** register themselves via `inventory`; instead this mediator
//! registers a single set of wrapper devices that route at runtime.

#[cfg(target_os = "linux")]
mod session;

#[cfg(target_os = "linux")]
pub use session::{SessionType, session_type};

#[cfg(target_os = "linux")]
mod mediator {
    use crate::session::{SessionType, session_type};
    use platynui_core::platform::{
        DesktopInfo, DesktopInfoProvider, HighlightProvider, HighlightRequest, KeyCode, KeyboardDevice, KeyboardError,
        KeyboardEvent, PlatformError, PlatformModule, PointerButton, PointerDevice, Screenshot, ScreenshotProvider,
        ScreenshotRequest, ScrollDelta, WindowId, WindowManager,
    };
    use platynui_core::types::{Point, Rect, Size};
    use platynui_core::ui::UiNode;
    use platynui_core::{
        register_desktop_info_provider, register_highlight_provider, register_keyboard_device,
        register_platform_module, register_pointer_device, register_screenshot_provider, register_window_manager,
    };
    use std::sync::Mutex;
    use std::time::Duration;

    use platynui_platform_linux_x11::desktop::LinuxDesktopInfo as X11Desktop;
    use platynui_platform_linux_x11::highlight::LinuxHighlightProvider as X11Highlight;
    use platynui_platform_linux_x11::init::LinuxX11Module as X11Module;
    use platynui_platform_linux_x11::keyboard::LinuxKeyboardDevice as X11Keyboard;
    use platynui_platform_linux_x11::pointer::LinuxPointerDevice as X11Pointer;
    use platynui_platform_linux_x11::screenshot::LinuxScreenshot as X11Screenshot;
    use platynui_platform_linux_x11::window_manager::X11EwmhWindowManager as X11WindowManager;

    use platynui_platform_linux_wayland::desktop::WaylandDesktopInfo as WlDesktop;
    use platynui_platform_linux_wayland::highlight::WaylandHighlightProvider as WlHighlight;
    use platynui_platform_linux_wayland::init::WaylandModule as WlModule;
    use platynui_platform_linux_wayland::keyboard::WaylandKeyboardDevice as WlKeyboard;
    use platynui_platform_linux_wayland::pointer::WaylandPointerDevice as WlPointer;
    use platynui_platform_linux_wayland::screenshot::WaylandScreenshot as WlScreenshot;
    use platynui_platform_linux_wayland::window_manager::WaylandWindowManager as WlWindowManager;

    // -----------------------------------------------------------------------
    //  One-time session resolution
    //
    //  `Resolved` bundles trait-object references for every platform trait.
    //  It is populated once in `LinuxModule::initialize()` and then used
    //  directly by all wrapper devices — no per-call session detection.
    // -----------------------------------------------------------------------

    #[derive(Clone, Copy)]
    struct Resolved {
        module: &'static dyn PlatformModule,
        pointer: &'static dyn PointerDevice,
        keyboard: &'static dyn KeyboardDevice,
        desktop: &'static dyn DesktopInfoProvider,
        screenshot: &'static dyn ScreenshotProvider,
        highlight: &'static dyn HighlightProvider,
        window_manager: &'static dyn WindowManager,
    }

    static RESOLVED: Mutex<Option<Resolved>> = Mutex::new(None);

    /// Returns the resolved platform backends.
    ///
    /// # Panics
    ///
    /// Panics if called before `LinuxModule::initialize()` has succeeded.
    fn resolved() -> Resolved {
        RESOLVED.lock().expect("resolved platform lock poisoned").expect("Linux platform not initialized")
    }

    // -----------------------------------------------------------------------
    //  Platform Module
    // -----------------------------------------------------------------------

    struct LinuxModule;

    impl PlatformModule for LinuxModule {
        fn name(&self) -> &'static str {
            "Linux Platform"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            let session = session_type()?;

            let r = match session {
                SessionType::Wayland => {
                    tracing::info!("Wayland session detected — using Wayland platform backends");
                    Resolved {
                        module: &WlModule,
                        pointer: &WlPointer,
                        keyboard: &WlKeyboard,
                        desktop: &WlDesktop,
                        screenshot: &WlScreenshot,
                        highlight: &WlHighlight,
                        window_manager: &WlWindowManager,
                    }
                }
                SessionType::X11 => Resolved {
                    module: &X11Module,
                    pointer: &X11Pointer,
                    keyboard: &X11Keyboard,
                    desktop: &X11Desktop,
                    screenshot: &X11Screenshot,
                    highlight: &X11Highlight,
                    window_manager: &X11WindowManager,
                },
            };
            *RESOLVED.lock().expect("resolved platform lock poisoned") = Some(r);
            r.module.initialize()
        }

        fn shutdown(&self) {
            let r = RESOLVED.lock().expect("resolved platform lock poisoned");
            if let Some(r) = *r {
                r.module.shutdown();
            }
        }
    }

    static MODULE: LinuxModule = LinuxModule;
    register_platform_module!(&MODULE);

    // -----------------------------------------------------------------------
    //  Pointer Device
    // -----------------------------------------------------------------------

    struct LinuxPointer;

    impl PointerDevice for LinuxPointer {
        fn position(&self) -> Result<Point, PlatformError> {
            resolved().pointer.position()
        }

        fn move_to(&self, point: Point) -> Result<(), PlatformError> {
            resolved().pointer.move_to(point)
        }

        fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
            resolved().pointer.press(button)
        }

        fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
            resolved().pointer.release(button)
        }

        fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
            resolved().pointer.scroll(delta)
        }

        fn double_click_time(&self) -> Result<Option<Duration>, PlatformError> {
            resolved().pointer.double_click_time()
        }

        fn double_click_size(&self) -> Result<Option<Size>, PlatformError> {
            resolved().pointer.double_click_size()
        }
    }

    static POINTER: LinuxPointer = LinuxPointer;
    register_pointer_device!(&POINTER);

    // -----------------------------------------------------------------------
    //  Keyboard Device
    // -----------------------------------------------------------------------

    struct LinuxKeyboard;

    impl KeyboardDevice for LinuxKeyboard {
        fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
            resolved().keyboard.key_to_code(name)
        }

        fn start_input(&self) -> Result<(), KeyboardError> {
            resolved().keyboard.start_input()
        }

        fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
            resolved().keyboard.send_key_event(event)
        }

        fn end_input(&self) -> Result<(), KeyboardError> {
            resolved().keyboard.end_input()
        }

        fn known_key_names(&self) -> Vec<String> {
            resolved().keyboard.known_key_names()
        }
    }

    static KEYBOARD: LinuxKeyboard = LinuxKeyboard;
    register_keyboard_device!(&KEYBOARD);

    // -----------------------------------------------------------------------
    //  Desktop Info Provider
    // -----------------------------------------------------------------------

    struct LinuxDesktopInfo;

    impl DesktopInfoProvider for LinuxDesktopInfo {
        fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
            resolved().desktop.desktop_info()
        }
    }

    static DESKTOP: LinuxDesktopInfo = LinuxDesktopInfo;
    register_desktop_info_provider!(&DESKTOP);

    // -----------------------------------------------------------------------
    //  Screenshot Provider
    // -----------------------------------------------------------------------

    struct LinuxScreenshot;

    impl ScreenshotProvider for LinuxScreenshot {
        fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
            resolved().screenshot.capture(request)
        }
    }

    static SCREENSHOT: LinuxScreenshot = LinuxScreenshot;
    register_screenshot_provider!(&SCREENSHOT);

    // -----------------------------------------------------------------------
    //  Highlight Provider
    // -----------------------------------------------------------------------

    struct LinuxHighlight;

    impl HighlightProvider for LinuxHighlight {
        fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
            resolved().highlight.highlight(request)
        }

        fn clear(&self) -> Result<(), PlatformError> {
            resolved().highlight.clear()
        }
    }

    static HIGHLIGHT: LinuxHighlight = LinuxHighlight;
    register_highlight_provider!(&HIGHLIGHT);

    // -----------------------------------------------------------------------
    //  Window Manager
    // -----------------------------------------------------------------------

    struct LinuxWindowManager;

    impl WindowManager for LinuxWindowManager {
        fn name(&self) -> &'static str {
            resolved().window_manager.name()
        }

        fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
            resolved().window_manager.resolve_window(node)
        }

        fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError> {
            resolved().window_manager.bounds(id)
        }

        fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
            resolved().window_manager.is_active(id)
        }

        fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
            resolved().window_manager.activate(id)
        }

        fn close(&self, id: WindowId) -> Result<(), PlatformError> {
            resolved().window_manager.close(id)
        }

        fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
            resolved().window_manager.minimize(id)
        }

        fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
            resolved().window_manager.maximize(id)
        }

        fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
            resolved().window_manager.restore(id)
        }

        fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
            resolved().window_manager.move_to(id, position)
        }

        fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
            resolved().window_manager.resize(id, size)
        }
    }

    static WM: LinuxWindowManager = LinuxWindowManager;
    register_window_manager!(&WM);
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxPlatformStub;
