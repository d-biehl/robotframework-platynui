use std::sync::Arc;
use std::time::Duration;

use platynui_core::platform::{HighlightRequest, PlatformError, Screenshot, ScreenshotRequest};
use platynui_core::ui::{
    ActivatableAction, ActivatablePattern, FocusableAction, FocusablePattern, Namespace, ResponsiveAction,
    ResponsivePattern, UiNode, UiNodeExt,
};

use super::error::{BringToFrontError, FocusError};
use super::{Runtime, default_sleep};

impl Runtime {
    /// Moves keyboard focus to `node` through its `Focusable` pattern.
    ///
    /// # Errors
    ///
    /// Returns [`FocusError::PatternMissing`] if `node` does not expose the `Focusable` pattern and
    /// [`FocusError::ActionFailed`] if the focus action fails.
    pub fn focus(&self, node: &Arc<dyn UiNode>) -> Result<(), FocusError> {
        let runtime_id = node.runtime_id().as_str().to_owned();
        let Some(pattern) = node.pattern::<FocusableAction>() else {
            return Err(FocusError::PatternMissing { runtime_id });
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

    /// Returns the nearest ancestor (including `node` itself) that exposes the `Activatable`
    /// pattern (i.e. is a top-level window). For `app:Application` nodes without a direct
    /// pattern, this method selects the first child that exposes `Activatable`.
    pub fn top_level_window_for(&self, node: &Arc<dyn UiNode>) -> Option<Arc<dyn UiNode>> {
        for anc in node.ancestors_including_self() {
            if anc.pattern::<ActivatableAction>().is_some() {
                return Some(anc);
            }
        }
        if node.namespace() == Namespace::App && node.role() == "Application" {
            for child in node.children() {
                if child.pattern::<ActivatableAction>().is_some() {
                    return Some(child);
                }
            }
        }
        None
    }

    /// Bring the window associated with `node` to the foreground by activating it.
    ///
    /// Activation brings a minimized window back in the state it was minimized from and never
    /// un-maximizes a window, so bringing an element to the front does not move or resize its
    /// window. Returning a window to its normal state is the `Restorable` pattern's job.
    ///
    /// # Errors
    ///
    /// Returns [`BringToFrontError::PatternMissing`] if no window with the `Activatable` pattern is
    /// found for `node` and [`BringToFrontError::ActionFailed`] if the activation fails.
    pub fn bring_to_front(&self, node: &Arc<dyn UiNode>) -> Result<(), BringToFrontError> {
        let Some(window) = self.top_level_window_for(node) else {
            return Err(BringToFrontError::PatternMissing { runtime_id: node.runtime_id().as_str().to_owned() });
        };
        let rid = window.runtime_id().as_str().to_owned();
        let activatable = window
            .pattern::<ActivatableAction>()
            .ok_or_else(|| BringToFrontError::PatternMissing { runtime_id: rid.clone() })?;
        activatable.activate().map_err(|source| BringToFrontError::ActionFailed { runtime_id: rid, source })
    }

    /// Bring the window to the foreground and wait until it accepts user input, or until `timeout`.
    /// If the platform does not report input readiness (`accepts_user_input` returns `None`), this
    /// returns immediately after activating the window.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Runtime::bring_to_front`], [`BringToFrontError::ActionFailed`] if
    /// querying input readiness fails, and [`BringToFrontError::Timeout`] if the window does not
    /// accept input within `timeout`.
    pub fn bring_to_front_and_wait(&self, node: &Arc<dyn UiNode>, timeout: Duration) -> Result<(), BringToFrontError> {
        self.bring_to_front(node)?;

        let Some(window) = self.top_level_window_for(node) else {
            return Err(BringToFrontError::PatternMissing { runtime_id: node.runtime_id().as_str().to_owned() });
        };
        let rid = window.runtime_id().as_str().to_owned();
        let Some(responsive) = window.pattern::<ResponsiveAction>() else {
            // No Responsive pattern \u2014 nothing to wait on; treat as immediately ready.
            return Ok(());
        };

        let start = std::time::Instant::now();
        loop {
            match responsive.accepts_user_input() {
                // Ready, or the platform does not report input readiness.
                Ok(Some(true) | None) => return Ok(()),
                Ok(Some(false)) => {
                    if start.elapsed() >= timeout {
                        return Err(BringToFrontError::Timeout { runtime_id: rid, waited: timeout });
                    }
                    default_sleep(Duration::from_millis(20));
                }
                Err(source) => return Err(BringToFrontError::ActionFailed { runtime_id: rid, source }),
            }
        }
    }

    /// Highlights the given regions using this runtime's platform highlight device.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::UnsupportedPlatform`] if the runtime has no platform backend, and
    /// otherwise the error of the highlight device.
    pub fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        match self.platform.as_ref() {
            Some(bundle) => bundle.highlight.highlight(request),
            None => Err(super::no_platform_backend()),
        }
    }

    /// Clears an active highlight overlay if a platform is available.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::UnsupportedPlatform`] if the runtime has no platform backend, and
    /// otherwise the error of the highlight device.
    pub fn clear_highlight(&self) -> Result<(), PlatformError> {
        match self.platform.as_ref() {
            Some(bundle) => bundle.highlight.clear(),
            None => Err(super::no_platform_backend()),
        }
    }

    /// Captures a screenshot using this runtime's platform screenshot device.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::UnsupportedPlatform`] if the runtime has no platform backend, and
    /// otherwise the error of the screenshot device.
    pub fn screenshot(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        match self.platform.as_ref() {
            Some(bundle) => bundle.screenshot.capture(request),
            None => Err(super::no_platform_backend()),
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
    use platynui_core::ui::{
        MaximizableAction, MaximizablePattern, MinimizableAction, MinimizablePattern, Namespace, UiNode, UiValue,
    };
    use platynui_platform_mock::{
        reset_highlight_state, reset_screenshot_state, take_highlight_log, take_screenshot_log,
    };
    use rstest::rstest;
    use serial_test::serial;

    use super::Runtime;

    #[rstest]
    fn runtime_focus_succeeds_on_focusable(rt_runtime_focus: Runtime) {
        let mut runtime = rt_runtime_focus;
        let desktop = runtime.desktop_node();
        let focus = FOCUS_FACTORY.create(&platynui_core::config::RuntimeConfig::default()).expect("focus provider");
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
        let focus = FOCUS_FACTORY.create(&platynui_core::config::RuntimeConfig::default()).expect("focus provider");
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

    fn window_flag(node: &Arc<dyn UiNode>, name: &str) -> Option<bool> {
        let attr = node.attribute(Namespace::Control, name)?;
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
        let Some(window) = results.into_iter().find_map(|it| match it {
            EvaluationItem::Node(n) => Some(n),
            _ => None,
        }) else {
            panic!("window not found");
        };

        let pattern = window.pattern::<MinimizableAction>().expect("mock window exposes Minimizable");
        pattern.minimize().expect("minimize succeeds");

        let is_min = window_flag(&window, attribute_names::minimizable::IS_MINIMIZED).unwrap_or(false);
        assert!(is_min, "window should be minimized before bring_to_front");

        runtime.bring_to_front(&window).expect("bring_to_front succeeds");

        let is_min = window_flag(&window, attribute_names::minimizable::IS_MINIMIZED).unwrap_or(true);
        assert!(!is_min, "window should be restored after bring_to_front");
    }

    fn first_node(runtime: &Runtime, xpath: &str) -> Arc<dyn UiNode> {
        runtime
            .evaluate(None, xpath)
            .expect("evaluate ok")
            .into_iter()
            .find_map(|it| match it {
                EvaluationItem::Node(n) => Some(n),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no node for {xpath}"))
    }

    #[rstest]
    fn bring_to_front_keeps_maximized_window_maximized() {
        let runtime = Runtime::new_with_factories(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY])
            .expect("runtime initializes with mock provider");
        let window = first_node(&runtime, "//control:Window[@Name='Operations Console']");

        window.pattern::<MaximizableAction>().expect("mock window exposes Maximizable").maximize().expect("maximize");
        runtime.bring_to_front(&window).expect("bring_to_front succeeds");

        assert_eq!(window_flag(&window, attribute_names::maximizable::IS_MAXIMIZED), Some(true));
        assert_eq!(window_flag(&window, attribute_names::window_state::IS_ACTIVE), Some(true));
    }

    #[rstest]
    fn bring_to_front_brings_back_window_minimized_from_maximized_as_maximized() {
        let runtime = Runtime::new_with_factories(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY])
            .expect("runtime initializes with mock provider");
        let window = first_node(&runtime, "//control:Window[@Name='Detail View']");

        window.pattern::<MaximizableAction>().expect("mock window exposes Maximizable").maximize().expect("maximize");
        window.pattern::<MinimizableAction>().expect("mock window exposes Minimizable").minimize().expect("minimize");
        runtime.bring_to_front(&window).expect("bring_to_front succeeds");

        assert_eq!(window_flag(&window, attribute_names::minimizable::IS_MINIMIZED), Some(false));
        assert_eq!(window_flag(&window, attribute_names::maximizable::IS_MAXIMIZED), Some(true));
    }

    #[rstest]
    fn bring_to_front_reports_failed_activation(rt_runtime_rejecting_window: Runtime) {
        let runtime = rt_runtime_rejecting_window;
        let window = first_node(&runtime, "//control:Window");
        let err = runtime.bring_to_front(&window).expect_err("activation is rejected");
        match err {
            super::super::error::BringToFrontError::ActionFailed { runtime_id, .. } => {
                assert_eq!(runtime_id, "rejecting-window");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[rstest]
    fn bring_to_front_reports_missing_pattern(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let results = runtime.evaluate(None, "//control:Button").expect("eval ok");
        let Some(panel) = results.into_iter().find_map(|it| match it {
            EvaluationItem::Node(n) => Some(n),
            _ => None,
        }) else {
            panic!("node not found");
        };
        let err = runtime.bring_to_front(&panel).expect_err("should fail: no Activatable ancestor");
        match err {
            super::super::error::BringToFrontError::PatternMissing { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn single_node(item: Option<EvaluationItem>) -> Arc<dyn UiNode> {
        match item {
            Some(EvaluationItem::Node(node)) => node,
            other => panic!("expected a node, got {other:?}"),
        }
    }

    // The next three tests guard `top_level_window_for` for nodes whose
    // snapshot is gone, with a provider that follows the rule of
    // `UiNode::parent`; `bring_to_front_needs_the_provider_to_keep_the_ancestors`
    // shows that the provider rule is what they rely on.

    /// `platynui-cli pointer`'s path: the button comes from an evaluation
    /// without a retained snapshot, which is gone once it returns.
    #[rstest]
    #[serial(lazy_tree)]
    fn bring_to_front_activates_the_window_of_a_result_without_a_snapshot(rt_runtime_lazy_tree: Runtime) {
        let runtime = rt_runtime_lazy_tree;
        let button = single_node(runtime.evaluate_single(None, "//control:Button").expect("evaluate"));

        runtime.bring_to_front(&button).expect("the button's window is activated");

        assert_eq!(LAZY_TREE_LOG.activations(), 1);
    }

    /// `BareMetal`'s path: the snapshot is discarded before the action.
    #[rstest]
    #[serial(lazy_tree)]
    fn bring_to_front_activates_the_window_after_the_snapshot_was_discarded(rt_runtime_lazy_tree: Runtime) {
        let runtime = rt_runtime_lazy_tree;
        let button = single_node(runtime.evaluate_single_runtime_cached(None, "//control:Button").expect("evaluate"));
        runtime.clear_cache();

        runtime.bring_to_front(&button).expect("the button's window is activated");

        assert_eq!(LAZY_TREE_LOG.activations(), 1);
    }

    /// A root inside a window: the button is found below a pane, whose
    /// snapshot replaced the desktop's in the shared slot.
    #[rstest]
    #[serial(lazy_tree)]
    fn bring_to_front_activates_the_window_of_a_node_found_below_a_pane(rt_runtime_lazy_tree: Runtime) {
        let runtime = rt_runtime_lazy_tree;
        let pane = single_node(runtime.evaluate_single_runtime_cached(None, "//control:Pane").expect("evaluate"));
        let button =
            single_node(runtime.evaluate_single_runtime_cached(Some(pane), ".//control:Button").expect("evaluate"));
        runtime.clear_cache();

        runtime.bring_to_front(&button).expect("the button's window is activated");

        assert_eq!(LAZY_TREE_LOG.activations(), 1);
    }

    /// The runtime itself keeps no ancestors once a snapshot is gone: the tests
    /// above pass because the provider keeps them. Without the provider rule,
    /// the button's window is gone with its snapshot.
    #[rstest]
    #[serial(lazy_tree)]
    fn bring_to_front_needs_the_provider_to_keep_the_ancestors(rt_runtime_lazy_tree: Runtime) {
        let runtime = rt_runtime_lazy_tree;
        LAZY_TREE_LOG.set_keep_parents(false);
        let button = single_node(runtime.evaluate_single_runtime_cached(None, "//control:Button").expect("evaluate"));
        runtime.clear_cache();

        let err = runtime.bring_to_front(&button).expect_err("nothing keeps the button's window");

        assert!(matches!(err, super::super::error::BringToFrontError::PatternMissing { .. }), "{err:?}");
        assert_eq!(LAZY_TREE_LOG.activations(), 0);
    }

    #[rstest]
    #[serial(lazy_tree)]
    fn shutdown_releases_the_snapshot_before_the_providers(rt_runtime_lazy_tree: Runtime) {
        let mut runtime = rt_runtime_lazy_tree;
        drop(runtime.evaluate_runtime_cached(None, "//control:Button").expect("evaluate"));
        assert!(LAZY_TREE_LOG.live() > 0, "the retained snapshot holds its nodes");

        runtime.shutdown();

        assert_eq!(LAZY_TREE_LOG.live(), 0, "shutdown releases the snapshot");
        assert_eq!(
            LAZY_TREE_LOG.events().last(),
            Some(&"shutdown"),
            "nodes are released before the provider shuts down"
        );
    }
}
