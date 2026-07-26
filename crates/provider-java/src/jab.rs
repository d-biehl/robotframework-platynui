//! The Java Access Bridge backend (Swing/AWT on Windows).
//!
//! A thin adapter: `platynui-provider-java-jab` already speaks the per-window
//! surface, so this maps its enumeration pass onto [`JavaBackend`] and hands
//! its nodes through untouched — `@Technology = "JAB"`, patterns, node
//! validity and RuntimeIds stay exactly what that crate produces.

use crate::backend::{Enumeration, JavaBackend, UnservedJavaWindow};
use platynui_core::config::ConfigMap;
use platynui_core::platform::WindowManager;
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;
use platynui_provider_java_jab::JabProvider;
use std::sync::Arc;

/// Backend id and config sub-map: `providers.java.jab.*`.
pub(crate) const BACKEND_ID: &str = platynui_provider_java_jab::BACKEND_ID;

pub(crate) struct JabBackend {
    inner: JabProvider,
}

impl JabBackend {
    /// Build the backend from its sub-map of the Java provider's config.
    pub(crate) fn from_config(settings: Option<&ConfigMap>) -> Self {
        Self { inner: JabProvider::from_config(settings) }
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
