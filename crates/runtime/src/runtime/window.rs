use std::sync::Arc;
use std::time::Duration;

use platynui_core::platform::{HighlightRequest, PlatformError, Screenshot, ScreenshotRequest};
use platynui_core::ui::{
    FocusableAction, FocusablePattern, Namespace, UiNode, UiNodeExt, WindowSurfaceActions, WindowSurfacePattern,
};

use super::error::{BringToFrontError, FocusError};
use super::{Runtime, default_sleep};

impl Runtime {
    pub fn focus(&self, node: &Arc<dyn UiNode>) -> Result<(), FocusError> {
        let runtime_id = node.runtime_id().as_str().to_owned();
        let pattern = match node.pattern::<FocusableAction>() {
            Some(pattern) => pattern,
            None => return Err(FocusError::PatternMissing { runtime_id }),
        };

        if let Err(source) = pattern.focus() {
            return Err(FocusError::ActionFailed { runtime_id, source });
        }

        Ok(())
    }

    pub fn desktop_node(&self) -> Arc<dyn UiNode> {
        self.desktop.as_ui_node()
    }

    pub fn desktop_info(&self) -> &platynui_core::platform::DesktopInfo {
        self.desktop.info()
    }

    /// Returns the nearest ancestor (including `node` itself) that exposes the `WindowSurface`
    /// pattern. For `app:Application` nodes without a direct pattern, this method selects the
    /// first child that exposes a `WindowSurface`.
    pub fn top_level_window_for(&self, node: &Arc<dyn UiNode>) -> Option<Arc<dyn UiNode>> {
        for anc in node.ancestors_including_self() {
            if anc.pattern::<WindowSurfaceActions>().is_some() {
                return Some(anc);
            }
        }
        if node.namespace() == Namespace::App && node.role() == "Application" {
            for child in node.children() {
                if child.pattern::<WindowSurfaceActions>().is_some() {
                    return Some(child);
                }
            }
        }
        None
    }

    /// Bring the window associated with `node` to the foreground. If minimized, tries `restore()`
    /// first, then `activate()`.
    pub fn bring_to_front(&self, node: &Arc<dyn UiNode>) -> Result<(), BringToFrontError> {
        let window = match self.top_level_window_for(node) {
            Some(w) => w,
            None => {
                return Err(BringToFrontError::PatternMissing { runtime_id: node.runtime_id().as_str().to_owned() });
            }
        };
        let rid = window.runtime_id().as_str().to_owned();
        let pattern = window
            .pattern::<WindowSurfaceActions>()
            .ok_or_else(|| BringToFrontError::PatternMissing { runtime_id: rid.clone() })?;

        // Always try to restore to a normal state first. On many platforms this is a no-op when
        // the window is already visible, but required when minimized. Ignore errors here and rely
        // on the subsequent activate() to surface meaningful failures.
        let _ = pattern.restore();
        pattern.activate().map_err(|source| BringToFrontError::ActionFailed { runtime_id: rid, source })
    }

    /// Bring the window to the foreground and wait until it accepts user input, or until `timeout`.
    /// If the platform does not report input readiness (`accepts_user_input` returns `None`), this
    /// returns immediately after activating the window.
    pub fn bring_to_front_and_wait(&self, node: &Arc<dyn UiNode>, timeout: Duration) -> Result<(), BringToFrontError> {
        self.bring_to_front(node)?;

        let window = match self.top_level_window_for(node) {
            Some(w) => w,
            None => {
                return Err(BringToFrontError::PatternMissing { runtime_id: node.runtime_id().as_str().to_owned() });
            }
        };
        let rid = window.runtime_id().as_str().to_owned();
        let pattern = window
            .pattern::<WindowSurfaceActions>()
            .ok_or_else(|| BringToFrontError::PatternMissing { runtime_id: rid.clone() })?;

        let start = std::time::Instant::now();
        loop {
            match pattern.accepts_user_input() {
                Ok(Some(true)) => return Ok(()),
                Ok(Some(false)) => {
                    if start.elapsed() >= timeout {
                        return Err(BringToFrontError::Timeout { runtime_id: rid, waited: timeout });
                    }
                    default_sleep(Duration::from_millis(20));
                }
                Ok(None) => return Ok(()),
                Err(source) => return Err(BringToFrontError::ActionFailed { runtime_id: rid, source }),
            }
        }
    }

    /// Highlights the given regions using the registered highlight provider.
    pub fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        match self.highlight {
            Some(provider) => provider.highlight(request),
            None => Err(PlatformError::UnsupportedPlatform {
                platform: "highlight provider registry",
                details: Some("no HighlightProvider registered".into()),
            }),
        }
    }

    /// Clears an active highlight overlay if a provider is available.
    pub fn clear_highlight(&self) -> Result<(), PlatformError> {
        match self.highlight {
            Some(provider) => provider.clear(),
            None => Err(PlatformError::UnsupportedPlatform {
                platform: "highlight provider registry",
                details: Some("no HighlightProvider registered".into()),
            }),
        }
    }

    /// Captures a screenshot using the registered screenshot provider.
    pub fn screenshot(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        match self.screenshot {
            Some(provider) => provider.capture(request),
            None => Err(PlatformError::UnsupportedPlatform {
                platform: "screenshot provider registry",
                details: Some("no ScreenshotProvider registered".into()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::EvaluationItem;
    use crate::runtime::test_fixtures::*;
    use platynui_core::platform::{HighlightRequest, ScreenshotRequest};
    use platynui_core::provider::UiTreeProviderFactory;
    use platynui_core::types::Rect;
    use platynui_core::ui::attribute_names;
    use platynui_core::ui::{Namespace, UiNode, UiValue, WindowSurfaceActions, WindowSurfacePattern};
    use platynui_platform_mock::{
        reset_highlight_state, reset_screenshot_state, take_highlight_log, take_screenshot_log,
    };
    use rstest::rstest;

    use super::Runtime;

    #[rstest]
    fn runtime_focus_succeeds_on_focusable(rt_runtime_focus: Runtime) {
        let mut runtime = rt_runtime_focus;
        let desktop = runtime.desktop_node();
        let focus = FOCUS_FACTORY.create().expect("focus provider");
        let nodes = focus.get_nodes(desktop).expect("children");
        let mut button = None;
        for node in nodes {
            if node.role() == "Button" {
                button = Some(node);
            }
        }
        let button = button.expect("button node available");
        runtime.focus(&button).expect("focus succeeds");
        runtime.shutdown();
    }

    #[rstest]
    fn runtime_focus_requires_focusable_pattern(rt_runtime_focus: Runtime) {
        let mut runtime = rt_runtime_focus;
        let desktop = runtime.desktop_node();
        let focus = FOCUS_FACTORY.create().expect("focus provider");
        let nodes = focus.get_nodes(desktop).expect("children");
        let mut panel = None;
        for node in nodes {
            if node.role() == "Panel" {
                panel = Some(node);
            }
        }
        let panel = panel.expect("panel node available");
        let err = runtime.focus(&panel).expect_err("panel should not support focus");
        assert!(matches!(err, super::super::error::FocusError::PatternMissing { .. }));
        runtime.shutdown();
    }

    #[rstest]
    fn highlight_invokes_registered_provider(rt_runtime_platform: Runtime) {
        reset_highlight_state();
        let runtime = rt_runtime_platform;
        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 50.0, 25.0));
        runtime.highlight(&request).expect("highlight succeeds");

        let log = take_highlight_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], request);
    }

    #[rstest]
    fn screenshot_invokes_registered_provider(rt_runtime_platform: Runtime) {
        reset_screenshot_state();
        let runtime = rt_runtime_platform;
        let request = ScreenshotRequest::with_region(Rect::new(0.0, 0.0, 20.0, 10.0));
        let screenshot = runtime.screenshot(&request).expect("screenshot captures");

        assert_eq!(screenshot.width, 20);
        assert_eq!(screenshot.height, 10);
        assert_eq!(take_screenshot_log().len(), 1);
    }

    fn is_minimized_bool(node: &Arc<dyn UiNode>) -> Option<bool> {
        let attr = node.attribute(Namespace::Control, attribute_names::window_surface::IS_MINIMIZED)?;
        match attr.value() {
            UiValue::Bool(b) => Some(b),
            UiValue::Integer(i) => Some(i != 0),
            UiValue::Number(n) => Some(n != 0.0),
            _ => None,
        }
    }

    #[rstest]
    fn bring_to_front_restores_minimized_window() {
        let runtime = Runtime::new_with_factories(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY])
            .expect("runtime initializes with mock provider");

        let results = runtime.evaluate(None, "//control:Window[@Name='Settings']").expect("evaluate ok");
        let window = match results.into_iter().find_map(|it| match it {
            EvaluationItem::Node(n) => Some(n),
            _ => None,
        }) {
            Some(n) => n,
            None => panic!("window not found"),
        };

        let pattern = window.pattern::<WindowSurfaceActions>().expect("mock window exposes WindowSurface");
        pattern.minimize().expect("minimize succeeds");

        let is_min = is_minimized_bool(&window).unwrap_or(false);
        assert!(is_min, "window should be minimized before bring_to_front");

        runtime.bring_to_front(&window).expect("bring_to_front succeeds");

        let is_min = is_minimized_bool(&window).unwrap_or(true);
        assert!(!is_min, "window should be restored after bring_to_front");
    }

    #[rstest]
    fn bring_to_front_reports_missing_pattern(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let results = runtime.evaluate(None, "//control:Button").expect("eval ok");
        let panel = match results.into_iter().find_map(|it| match it {
            EvaluationItem::Node(n) => Some(n),
            _ => None,
        }) {
            Some(n) => n,
            None => panic!("node not found"),
        };
        let err = runtime.bring_to_front(&panel).expect_err("should fail: no WindowSurface ancestor");
        match err {
            super::super::error::BringToFrontError::PatternMissing { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
