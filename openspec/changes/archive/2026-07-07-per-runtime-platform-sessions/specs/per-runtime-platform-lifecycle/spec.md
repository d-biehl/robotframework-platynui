## ADDED Requirements

### Requirement: A runtime owns and releases its platform instances

Each runtime SHALL own the platform devices it uses (pointer, keyboard, screenshot, highlight, window manager, desktop info) as instances created for that runtime, holding their connection through a shared owned handle rather than a process-global static. When the runtime is dropped, its platform instances SHALL be released and their connection closed, and any per-session background threads (for example the highlight overlay thread) SHALL be signalled to stop and joined. There SHALL be no reference-counted, process-global lease governing platform initialization or shutdown.

#### Scenario: Dropping a runtime closes its connection

- **WHEN** a runtime is constructed and then dropped
- **THEN** the platform connection it opened SHALL be closed and its per-session threads SHALL have stopped, with no process-global platform state left initialized

### Requirement: A runtime built after a previous teardown connects freshly

Constructing a runtime after a previous runtime has been fully torn down SHALL establish a new platform connection. Connection state SHALL NOT be memoized in a way that survives teardown and prevents reconnection. A first connection attempt that fails SHALL NOT poison later attempts within the same process.

#### Scenario: Sequential runtimes across suites reconnect (real provider)

- **WHEN** the multi-suite `just test-acceptance-x11` lane runs, so each suite builds a fresh `Runtime` after the previous suite's runtime has been dropped
- **THEN** every suite's runtime SHALL establish a working X11 connection, and no suite SHALL fail with `x11 connection: not available after shutdown or failed connect`

#### Scenario: A failed connect does not poison later attempts

- **WHEN** a runtime construction fails because a platform connection could not be established, and a later construction is attempted after the cause is resolved
- **THEN** the later construction SHALL attempt a fresh connection rather than reusing the cached failure

### Requirement: Runtimes share no platform state

Two runtimes SHALL NOT share any platform connection, device instance, or mutable process-global platform state. One runtime's construction or teardown SHALL NOT affect another runtime's ability to operate. A runtime SHALL connect to exactly the session named by its `config` (or, absent config, the environment-derived session), independent of any other runtime. This holds for every backend converted to per-runtime ownership; a backend that still keeps process-global state (Wayland in this phase — see the proposal's non-goals) satisfies this requirement only once it is internalized.

#### Scenario: One runtime's teardown does not disturb another (real provider, X11)

- **WHEN** two X11 runtimes exist (each with its own connection) and one is dropped
- **THEN** the surviving runtime SHALL continue to query and act against its own session without interruption

#### Scenario: A config-bound runtime connects to the named session (real provider)

- **WHEN** a runtime is constructed with `config` binding it to a specific X11 display
- **THEN** its queries and input SHALL be directed at that display's session, regardless of the ambient `DISPLAY`

### Requirement: Highlight survives across sequential runtimes

The screen-highlight overlay SHALL be functional in every runtime, including runtimes constructed after an earlier runtime has been torn down. The highlight controller SHALL be owned per runtime, not a process-global controller that is emptied on the first shutdown and never rebuilt.

#### Scenario: Highlight works in a later suite's runtime (real provider)

- **WHEN** a highlight is requested from a runtime that was constructed after an earlier runtime in the same process was dropped
- **THEN** the highlight SHALL be drawn and SHALL NOT fail with a "shut down" controller error
