# Java UI toolkits: detection and accessibility coverage

How PlatynUI recognizes JVM-backed GUI applications and which accessibility path
reaches each Java UI toolkit, per platform. This is the cross-platform map; the
Windows Java provider and its JAB backend live in [`platform-windows.md`](platform-windows.md)
§2a and the AT-SPI provider in [`platform-linux.md`](platform-linux.md).

Scope: facts as of 2026-07. This document records what is true today and the
settled decisions that constrain the forward design; the in-JVM *agent
provider* that would fill the gaps below is tracked in OpenSpec, not here.

The detection half of this map is implemented as the **Java-app classifier**:
`platynui_core::platform::java` defines the `JavaClassifier` platform-bundle
capability (is-JVM / toolkit / native-accessibility reachability per top-level
window), the pure signal→classification logic, and the shared "JVM window
absent from native accessibility" diagnostic. The Windows backend lives in
`platform-windows` (`src/java.rs`: window class + Toolhelp `jvm.dll` scan);
providers surface the result as `native:IsJvm`, `native:JvmToolkit`, and
`native:JvmAccessibilityReachable` attributes on their top-level window nodes.
Linux/macOS backends are follow-ups — callers degrade to "unknown".

## "Is this a JVM?" — the portable primitive

The most robust, target-cooperation-free signal is that the process has the JVM
runtime loaded. It survives renamed/bundled launchers (jpackage apps embed a JRE
and a custom `.exe`/binary, so the executable name is unreliable):

| Platform | Signal | How |
|---|---|---|
| Windows | `jvm.dll` loaded in the process | `CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid)` + `Module32FirstW/NextW` (HotSpot **and** OpenJ9 ship `jvm.dll`) |
| Linux | `libjvm.so` in `/proc/<pid>/maps` | read the maps file (same user, or `CAP_SYS_PTRACE`); display-server-independent (X11 **and** Wayland) |
| macOS | attach artifacts | `hsperfdata_<user>/<pid>` under `$TMPDIR` and the attach socket `/tmp/.java_pid<pid>` — entitlement-free (inspecting another process's loaded dylibs needs `task_for_pid`, which is entitlement/root-gated, so avoid it) |

On all three, the JVM's attach discovery (`VirtualMachine.list()` / `hsperfdata`
/ `jattach`) also enumerates same-user JVMs with their main class — usable as a
secondary signal and the entry point for the agent path.

**Window → process:**

- Windows: `GetWindowThreadProcessId(hwnd, &pid)`.
- Linux/X11: prefer the **X-Resource extension v1.2** (`XResQueryClientIds` with
  the `LocalClientPID` mask) — the X server derives the client's PID from the
  connection's socket credentials, so it is authoritative and works even for apps
  that set no property. Fall back to the EWMH `_NET_WM_PID` property on the
  top-level window, but only trust it when `WM_CLIENT_MACHINE` matches the local
  host (the property is client-claimed and meaningless for remote X clients). For
  a nested XID, climb via `XQueryTree` to the top-level (the `WM_STATE` window)
  first. Under Wayland there is no cross-client window enumeration; it is
  compositor-mediated (see [`platform-linux-wayland.md`](platform-linux-wayland.md)
  and `apps/wayland-compositor`).
- macOS: `CGWindowListCopyWindowInfo` → `kCGWindowOwnerPID`, or
  `NSWorkspace.runningApplications`.

## Toolkit discrimination

Precision degrades from Windows to macOS. Where it is fuzzy, an in-JVM agent (if
present) is the authoritative classifier — from inside the JVM the toolkit is
unambiguous (a `javax.swing` window hierarchy vs. a JavaFX `Stage` vs. an SWT
`Display`).

- **Windows — precise, via the top-level window class** (`GetClassNameW`):

  | Toolkit | Window class | Match |
  |---|---|---|
  | Swing/AWT | `SunAwtFrame`, `SunAwtDialog`, `SunAwtWindow`, … | prefix `SunAwt` |
  | SWT | `SWT_Window0`, `SWT_Window1`, … | prefix `SWT_Window` |
  | JavaFX (Glass) | `GlassWindowClass` (exact literal has drifted across versions) | prefix `Glass` |

  The class is also visible through UIA (`UIA_ClassNamePropertyId`), so the UIA
  provider can recognize a JavaFX/SWT window without any Java-specific code.

- **Linux — fuzzy.** X11 `WM_CLASS` is not a reliable discriminator (AWT derives
  it from `awt.appClassName`/the main class; there is no `SunAwt` equivalent).
  AT-SPI exposes a per-application *toolkit name*, but the exact strings vary by
  wrapper/version and SWT-on-GTK is indistinguishable from a native GTK app —
  verify empirically with Accerciser rather than hard-coding a literal.

- **macOS — least distinguishable.** Swing and JavaFX both bridge to
  NSAccessibility; SWT uses native Cocoa controls. From the AX side all three
  look like Cocoa apps; `bundleIdentifier`/executable path are weak heuristics.

## Native accessibility coverage (toolkit × platform)

Which toolkit is reachable through the platform's *native* accessibility stack
(and thus PlatynUI's existing provider), with no in-JVM agent:

| Toolkit | Windows | Linux | macOS |
|---|---|---|---|
| Swing/AWT | JAB backend of `provider-java` (`provider-java-jab`) — zero-config, limited fidelity | `java-atk-wrapper`/AT-SPI **only if enabled** — see decision below | NSAccessibility (`provider-macos-ax`) |
| SWT | native Win32 → UIA (`provider-windows-uia`) | GTK-native → AT-SPI (`provider-atspi`) | native Cocoa → NSAccessibility |
| JavaFX | UIA (`provider-windows-uia`) | **none — not accessible at all** | NSAccessibility |

The one hard gap: **JavaFX has no accessibility on Linux.** Oracle stated in 2015
"We have no plan to make FX accessible on Linux"; the accessibility
implementation was built for Windows and Mac only and it remains unimplemented on
Linux (still open on `openjfx-dev` as of 2025). So a JavaFX app on Linux appears
in *no* native accessibility tree — an in-JVM agent reading JavaFX's own
in-process model (`javafx.scene.Node.queryAccessibleAttribute` / the scene graph)
is the only way to reach it. Sources:
[orca-list 2015](https://mail.gnome.org/archives/orca-list/2015-July/msg00009.html),
[OpenJFX Accessibility Exploration](https://wiki.openjdk.org/display/OpenJFX/Accessibility+Exploration).

## Decision: do not rely on `java-atk-wrapper` on Linux

`java-atk-wrapper` (the Swing/AWT→ATK bridge, distro package
`libatk-wrapper-java`) is **not** part of PlatynUI's Linux strategy:

- it is fragile and can destabilize the target Swing app;
- enabling it requires modifying the target launch
  (`-Djavax.accessibility.assistive_technologies=org.GNOME.Accessibility.AtkWrapper`)
  plus a distro package on the target host — the same launch-modification problem
  we avoid for e.g. Java Web Start apps.

Consequence with the wrapper off: on Linux **only SWT** is covered by the native
provider (`provider-atspi`, GTK). **Swing/AWT and JavaFX are both agent-only**
there (JavaFX necessarily so — it has no native path at all). This mirrors the
shared "JVM window absent from native accessibility" diagnostic
(`platynui_core::platform::java`, emitted on Windows for bridge-less `SunAwt*`
windows): a JVM process with windows that is *absent from the AT-SPI tree* is
the agent target; process-level detection finds it, the agent classifies the
toolkit from inside.

## Agent injection paths and JEP 451 (facts)

The agent exists ([`java/agent`](../java/agent), transport in
[`crates/java-agent`](../crates/java-agent)). Two ways get it into the target
JVM; they load the *same* agent and differ only in how:

- **Attach + `loadAgent()`** — **the primary path**. No launch change, and it
  works on an application that is already running, which is the only way the
  Inspector can look into one. Java applications are started by scripts,
  installers or Web Start, so the launch line is typically not PlatynUI's to
  change. PlatynUI speaks the attach protocol **natively** (Unix: trigger file +
  `SIGQUIT` + the JVM's socket; Windows: a remote thread invoking
  `JVM_EnqueueOperation`), so the test host needs no JDK — and, unlike bundling
  `jattach`, ships no unsigned foreign binary performing `OpenProcess` /
  `CreateRemoteThread`.
- **`-javaagent:...` at launch** — the **durable fallback**: the JDK-blessed
  path ("explicit grant of privileges" since JDK 5), for hosts where attach is
  blocked and for the day JEP 451 lands. Requires controlling the launch.

[JEP 451](https://openjdk.org/jeps/451) ("Prepare to Disallow the Dynamic Loading
of Agents") puts the Attach path on a sunset trajectory: JDK 21 warns when an
agent is loaded dynamically, and a future release will disallow it by default.
The opt-in is `-XX:+EnableDynamicAgentLoading`.

**Measured on Temurin JDK 21.0.11** (`java-agent-core` task 1.3), because the
"remedy without editing the command line" claim is load-bearing:

| `EnableDynamicAgentLoading` | Set via | Attach | JEP 451 warning |
|---|---|---|---|
| unset | — | works | printed |
| `+` | command line | works | **silent** |
| `+` | `JAVA_TOOL_OPTIONS` | works | still printed |
| `-` | `JAVA_TOOL_OPTIONS` | **refused** | — |

So `JAVA_TOOL_OPTIONS` genuinely **controls the flag** — proven by the refusal in
the last row — which is what makes the future sunset remediable per environment
without touching how an application is started. Only the cosmetic *warning
suppression* needs the flag on the real command line. The warning is no loss
either way: it makes a dynamic load visible in the target's own log, which is the
transparency an instrumenting tool should want.

A refused load is reported as **agent-refused**, distinct from a failed attach,
and carries the target's own remedy text (`"Dynamic agent loading is not
enabled. Use -XX:+EnableDynamicAgentLoading to launch target VM."`) — the two
failures need different things from the operator, so they must not collapse into
one message.

JEP 451 targets *agent loading only*; JVM discovery/listing (`jcmd -l`, `jps`,
`VirtualMachine.list()`, via `hsperfdata`) and non-agent attach operations are
unaffected. PlatynUI does not use any of them: it reaches a JVM through the
window that owns it, so machine-wide enumeration answers a question it never
asks.

## Routing summary

- **SWT, everywhere** → native provider (UIA / AT-SPI / NSAccessibility). No
  Java-specific handling needed.
- **JavaFX** → native provider on Windows/macOS; **agent-only on Linux**.
- **Swing/AWT** → JAB on Windows (zero-config fallback) or the agent (full
  fidelity); agent on Linux (wrapper off); NSAccessibility or agent on macOS.
- Cross-provider arbitration uses the process-wide `window_claims` registry
  (`platynui_core::platform::window_claims`): whichever provider handles a Java
  window claims its HWND and the others abstain. A preferred-provider priority
  (agent > JAB > native-for-FX/SWT) is part of the forward design.

## Getting the agent

The agent artifact ships in its own package, `platynui-provider-java`, pulled in
by the `[java]` extras — **installing it is the consent** for in-JVM
instrumentation, and uninstalling removes the capability:

```sh
pip install "robotframework-PlatynUI[java]"     # a test run
pip install "platynui-inspector[java]"          # the Inspector
```

Without it, nothing is injected and the diagnostic names the install. Provider
and agent versions must match **exactly** (an agent cannot be unloaded from a
JVM, so a mismatch's only remedy is restarting the application); the extras pin
them together, and the connection handshake refuses a mismatch rather than
degrading. `platynui-provider-java agent-path` prints the JAR path for the
hand-written `-javaagent:` case.
