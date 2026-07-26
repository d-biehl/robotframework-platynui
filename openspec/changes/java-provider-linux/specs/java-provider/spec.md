## ADDED Requirements

### Requirement: Java windows where native accessibility does not enumerate them
The Java provider SHALL be registered and functional on platforms whose native accessibility stack enumerates from an accessibility registry rather than from windows — where an application that never registered has no native representation at all. On such a platform the provider SHALL source a claimed Java window's top-level node from its backend (the in-JVM agent), not from the native provider, because leaving an unserved Java window "to the platform's native provider" there means leaving it invisible rather than degraded. Discovering which JVMs to serve SHALL NOT depend on the native accessibility stack or on a platform Java classifier. With no backend reachable the provider SHALL remain inert: no nodes, no failures, and no cost beyond one discovery pass that finds nothing.

#### Scenario: Swing on X11 through the agent
- **WHEN** the Swing fixture runs on an X11 session with a PlatynUI agent in its JVM, and the desktop is enumerated
- **THEN** its window appears exactly once as a top-level node with a working tree, geometry, and window capability patterns — although the process never registered on the accessibility bus and the AT-SPI provider therefore reports nothing for it

#### Scenario: A JVM without an agent costs nothing
- **WHEN** a runtime is created on Linux and no JVM on the session carries an agent
- **THEN** the runtime comes up normally, the Java provider contributes no nodes, and nothing fails

#### Scenario: Wayland is served through XWayland
- **WHEN** the same fixture runs on a Wayland session, where the JDK has no native Wayland backend and the application is an XWayland client
- **THEN** it is served through the same X11 path, with no separate Wayland implementation involved
