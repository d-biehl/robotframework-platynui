//! The Java Access Bridge backend (Swing/AWT on Windows).
//!
//! A thin adapter: `platynui-provider-java-jab` already speaks the per-window
//! surface, so this maps its enumeration pass onto [`JavaBackend`] and hands
//! its nodes through untouched — `@Technology = "JAB"`, patterns, node
//! validity and RuntimeIds stay exactly what that crate produces.

use crate::backend::{Enumeration, ForeignWindows, JavaBackend, UnservedJavaWindow};
use platynui_core::config::ConfigMap;
use platynui_core::platform::WindowManager;
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;
use platynui_provider_java_jab::{JabProvider, WindowExclusions};
use std::sync::Arc;

/// Backend id and config sub-map: `providers.java.jab.*`.
pub(crate) const BACKEND_ID: &str = platynui_provider_java_jab::BACKEND_ID;

/// The router's ownership map, in the shape the JAB crate asks for. Living here
/// rather than in `backend.rs` keeps that module free of the Windows-only crate.
impl WindowExclusions for ForeignWindows {
    fn excludes(&self, window: u64) -> bool {
        self.is_foreign(window)
    }
}

pub(crate) struct JabBackend {
    inner: JabProvider,
}

impl JabBackend {
    /// Build the backend from its sub-map of the Java provider's config, plus
    /// its view of what the stronger backends serve.
    pub(crate) fn from_config(settings: Option<&ConfigMap>, foreign: Arc<ForeignWindows>) -> Self {
        Self { inner: JabProvider::from_config(settings, Some(foreign as Arc<dyn WindowExclusions>)) }
    }
}

impl JavaBackend for JabBackend {
    fn id(&self) -> &'static str {
        BACKEND_ID
    }

    fn enumerate(&self, parent: &Arc<dyn UiNode>) -> Enumeration {
        let pass = self.inner.enumerate(parent);
        Enumeration {
            served_windows: pass.served_windows,
            nodes: pass.nodes,
            unserved: pass
                .unserved
                .into_iter()
                .map(|window| UnservedJavaWindow {
                    window: window.window,
                    pid: window.pid,
                    class_name: window.class_name,
                })
                .collect(),
            java_processes: pass.java_processes,
        }
    }

    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        self.inner.element_at_point(point)
    }

    fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>) {
        self.inner.set_window_manager(window_manager);
    }

    fn shutdown(&self) {
        self.inner.shutdown();
    }
}
