# Java UI toolkits: detection and accessibility coverage

How PlatynUI recognizes JVM-backed GUI applications and which accessibility path
reaches each Java UI toolkit, per platform. This is the cross-platform map; the
Windows JAB provider's internals live in [`platform-windows.md`](platform-windows.md)
§2a and the AT-SPI provider in [`platform-linux.md`](platform-linux.md).

Scope: facts as of 2026-07. The *forward design* — an automatic Java-app
classifier and an in-JVM agent provider that would fill the gaps below — is
tracked in OpenSpec, not here; this document records only what is true today and
the settled decisions that constrain that design.

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
| Swing/AWT | JAB (`provider-jab`) — zero-config, limited fidelity | `java-atk-wrapper`/AT-SPI **only if enabled** — see decision below | NSAccessibility (`provider-macos-ax`) |
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
Windows `SunAwtSuspect` diagnostic: a JVM process with windows that is *absent
from the AT-SPI tree* is the agent target; process-level detection finds it, the
agent classifies the toolkit from inside.

## Agent injection paths and JEP 451 (facts)

If/when an in-JVM agent is adopted, there are two ways to get it into the target
JVM — they load the *same* agent, differing only in how:

- **`-javaagent:...` at launch** — the durable, JDK-blessed path ("explicit grant
  of privileges" since JDK 5). Requires controlling the launch.
- **Attach API + `loadAgent()`** — no launch change (good for Java Web Start),
  attaches a running JVM; native attach without a JDK via `jattach`.

[JEP 451](https://openjdk.org/jeps/451) ("Prepare to Disallow the Dynamic Loading
of Agents") puts the Attach path on a sunset trajectory: JDK 21 warns when an
agent is loaded dynamically, and a future release will disallow it by default.
The opt-in is `-XX:+EnableDynamicAgentLoading`, set **at launch of the target**
(or via the `JAVA_TOOL_OPTIONS` environment variable, which the launcher honors
without editing the command line — scoped per environment, not per app). JEP 451
targets *agent loading only*; JVM discovery/listing (`jcmd -l`, `jps`,
`VirtualMachine.list()`, via `hsperfdata`) and non-agent attach operations are
unaffected. Net: `-javaagent` is the durable path; Attach-load is a best-effort
convenience with a documented expiry.

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
