## 1. Core API

- [x] 1.1 New module `platynui_core::platform::java`: a `JavaClassifier` trait (or an addition to the platform bundle) returning `JavaClassification { is_jvm: bool, toolkit: Option<JavaToolkit>, native_a11y_visible: Option<bool> }`, plus the `JavaToolkit` enum (`SwingAwt`, `Swt`, `JavaFx`, `Unknown`); keyed on a top-level window handle + PID
- [x] 1.2 Expose it via the platform bundle (like `WindowManager`), `Option`-typed so a platform without a backend yields `None` and callers degrade to "unknown"
- [x] 1.3 Mock-lane unit test: the pure classification logic (given signal inputs → expected fields), including "no toolkit discriminator ⇒ Unknown, is-JVM still set"

## 2. Windows backend

- [x] 2.1 `platform-windows`: top-level window class via `GetClassNameW` → toolkit (`SunAwt*`=Swing/AWT, `SWT_Window*`=SWT, `Glass*`=JavaFX; prefix matches per `java-toolkits.md`)
- [x] 2.2 `platform-windows`: `jvm.dll` module presence in the owning PID via `CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid)` + `Module32FirstW/NextW` → is-JVM (robust against renamed/jpackage launchers)
- [x] 2.3 `native_a11y_visible`: computed only where free (Swing via the JAB `isJavaWindow` result, materialized as the process-wide window claim); left `None` otherwise (design decision — no eager UIA probe)

## 3. Observability + diagnostic

- [x] 3.1 Surface the classification as `native:*` attributes on the owning app/window node (names pinned against the Inspector display: `native:IsJvm`, `native:JvmToolkit`, `native:JvmAccessibilityReachable` on top-level window nodes; UIA via the injected classifier, JAB via its own facts)
- [x] 3.2 Shared "JVM window absent from native accessibility" warn-once diagnostic naming the enablement path per toolkit/platform; `provider-jab` emits it in place of its own `SunAwtSuspect` message (JAB enumeration otherwise untouched)

## 4. Verification

- [x] 4.1 Windows real-provider scenario: the Swing fixture (bridge on) classifies as JVM+Swing+reachable; a bridge-less Swing launch classifies as JVM+Swing+not-reachable and fires the diagnostic once (`live_jvm_classification_facts_and_diagnostic` in `provider-jab/tests/live_fixture.rs`)
- [x] 4.2 Windows real-provider scenario: an SWT and/or JavaFX window classifies by window class as JVM+SWT / JVM+JavaFX (toolkit correct even though served by UIA) — via real native windows carrying the `SWT_Window0`/`GlassWindowClass` classes (`platform-windows/src/java.rs` tests; no SWT/JavaFX fixture app exists in the repo)
- [x] 4.3 A native (non-Java) window classifies as not-JVM with no diagnostic (`platform-windows/src/java.rs` tests)
- [x] 4.4 `dev-docs/java-toolkits.md` gains a pointer to the classifier API; then `just check`, `just test`, `just build-native`, and the Windows acceptance run green (68/2/5 — the 2 failures are the pre-existing egui/Qt open-menu failures already present on unmodified main)
