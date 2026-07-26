//! Provider-independent classification of JVM-backed top-level windows.
//!
//! Answers, for a native top-level window, the question that governs routing
//! and diagnosability of Java applications: *is this window backed by a JVM,
//! which UI toolkit renders it (Swing/AWT, SWT, JavaFX), and is it reachable
//! through the platform's native accessibility stack at all?*
//!
//! The [`JavaClassifier`] trait is a platform-bundle capability (like
//! [`WindowManager`](crate::platform::WindowManager)): the platform-specific
//! signal collection (module scans, window classes) lives in the platform
//! crates, while the pure signal→classification logic ([`classify_from_signals`])
//! and the shared "JVM window absent from native accessibility" diagnostic
//! live here so every provider reports the same facts the same way. The
//! cross-platform signal table is documented in `dev-docs/java-toolkits.md`.

use crate::platform::{PlatformError, WindowId};
use std::collections::HashSet;
use std::fmt;
use std::sync::{LazyLock, Mutex};

/// Name of the `native:` attribute carrying [`JavaClassification::is_jvm`].
pub const IS_JVM_ATTRIBUTE: &str = "IsJvm";
/// Name of the `native:` attribute carrying [`JavaClassification::toolkit`].
pub const JVM_TOOLKIT_ATTRIBUTE: &str = "JvmToolkit";
/// Name of the `native:` attribute carrying [`JavaClassification::native_a11y_visible`].
pub const JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE: &str = "JvmAccessibilityReachable";
/// Name of the `native:` attribute carrying [`JavaClassification::agent_present`].
pub const JVM_AGENT_PRESENT_ATTRIBUTE: &str = "JvmAgentPresent";

/// The Java UI toolkit hosting a top-level window.
///
/// This is the *best-effort host toolkit*: embedded content (`JFXPanel`,
/// `FXCanvas`, `SWT_AWT`) still reports the host window's toolkit. The
/// authoritative per-subtree answer requires an in-JVM agent (forward design).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum JavaToolkit {
    /// Swing or plain AWT (indistinguishable from outside the JVM).
    SwingAwt,
    /// Eclipse SWT.
    Swt,
    /// JavaFX (Glass windowing layer).
    JavaFx,
    /// Confirmed JVM window, but no toolkit discriminator matched.
    Unknown,
}

impl JavaToolkit {
    /// Toolkit from a Windows top-level window class name, per the prefix
    /// table in `dev-docs/java-toolkits.md` (`SunAwt*`, `SWT_Window*`,
    /// `Glass*`). `None` when the class matches no Java toolkit.
    pub fn from_window_class(class_name: &str) -> Option<Self> {
        if class_name.starts_with("SunAwt") {
            Some(Self::SwingAwt)
        } else if class_name.starts_with("SWT_Window") {
            Some(Self::Swt)
        } else if class_name.starts_with("Glass") {
            Some(Self::JavaFx)
        } else {
            None
        }
    }

    /// Stable display label, used verbatim as the `native:JvmToolkit`
    /// attribute value (Inspector display and XPath selectors).
    pub fn label(self) -> &'static str {
        match self {
            Self::SwingAwt => "Swing/AWT",
            Self::Swt => "SWT",
            Self::JavaFx => "JavaFX",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for JavaToolkit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The classification of one native top-level window.
///
/// The fields are deliberately independent signals — their reliability differs
/// per platform, and the actionable states are combinations ("JVM yes, toolkit
/// Swing, accessibility no, no agent"). A field that cannot be answered is
/// reported as unknown (`None`), never guessed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JavaClassification {
    /// The window's process runs a JVM (robust signal, independent of any
    /// accessibility bridge — e.g. `jvm.dll` loaded on Windows).
    pub is_jvm: bool,
    /// Best-effort host toolkit; `Some(JavaToolkit::Unknown)` for a confirmed
    /// JVM window without a toolkit discriminator, `None` for non-JVM windows.
    pub toolkit: Option<JavaToolkit>,
    /// Whether the window is reachable through the platform's native
    /// accessibility stack. Computed only where a free signal exists (see
    /// [`classify_from_signals`]); `None` where answering would require an
    /// eager probe.
    pub native_a11y_visible: Option<bool>,
    /// Whether a PlatynUI agent is present in the window's process, from the
    /// agent's per-user handshake file. `None` for non-JVM windows and where
    /// the probe was not possible.
    ///
    /// Reporting it is deliberately separate from *using* it: this is the
    /// breadcrumb that turns "this window is empty" into "this is a JVM, it has
    /// no agent, and here is how to get one". Reading the signal never triggers
    /// an attach.
    pub agent_present: Option<bool>,
}

/// Platform capability that classifies a native top-level window as
/// JVM-backed or not.
///
/// Implementations live in platform crates and are exposed through the
/// [`PlatformBundle`](crate::platform::PlatformBundle); a platform without a
/// backend leaves the bundle slot `None` and callers degrade to "unknown".
pub trait JavaClassifier: Send + Sync {
    /// Human-readable backend name for diagnostics.
    fn name(&self) -> &'static str;

    /// Classify the top-level window `window` owned by process `pid`.
    fn classify(&self, window: WindowId, pid: u32) -> Result<JavaClassification, PlatformError>;
}

/// Pure signal→classification logic, shared by the platform backends.
///
/// - `jvm_runtime_loaded`: the platform's process-level JVM signal (`jvm.dll`
///   / `libjvm.so` present); `None` when the scan was not possible (e.g.
///   access denied on an elevated process).
/// - `window_class`: the platform's toolkit discriminator (top-level window
///   class on Windows); `None` when unavailable.
/// - `claimed_by_provider`: whether a provider claims the window in
///   [`window_claims`](crate::platform::window_claims) — the materialized
///   "a native accessibility provider genuinely serves this window" fact.
/// - `agent_present`: whether a PlatynUI agent published a live handshake file
///   for the window's process; `None` when the probe was not possible.
///
/// Rules:
/// - is-JVM when the runtime is loaded **or** the window class names a Java
///   toolkit (the class keeps classification working where the module scan is
///   denied).
/// - The toolkit is never guessed: unmatched class on a JVM window ⇒
///   [`JavaToolkit::Unknown`]; non-JVM ⇒ `None`.
/// - `native_a11y_visible` is answered only where free: for Swing/AWT the
///   provider claim is the accessibility-bridge answer (on Windows, the JAB
///   provider claims exactly the windows `isJavaWindow` acknowledged). SWT and
///   JavaFX windows are served by the generic native provider, so answering
///   would need an eager probe — left `None` by design.
/// - `agent_present` is reported only for JVM windows: "no agent" about a
///   process that is not a JVM would be a fact about nothing.
pub fn classify_from_signals(
    jvm_runtime_loaded: Option<bool>,
    window_class: Option<&str>,
    claimed_by_provider: bool,
    agent_present: Option<bool>,
) -> JavaClassification {
    let class_toolkit = window_class.and_then(JavaToolkit::from_window_class);
    let is_jvm = jvm_runtime_loaded == Some(true) || class_toolkit.is_some();
    let toolkit = is_jvm.then(|| class_toolkit.unwrap_or(JavaToolkit::Unknown));
    let native_a11y_visible = match toolkit {
        Some(JavaToolkit::SwingAwt) => Some(claimed_by_provider),
        _ => None,
    };
    JavaClassification {
        is_jvm,
        toolkit,
        native_a11y_visible,
        agent_present: is_jvm.then_some(agent_present).flatten(),
    }
}

/// Windows already warned about, process-wide (all runtimes see the same
/// desktop reality, mirroring `window_claims`).
static UNREACHABLE_WARNED: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// The shared "JVM window absent from native accessibility" diagnostic,
/// de-duplicated process-wide.
///
/// Returns the actionable enablement message for `toolkit` the first time it
/// is called for `window` (the raw native handle, `WindowId::raw()` /
/// `window_claims` semantics) and `None` on every later call, so the caller
/// logs it at most once per window. Core deliberately does not log itself —
/// the calling provider owns the log line and its structured fields.
pub fn jvm_unreachable_diagnostic_once(window: u64, toolkit: JavaToolkit) -> Option<&'static str> {
    let mut warned = UNREACHABLE_WARNED.lock().expect("jvm diagnostic registry mutex poisoned");
    if !warned.insert(window) {
        return None;
    }
    Some(enablement_hint(toolkit))
}

/// Whether the diagnostic has already been emitted for `window`. Intended for
/// tests and diagnostics tooling.
pub fn jvm_unreachable_diagnostic_emitted(window: u64) -> bool {
    UNREACHABLE_WARNED.lock().expect("jvm diagnostic registry mutex poisoned").contains(&window)
}

/// The actionable enablement path for `toolkit` on the current platform
/// (`dev-docs/java-toolkits.md` records the per-platform facts). The message
/// never mutates anything — it only tells the user how to.
fn enablement_hint(toolkit: JavaToolkit) -> &'static str {
    if cfg!(windows) {
        match toolkit {
            JavaToolkit::SwingAwt => {
                "Enable the Java Access Bridge in the target JVM: launch it with \
                 -Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge \
                 (per-process), or run 'jabswitch -enable' once for the user (persistent)."
            }
            JavaToolkit::Swt => {
                "SWT windows are served through native Win32/UIA; if this window stays empty, check the \
                 application's accessibility configuration."
            }
            JavaToolkit::JavaFx => {
                "JavaFX serves UIA natively on Windows; ensure the application does not disable \
                 accessibility (e.g. -Dglass.accessible.force=false)."
            }
            JavaToolkit::Unknown => {
                "The Java UI toolkit could not be determined; enable the toolkit's platform \
                 accessibility bridge in the target JVM."
            }
        }
    } else if cfg!(target_os = "linux") {
        match toolkit {
            JavaToolkit::SwingAwt => {
                "Swing/AWT is reachable via AT-SPI only with the java-atk-wrapper enabled in the target \
                 JVM (-Djavax.accessibility.assistive_technologies=org.GNOME.Accessibility.AtkWrapper); \
                 PlatynUI does not rely on or enable it (see dev-docs/java-toolkits.md)."
            }
            JavaToolkit::JavaFx => {
                "JavaFX has no native accessibility on Linux at all; only an in-JVM agent can reach it."
            }
            _ => "Enable the toolkit's platform accessibility bridge in the target JVM.",
        }
    } else {
        "Enable the toolkit's platform accessibility bridge in the target JVM."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock-lane coverage of the pure classification logic: given signal
    // inputs → expected fields, per the java-app-classification spec.

    #[test]
    fn swing_window_class_and_jvm_module_classify_as_swing() {
        let c = classify_from_signals(Some(true), Some("SunAwtFrame"), true, None);
        assert!(c.is_jvm);
        assert_eq!(c.toolkit, Some(JavaToolkit::SwingAwt));
        assert_eq!(c.native_a11y_visible, Some(true));
    }

    #[test]
    fn unclaimed_swing_window_reports_native_a11y_not_visible() {
        let c = classify_from_signals(Some(true), Some("SunAwtDialog"), false, None);
        assert!(c.is_jvm);
        assert_eq!(c.toolkit, Some(JavaToolkit::SwingAwt));
        assert_eq!(c.native_a11y_visible, Some(false));
    }

    #[test]
    fn swt_and_javafx_leave_native_a11y_unknown() {
        for (class, toolkit) in [("SWT_Window0", JavaToolkit::Swt), ("GlassWndClass", JavaToolkit::JavaFx)] {
            let c = classify_from_signals(Some(true), Some(class), false, None);
            assert!(c.is_jvm);
            assert_eq!(c.toolkit, Some(toolkit), "class {class}");
            assert_eq!(c.native_a11y_visible, None, "no eager probe for {class}");
        }
    }

    // The agent-presence signal: reported for JVM windows, and never invented
    // for a window that is not one.
    #[test]
    fn agent_presence_is_reported_only_for_jvm_windows() {
        let with_agent = classify_from_signals(Some(true), Some("SunAwtFrame"), false, Some(true));
        assert_eq!(with_agent.agent_present, Some(true));

        let without_agent = classify_from_signals(Some(true), Some("SunAwtFrame"), false, Some(false));
        assert_eq!(without_agent.agent_present, Some(false), "\"no agent here\" is a fact worth reporting");

        let not_probed = classify_from_signals(Some(true), Some("SunAwtFrame"), false, None);
        assert_eq!(not_probed.agent_present, None, "an unavailable probe must not read as \"no agent\"");

        let not_a_jvm = classify_from_signals(Some(false), Some("Notepad"), false, Some(false));
        assert_eq!(not_a_jvm.agent_present, None, "a fact about nothing is not a fact");
    }

    #[test]
    fn no_toolkit_discriminator_yields_unknown_with_jvm_still_set() {
        let c = classify_from_signals(Some(true), Some("Chrome_WidgetWin_1"), false, None);
        assert!(c.is_jvm, "the JVM signal must survive an unmatched window class");
        assert_eq!(c.toolkit, Some(JavaToolkit::Unknown), "the toolkit must not be guessed");
        assert_eq!(c.native_a11y_visible, None);

        let c = classify_from_signals(Some(true), None, false, None);
        assert!(c.is_jvm);
        assert_eq!(c.toolkit, Some(JavaToolkit::Unknown));
    }

    #[test]
    fn non_jvm_window_is_not_misclassified() {
        let c = classify_from_signals(Some(false), Some("Notepad"), false, None);
        assert!(!c.is_jvm);
        assert_eq!(c.toolkit, None);
        assert_eq!(c.native_a11y_visible, None);
    }

    #[test]
    fn java_window_class_implies_jvm_when_module_scan_is_unavailable() {
        // Elevated targets deny the module scan; the class keeps working.
        let c = classify_from_signals(None, Some("SunAwtFrame"), false, None);
        assert!(c.is_jvm);
        assert_eq!(c.toolkit, Some(JavaToolkit::SwingAwt));
    }

    #[test]
    fn unknown_signals_classify_as_not_jvm() {
        let c = classify_from_signals(None, None, false, None);
        assert!(!c.is_jvm);
        assert_eq!(c.toolkit, None);
        assert_eq!(c.native_a11y_visible, None);
    }

    #[test]
    fn window_class_prefixes_match_per_toolkit_table() {
        assert_eq!(JavaToolkit::from_window_class("SunAwtFrame"), Some(JavaToolkit::SwingAwt));
        assert_eq!(JavaToolkit::from_window_class("SunAwtWindow"), Some(JavaToolkit::SwingAwt));
        assert_eq!(JavaToolkit::from_window_class("SWT_Window0"), Some(JavaToolkit::Swt));
        assert_eq!(JavaToolkit::from_window_class("GlassWindowClass"), Some(JavaToolkit::JavaFx));
        assert_eq!(JavaToolkit::from_window_class("Chrome_WidgetWin_1"), None);
        assert_eq!(JavaToolkit::from_window_class(""), None);
    }

    // The registry is process-global, so tests use distinct window values to
    // stay independent of each other and of test-ordering.

    #[test]
    fn diagnostic_fires_once_per_window() {
        assert!(!jvm_unreachable_diagnostic_emitted(0xC100));
        let first = jvm_unreachable_diagnostic_once(0xC100, JavaToolkit::SwingAwt);
        assert!(first.is_some_and(|hint| !hint.is_empty()));
        assert!(jvm_unreachable_diagnostic_once(0xC100, JavaToolkit::SwingAwt).is_none(), "must not repeat");
        assert!(jvm_unreachable_diagnostic_emitted(0xC100));
    }

    #[test]
    fn diagnostic_is_deduplicated_per_window_not_globally() {
        assert!(jvm_unreachable_diagnostic_once(0xC200, JavaToolkit::JavaFx).is_some());
        assert!(jvm_unreachable_diagnostic_once(0xC201, JavaToolkit::Swt).is_some());
    }
}
