use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use platynui_core::ui::UiNode;

use crate::{EvaluateError, EvaluateOptions, EvaluationItem, evaluate};

use super::Runtime;

impl Runtime {
    pub fn create_cache(&self) -> crate::xpath::XdmCache {
        crate::xpath::XdmCache::new()
    }

    pub fn evaluate_options(&self) -> EvaluateOptions {
        EvaluateOptions::new(self.desktop_node())
    }

    pub fn evaluate(&self, node: Option<Arc<dyn UiNode>>, xpath: &str) -> Result<Vec<EvaluationItem>, EvaluateError> {
        evaluate(node, xpath, self.evaluate_options())
    }

    pub fn evaluate_iter(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<impl Iterator<Item = Result<crate::xpath::EvaluationItem, EvaluateError>>, EvaluateError> {
        crate::xpath::evaluate_iter(node, xpath, self.evaluate_options())
    }

    pub fn evaluate_iter_owned(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<crate::xpath::EvaluationStream, EvaluateError> {
        crate::xpath::EvaluationStream::new(node, xpath.to_string(), self.evaluate_options())
    }

    /// Create a streaming (lazy) evaluator with a cancellation flag.
    ///
    /// The returned `EvaluationStream` is `!Send` — it must be iterated on the
    /// same thread that called this method. The `cancel_flag` is checked at each
    /// axis step; setting it to `true` causes the iterator to yield an error and stop.
    pub fn evaluate_iter_owned_cancellable(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<crate::xpath::EvaluationStream, EvaluateError> {
        crate::xpath::EvaluationStream::new(
            node,
            xpath.to_string(),
            self.evaluate_options().with_cancel_flag(cancel_flag),
        )
    }

    pub fn evaluate_single(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<Option<EvaluationItem>, EvaluateError> {
        let mut iter = self.evaluate_iter(node, xpath)?;
        match iter.next() {
            Some(Ok(item)) => Ok(Some(item)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    pub fn evaluate_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
        cache: &crate::xpath::XdmCache,
    ) -> Result<Vec<EvaluationItem>, EvaluateError> {
        evaluate(node, xpath, self.evaluate_options().with_cache(cache.clone()))
    }

    fn shared_xpath_cache(&self) -> crate::xpath::XdmCache {
        self.xpath_cache.lock().expect("xpath cache mutex poisoned").clone()
    }

    pub fn clear_cache(&self) {
        self.xpath_cache.lock().expect("xpath cache mutex poisoned").clear();
    }

    pub fn evaluate_runtime_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<Vec<EvaluationItem>, EvaluateError> {
        let cache = self.shared_xpath_cache();
        self.evaluate_cached(node, xpath, &cache)
    }

    pub fn evaluate_iter_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
        cache: &crate::xpath::XdmCache,
    ) -> Result<impl Iterator<Item = Result<crate::xpath::EvaluationItem, EvaluateError>>, EvaluateError> {
        crate::xpath::evaluate_iter(node, xpath, self.evaluate_options().with_cache(cache.clone()))
    }

    pub fn evaluate_iter_owned_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
        cache: &crate::xpath::XdmCache,
    ) -> Result<crate::xpath::EvaluationStream, EvaluateError> {
        crate::xpath::EvaluationStream::new(node, xpath.to_string(), self.evaluate_options().with_cache(cache.clone()))
    }

    pub fn evaluate_iter_owned_runtime_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<crate::xpath::EvaluationStream, EvaluateError> {
        let cache = self.shared_xpath_cache();
        self.evaluate_iter_owned_cached(node, xpath, &cache)
    }

    pub fn evaluate_single_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
        cache: &crate::xpath::XdmCache,
    ) -> Result<Option<EvaluationItem>, EvaluateError> {
        let mut iter = self.evaluate_iter_cached(node, xpath, cache)?;
        match iter.next() {
            Some(Ok(item)) => Ok(Some(item)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    pub fn evaluate_single_runtime_cached(
        &self,
        node: Option<Arc<dyn UiNode>>,
        xpath: &str,
    ) -> Result<Option<EvaluationItem>, EvaluateError> {
        let cache = self.shared_xpath_cache();
        self.evaluate_single_cached(node, xpath, &cache)
    }
}

#[cfg(test)]
mod tests {
    use crate::EvaluationItem;
    use crate::runtime::test_fixtures::*;
    use crate::test_support::rt_runtime_mock;
    use rstest::rstest;

    use super::Runtime;

    #[rstest]
    fn query_windows_streams_and_completes(rt_runtime_stub: Runtime) {
        // Broad window query should return promptly and complete without hanging
        let res = rt_runtime_stub.evaluate(None, "//control:Window").expect("evaluate windows");
        // Pull all items to ensure completion
        for _ in res {}
    }

    #[rstest]
    fn query_window_by_name_streams_and_completes(rt_runtime_stub: Runtime) {
        // Narrow query with attribute predicate must stream and complete
        let res = rt_runtime_stub
            .evaluate(None, "//control:Window[@Name='Operations Console']")
            .expect("evaluate windows by name");
        // It's fine if mock doesn't match; ensure evaluation completes
        for _ in res {}
    }

    #[rstest]
    fn union_windows_or_buttons_returns_both(rt_runtime_mock: Runtime) {
        // The mock tree contains 1 Window and 2 Buttons
        let res = rt_runtime_mock.evaluate(None, "//control:Window | //control:Button").expect("evaluate union");
        let mut count = 0usize;
        let mut names = Vec::new();
        for item in res {
            if let EvaluationItem::Node(node) = item {
                count += 1;
                names.push(node.name().to_string());
            }
        }
        assert!(count >= 3, "expected at least 3 nodes (1 window + 2 buttons), got {}", count);
        assert!(names.iter().any(|n| n == "Operations Console"));
        assert!(names.iter().any(|n| n == "OK"));
        assert!(names.iter().any(|n| n == "Cancel"));
    }

    #[rstest]
    fn intersect_windows_and_buttons_is_empty(rt_runtime_mock: Runtime) {
        let res =
            rt_runtime_mock.evaluate(None, "//control:Window intersect //control:Button").expect("evaluate intersect");
        let mut count = 0usize;
        for _ in res {
            count += 1;
        }
        assert_eq!(count, 0, "Windows and Buttons should be disjoint");
    }

    #[rstest]
    fn except_windows_minus_buttons_equals_windows(rt_runtime_mock: Runtime) {
        // Count all windows
        let windows = rt_runtime_mock.evaluate(None, "//control:Window").expect("evaluate windows");
        let mut windows_count = 0usize;
        for _ in windows {
            windows_count += 1;
        }

        // Subtract buttons from windows (disjoint sets) — result should equal windows
        let diff = rt_runtime_mock.evaluate(None, "//control:Window except //control:Button").expect("evaluate except");
        let mut diff_count = 0usize;
        for _ in diff {
            diff_count += 1;
        }

        assert_eq!(diff_count, windows_count);
    }

    #[rstest]
    fn intersect_buttons_with_self_is_identity(rt_runtime_mock: Runtime) {
        let buttons = rt_runtime_mock.evaluate(None, "//control:Button").expect("evaluate buttons");
        let mut buttons_count = 0usize;
        for _ in buttons {
            buttons_count += 1;
        }

        let inter = rt_runtime_mock
            .evaluate(None, "//control:Button intersect //control:Button")
            .expect("evaluate intersect self");
        let mut inter_count = 0usize;
        for _ in inter {
            inter_count += 1;
        }

        assert_eq!(inter_count, buttons_count);
    }

    #[rstest]
    fn runtime_evaluate_executes_xpath(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let results = runtime.evaluate(None, "//control:Button").expect("evaluation");
        assert!(!results.is_empty());
    }
}
