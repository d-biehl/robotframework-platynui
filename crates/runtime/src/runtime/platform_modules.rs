use std::sync::Mutex;

use platynui_core::platform::{
    DesktopInfoProvider, HighlightProvider, KeyboardDevice, PointerDevice, ScreenshotProvider, platform_modules,
};
use platynui_core::provider::ProviderError;

pub struct PlatformOverrides {
    pub desktop_info: Option<&'static dyn DesktopInfoProvider>,
    pub highlight: Option<&'static dyn HighlightProvider>,
    pub screenshot: Option<&'static dyn ScreenshotProvider>,
    pub pointer: Option<&'static dyn PointerDevice>,
    pub keyboard: Option<&'static dyn KeyboardDevice>,
}

#[derive(Default)]
pub(super) struct PlatformModulesState {
    pub(super) active_runtimes: usize,
}

pub(super) static PLATFORM_MODULES_STATE: Mutex<PlatformModulesState> =
    Mutex::new(PlatformModulesState { active_runtimes: 0 });

pub(super) struct PlatformModulesLease {
    active: bool,
}

impl PlatformModulesLease {
    pub(super) fn acquire() -> Result<Self, ProviderError> {
        let modules: Vec<_> = platform_modules().collect();
        let mut state = PLATFORM_MODULES_STATE.lock().expect("platform module state mutex poisoned");

        if state.active_runtimes == 0 {
            let mut initialized_modules: Vec<&'static dyn platynui_core::platform::PlatformModule> = Vec::new();
            for module in &modules {
                tracing::debug!(module = module.name(), "initializing platform module");
                if let Err(err) = module.initialize() {
                    tracing::error!(module = module.name(), %err, "platform module initialization failed");
                    for initialized in initialized_modules.into_iter().rev() {
                        tracing::debug!(module = initialized.name(), "rolling back platform module initialization");
                        initialized.shutdown();
                    }
                    return Err(ProviderError::InitializationFailed {
                        provider: "runtime",
                        details: Some(format!("platform module `{}` failed to initialize: {err}", module.name())),
                    });
                }
                initialized_modules.push(*module);
            }
        }

        state.active_runtimes += 1;
        Ok(Self { active: true })
    }

    pub(super) fn release(&mut self) {
        if !self.active {
            return;
        }

        let modules: Vec<_> = platform_modules().collect();
        let mut state = PLATFORM_MODULES_STATE.lock().expect("platform module state mutex poisoned");
        if state.active_runtimes == 0 {
            self.active = false;
            return;
        }

        state.active_runtimes -= 1;
        let should_shutdown = state.active_runtimes == 0;
        self.active = false;

        if should_shutdown {
            for module in modules {
                tracing::debug!(module = module.name(), "shutting down platform module");
                module.shutdown();
            }
        }
    }
}

impl Drop for PlatformModulesLease {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) fn platform_overrides_require_global_modules(platforms: &PlatformOverrides) -> bool {
    platforms.desktop_info.is_none()
        || platforms.highlight.is_none()
        || platforms.screenshot.is_none()
        || platforms.pointer.is_none()
        || platforms.keyboard.is_none()
}
