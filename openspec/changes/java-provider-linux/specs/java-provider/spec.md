## ADDED Requirements

### Requirement: Java windows where native accessibility does not enumerate them
The Java provider SHALL be registered and functional on platforms whose native accessibility stack enumerates from an accessibility registry rather than from windows — where an application that never registered has no native representation at all. On such a platform the provider SHALL source a claimed Java window's top-level node from its backend (the in-JVM agent), not from the native provider, because leaving an unserved Java window "to the platform's native provider" there means leaving it invisible rather than degraded. Discovering which JVMs to serve SHALL NOT depend on the native accessibility stack or on a platform Java classifier. With no backend reachable the provider SHALL remain inert: no nodes, no failures, and no cost beyond one discovery pass that finds nothing.

Automatic attachment SHALL work on such a platform too. An in-JVM agent can report the windows of the JVM it is in, but it cannot report a JVM it is not in yet — so on a platform where no backend enumerates native windows, the provider SHALL obtain the candidate JVMs from the **windowing system**: the native top-level windows, each resolved to its owning process by a means the display server vouches for rather than one the client merely claims, and filtered to those actually running a JVM. Without this, "attach automatically to a Java window's JVM" is configured on and structurally unable to fire, which is worse than being off.

This enumeration SHALL remain window-scoped: processes are considered only as owners of windows already under consideration, and machine-wide enumeration of processes or JVMs SHALL NOT be performed — a JVM with no window is never probed and never attached.

#### Scenario: Swing on X11 through the agent
- **WHEN** the Swing fixture runs on an X11 session with a PlatynUI agent in its JVM, and the desktop is enumerated
- **THEN** its window appears exactly once as a top-level node with a working tree, geometry, and window capability patterns — although the process never registered on the accessibility bus and the AT-SPI provider therefore reports nothing for it

#### Scenario: An agent-less Java application on X11 is attached automatically
- **WHEN** a Swing application started by its own script, with no PlatynUI arguments and no agent in its JVM, is running on an X11 session and the desktop is enumerated
- **THEN** its window is found through the windowing system, its owning process is recognised as a JVM, the agent is injected, and the window is served through the agent backend **in that same enumeration** — with no keyword called and no restart

#### Scenario: A window is not followed to a process it only claims
- **WHEN** a window advertises an owning process id that it does not actually belong to
- **THEN** the provider does not offer an agent to that process, because the owner is taken from what the display server vouches for and a client-claimed value is used only when it is corroborated

#### Scenario: A JVM with no window is never touched
- **WHEN** a session runs a JVM that has no windows at all, alongside a Java application that has one
- **THEN** only the windowed application's process is considered, the windowless JVM is neither probed nor attached, and no machine-wide process or JVM enumeration takes place

#### Scenario: Automatic attachment switched off leaves the window unserved, not degraded
- **WHEN** `providers.java.agent.auto_attach` is `false` on Linux and an agent-less Java application is enumerated
- **THEN** nothing is injected and the window is not served at all — unlike on Windows, where the Access Bridge would still serve it — and the diagnostic says so rather than implying a fallback exists

#### Scenario: A JVM without an agent costs nothing
- **WHEN** a runtime is created on Linux, no JVM on the session carries an agent, and no Java window is present
- **THEN** the runtime comes up normally, the Java provider contributes no nodes, and nothing fails

#### Scenario: Wayland is served through XWayland
- **WHEN** the same fixture runs on a Wayland session, where the JDK has no native Wayland backend and the application is an XWayland client
- **THEN** it is served through the same X11 path, with no separate Wayland implementation involved
