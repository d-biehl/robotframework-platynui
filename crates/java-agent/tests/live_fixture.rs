//! Live checks against a real JVM — the Swing fixture app.
//!
//! Everything the transport actually claims can only be proven against a
//! running JVM: that the native attach reaches it, that the handshake file
//! rendezvous works, that two targets stay distinct, and that two clients share
//! one agent. These tests are `#[ignore]`d so the plain `just test` lane needs
//! no Java at all; the lane that exercises the agent runs them explicitly:
//!
//! ```text
//! cargo nextest run -p platynui-java-agent --run-ignored ignored-only
//! ```
//!
//! Prerequisites: the built fixture (`just build-test-app-swing`) and the built
//! agent JAR (`just build-java-agent`). Both are hard prerequisites of the
//! recipe — a missing artifact fails loudly rather than skipping the coverage.
//!
//! The fixture is a plain JVM here, not a Swing test subject: nothing in this
//! change reads a UI node. It is simply the JVM we have.

// Integration-test ergonomics: scenarios are long and linear, and define their
// expectations next to where they are used.
#![allow(clippy::too_many_lines, clippy::doc_markdown)]

use platynui_java_agent::attach::{self, DEFAULT_ATTACH_TIMEOUT};
use platynui_java_agent::{AgentClient, AgentError, ClientConfig, handshake, jvm, paths};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

// Dependencies of the library that this test target does not use directly
// (`unused_crate_dependencies` is target-scoped).
#[cfg(unix)]
use rustix as _;
use serde as _;
use tempfile as _;
use thiserror as _;
use tracing as _;
#[cfg(windows)]
use windows as _;

/// How long a JVM may take to publish its handshake file after injection.
const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------------------
// Fixture plumbing

/// The launched fixture JVM; killed on drop so a panicking test cleans up.
struct FixtureJvm {
    child: Child,
    title: String,
}

/// Optional launch shapes, for the two conditions a Java Web Start target
/// imposes (OpenSpec `java-agent-web-start`). Both default to off: every test
/// that is not about them launches the fixture exactly as before.
#[derive(Clone, Default)]
struct FixtureModes {
    /// Where the fixture's JVM publishes its handshake file — the agent reads
    /// `PLATYNUI_AGENT_DIR` from the environment it was injected into. A path
    /// the agent cannot create is how a start-up failure is induced from
    /// outside, after the agent is already loaded and running its own code.
    agent_dir: Option<PathBuf>,
    /// Start under a security manager with the trusted-app policy: the fixture's
    /// own code holds all permissions, everything else in the JVM — an injected
    /// agent included — gets the sandbox. The shape of a signed
    /// `<all-permissions/>` JNLP.
    sandboxed: bool,
    /// Build the UI in a second AWT `AppContext`, with the launcher's furniture
    /// left in the first one. The shape Web Start and applet runtimes produce,
    /// in which an observer's own threads are in neither.
    second_app_context: bool,
    /// Also *show* the launcher's window, so both worlds hold something readable.
    /// Without it the first world is invisible by construction, and "one world
    /// wedged, the other still answering" has nothing to answer with.
    companion_window: bool,
    /// Stop the application world's event queue: `(after, for)`, in seconds.
    wedge: Option<(u32, u32)>,
}

impl FixtureJvm {
    /// Launches the fixture **without** an agent — the state the attach path
    /// exists for: an application already running, started by someone else.
    fn launch_bare(title_suffix: &str) -> Self {
        Self::launch(title_suffix, &[])
    }

    /// Launches the fixture with `-javaagent`, the durable fallback path.
    fn launch_with_javaagent(title_suffix: &str) -> Self {
        let argument = format!("-javaagent:{}", agent_jar().display());
        Self::launch(title_suffix, &[argument])
    }

    fn launch(title_suffix: &str, jvm_args: &[String]) -> Self {
        Self::launch_with(title_suffix, jvm_args, &FixtureModes::default())
    }

    /// Launches the fixture in one of the Web Start shapes, or both at once.
    ///
    /// The two conditions are independent — a restrictive policy and a second
    /// toolkit world — and a real Web Start target has both, so they compose.
    fn launch_with(title_suffix: &str, jvm_args: &[String], modes: &FixtureModes) -> Self {
        let classes = swing_classes_dir();
        assert!(
            classes.is_dir(),
            "Swing fixture classes not found at {} — run `just build-test-app-swing` first",
            classes.display()
        );
        let title = format!("PlatynUI Agent Live {} {}", std::process::id(), title_suffix);

        let mut command = Command::new(swing_java_launcher());
        command.args(jvm_args);
        if let Some(directory) = &modes.agent_dir {
            command.env("PLATYNUI_AGENT_DIR", directory);
        }

        if modes.sandboxed {
            let major = java_major_version();
            assert!(
                major < 24,
                "the sandbox mode needs a JDK that still has a security manager: JEP 486 disabled \
                 it permanently in JDK 24, where -Djava.security.manager with any value but \
                 `disallow` makes the JVM refuse to start. The launcher {} is JDK {major} — point \
                 PLATYNUI_TEST_APP_SWING_JAVA at a JDK 23 or older, or drop this coverage \
                 deliberately rather than by a toolchain bump.",
                swing_java_launcher().display()
            );
            command
                .arg("-Djava.security.manager")
                .arg(format!("-Djava.security.policy={}", policy_file().display()))
                .arg(format!("-Dplatynui.fixture.classes={}", url_path(&classes)));
        }
        if modes.second_app_context {
            // `sun.awt` is not exported on JDK 9+ and the fixture needs it to create the second
            // context. JDK_JAVA_OPTIONS carries the flag there and is ignored by Java 8, which
            // needs none — so one environment variable covers both without first asking the
            // launcher which it is.
            command.env("JDK_JAVA_OPTIONS", "--add-exports java.desktop/sun.awt=ALL-UNNAMED");
        }

        command.arg("-cp").arg(&classes).arg("platynui.testapp.Main");
        command.arg("--title").arg(&title).arg("--auto-close").arg("180");
        if modes.sandboxed {
            // The fixture checks that the policy actually reached it. A codeBase matching nothing
            // loads silently and would leave the fixture sandboxed too — a different target.
            command.arg("--require-security-manager");
        }
        if modes.second_app_context {
            command.arg("--app-context");
        }
        if modes.companion_window {
            command.arg("--companion-window");
        }
        if let Some((after, duration)) = modes.wedge {
            command.arg("--wedge-after").arg(after.to_string()).arg("--wedge-for").arg(duration.to_string());
        }

        let child = command
            .spawn()
            .expect("failed to launch the fixture JVM — set PLATYNUI_TEST_APP_SWING_JAVA or put `java` on PATH");
        Self { child, title }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The window title this instance was launched with — also its accessible name.
    fn title(&self) -> &str {
        &self.title
    }

    /// Waits until the JVM is far enough along that its attach listener can
    /// answer. Started too early, an attach races the VM's own initialisation.
    fn wait_until_started(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if jvm::process_runs_jvm(self.pid()) == Some(true) {
                // The module is loaded; give the VM a moment to finish coming up.
                std::thread::sleep(Duration::from_millis(500));
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("the fixture JVM {} never loaded a JVM runtime", self.pid());
    }

    /// Whether the target process is still alive — the question behind "the
    /// agent's failure stayed inside the agent".
    fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for FixtureJvm {
    fn drop(&mut self) {
        self.kill();
    }
}

fn swing_classes_dir() -> PathBuf {
    std::env::var_os("PLATYNUI_TEST_APP_SWING_CLASSES").map_or_else(
        || repo_root().join("apps").join("test-app-swing").join("build").join("classes").join("java").join("main"),
        PathBuf::from,
    )
}

fn swing_java_launcher() -> PathBuf {
    std::env::var_os("PLATYNUI_TEST_APP_SWING_JAVA").map_or_else(|| PathBuf::from("java"), PathBuf::from)
}

fn agent_jar() -> PathBuf {
    let jar = std::env::var_os("PLATYNUI_JAVA_AGENT_JAR").map_or_else(
        || repo_root().join("java").join("agent").join("build").join("libs").join("platynui-agent.jar"),
        PathBuf::from,
    );
    assert!(jar.is_file(), "agent JAR not found at {} — run `just build-java-agent` first", jar.display());
    jar
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn policy_file() -> PathBuf {
    let policy = repo_root().join("apps").join("test-app-swing").join("policy").join("trusted-app.policy");
    assert!(policy.is_file(), "fixture policy file not found at {}", policy.display());
    policy
}

/// A path as a policy file's `codeBase` URL wants it: forward slashes, no
/// trailing separator. A `codeBase` that matches nothing is accepted silently,
/// so this conversion is load-bearing rather than cosmetic.
fn url_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/").trim_end_matches('/').to_string()
}

/// The major version of the JVM the fixture launches on, read from `java -version`.
///
/// Only one launch mode needs it, and it needs it before launching rather than
/// after: a JDK 24 refusing `-Djava.security.manager` looks like a fixture that
/// will not start, with nothing naming the reason.
fn java_major_version() -> u32 {
    let launcher = swing_java_launcher();
    let output = Command::new(&launcher).arg("-version").output().expect("could not run the fixture's java launcher");
    // Both dialects put it in the first quoted token: `java version "1.8.0_442"`
    // before 9, `openjdk version "21.0.12"` from 9 on.
    let text = String::from_utf8_lossy(&output.stderr);
    let quoted = text
        .split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("no version string in `{} -version`: {text}", launcher.display()));
    let mut parts = quoted.split('.');
    let first = parts.next().unwrap_or_default();
    let major = if first == "1" { parts.next().unwrap_or("0") } else { first };
    major
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or_else(|_| panic!("could not read a major version from {quoted:?}"))
}

/// Polls until the JVM has published a handshake file into `directory` — the
/// variant for a fixture pointed at its own `PLATYNUI_AGENT_DIR`.
fn await_agent_in(directory: &std::path::Path, pid: u32) -> handshake::HandshakeInfo {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    while Instant::now() < deadline {
        match handshake::for_pid_in(directory, pid) {
            Ok(Some(info)) => return info,
            Ok(None) => {}
            Err(e) => panic!("handshake file for {pid} in {} is unusable: {e}", directory.display()),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("no agent published itself in process {pid} within {HANDSHAKE_DEADLINE:?}");
}

/// Polls until the JVM has published a handshake file.
fn await_agent(pid: u32) -> handshake::HandshakeInfo {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    while Instant::now() < deadline {
        match handshake::for_pid(pid) {
            Ok(Some(info)) => return info,
            Ok(None) => {}
            Err(e) => panic!("handshake file for {pid} is unusable: {e}"),
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("no agent published itself in process {pid} within {HANDSHAKE_DEADLINE:?}");
}

/// Polls until the agent reports a toolkit — the fixture's Swing classes load
/// after the agent does, so an immediately-empty set is expected, not a bug.
fn await_toolkit(client: &mut AgentClient) -> Vec<String> {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    while Instant::now() < deadline {
        let info = client.refresh_info().expect("agent/info");
        if !info.toolkits.is_empty() {
            return info.toolkits.clone();
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("the agent never reported a toolkit");
}

// ---------------------------------------------------------------------------
// Scenarios

/// The primary path: a running application, started by its own launcher with
/// no PlatynUI arguments, is instrumented **without being restarted**.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn attach_injects_the_agent_into_a_running_jvm() {
    let fixture = FixtureJvm::launch_bare("attach");
    fixture.wait_until_started();
    let pid = fixture.pid();

    assert!(
        handshake::for_pid(pid).expect("handshake lookup").is_none(),
        "the fixture was launched without an agent, so none may be reachable yet"
    );

    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");

    let info = await_agent(pid);
    assert_eq!(info.pid, pid);
    assert!(info.port > 0, "the agent must publish the port it bound");
    assert!(!info.token.is_empty(), "the agent must publish a token");

    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    client.ping().expect("ping");
    assert_eq!(client.info().pid, pid);
    assert!(await_toolkit(&mut client).contains(&"swing".to_owned()), "the fixture is a Swing application");
}

/// Attaching twice must not produce a second agent or a broken one: the agent
/// guards its own start, so the second injection is a no-op.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn attaching_twice_is_harmless() {
    let fixture = FixtureJvm::launch_bare("twice");
    fixture.wait_until_started();
    let pid = fixture.pid();

    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("first attach");
    let first = await_agent(pid);
    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("second attach");
    let second = await_agent(pid);

    assert_eq!(first.port, second.port, "the second injection must not start a second server");
    assert_eq!(first.token, second.token);
    AgentClient::connect(&second, ClientConfig::default()).expect("still connectable").ping().expect("ping");
}

/// The durable fallback: injected at launch, discovered exactly the same way —
/// no port argument, and the token never touches the command line.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn javaagent_at_launch_is_discovered_the_same_way() {
    let fixture = FixtureJvm::launch_with_javaagent("javaagent");
    let pid = fixture.pid();

    let info = await_agent(pid);
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    client.ping().expect("ping");
    assert!(await_toolkit(&mut client).contains(&"swing".to_owned()));
}

/// Two instrumented JVMs at once — the case a fixed or derived port would get
/// wrong, and the reason the OS picks the port and the file publishes it.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn two_concurrent_jvms_publish_distinct_agents() {
    let first = FixtureJvm::launch_with_javaagent("multi-a");
    let second = FixtureJvm::launch_with_javaagent("multi-b");

    let a = await_agent(first.pid());
    let b = await_agent(second.pid());

    assert_ne!(a.pid, b.pid);
    assert_ne!(a.port, b.port, "concurrent targets must not share a port");
    assert_ne!(a.token, b.token, "each agent must have its own token");

    // Each client must reach the JVM its handshake file named, not whichever
    // answered first.
    let mut client_a = AgentClient::connect(&a, ClientConfig::default()).expect("connect a");
    let mut client_b = AgentClient::connect(&b, ClientConfig::default()).expect("connect b");
    assert_eq!(client_a.refresh_info().expect("info a").pid, first.pid());
    assert_eq!(client_b.refresh_info().expect("info b").pid, second.pid());
}

/// A killed JVM never runs its shutdown hook. The file it leaves behind must
/// never lead to a connection — by then the port may belong to anything.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn a_killed_jvm_leaves_a_stale_file_that_is_ignored_and_cleanable() {
    let mut fixture = FixtureJvm::launch_with_javaagent("stale");
    let pid = fixture.pid();
    let info = await_agent(pid);
    let path = paths::handshake_file(pid);
    assert!(path.is_file());

    fixture.kill();
    let deadline = Instant::now() + Duration::from_secs(10);
    while jvm::process_is_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(path.is_file(), "a killed JVM cannot have cleaned up after itself");
    assert!(
        handshake::for_pid(pid).expect("lookup").is_none(),
        "a handshake file whose process is gone must not be offered as an agent"
    );
    assert!(!handshake::agent_present(pid));

    assert!(handshake::remove_stale().contains(&path), "cleanup must remove exactly the dead entries");
    assert!(!path.is_file());
    drop(info);
}

/// The Inspector and a test run are separate processes. Neither may lock the
/// other out — the whole reason the agent's server is multi-client.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn two_clients_share_one_agent() {
    let fixture = FixtureJvm::launch_with_javaagent("multi-client");
    let info = await_agent(fixture.pid());

    let mut inspector = AgentClient::connect(&info, ClientConfig::default()).expect("connect inspector");
    let mut test_run = AgentClient::connect(&info, ClientConfig::default()).expect("connect test run");

    for _ in 0..5 {
        inspector.ping().expect("inspector ping");
        test_run.ping().expect("test-run ping");
    }
    // Interleaved, and after the other has been busy: a serialised-per-process
    // agent would have deadlocked or timed out by now.
    assert_eq!(inspector.refresh_info().expect("inspector info").pid, fixture.pid());
    assert_eq!(test_run.refresh_info().expect("test-run info").pid, fixture.pid());
}

/// The liveness endpoint and the generation counter are what a provider builds
/// node validity and cache invalidation on. No adapter registers elements yet,
/// so what is checkable here is the contract itself: an unknown id is not live,
/// and the counter moves when the UI structurally changes.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn liveness_and_generation_answer_over_the_wire() {
    let fixture = FixtureJvm::launch_with_javaagent("liveness");
    let info = await_agent(fixture.pid());
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");

    let answer = client.call("element/live", serde_json::json!({ "id": 1 })).expect("element/live");
    assert_eq!(answer["live"], false, "an id nobody handed out must never report live");

    let malformed = client.call("element/live", serde_json::json!({})).expect_err("must reject");
    assert!(matches!(malformed, AgentError::Call { .. }), "got {malformed:?}");

    // The toolkit coming up is a structural change, so by the time a toolkit is
    // reported the counter must have moved off zero.
    await_toolkit(&mut client);
    let generation = client.call("ui/generation", serde_json::json!({})).expect("ui/generation");
    assert!(
        generation["generation"].as_u64().expect("a number") >= 1,
        "a toolkit appearing must bump the generation: {generation}"
    );
}

/// Attaching into something that is not a JVM must be refused before any
/// memory is written into it.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn attaching_to_a_non_jvm_process_is_refused() {
    let own_pid = std::process::id();
    let error = attach::load_agent(own_pid, &agent_jar(), None, Duration::from_secs(2))
        .expect_err("this test binary is not a JVM");
    assert!(matches!(error, AgentError::NotAJvm { .. }), "expected NotAJvm, got {error:?}");
}

/// Attaching to a process that does not exist reports the process, not a
/// mysterious transport failure.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn attaching_to_a_dead_process_reports_the_process() {
    let mut fixture = FixtureJvm::launch_bare("dead");
    fixture.wait_until_started();
    let pid = fixture.pid();
    fixture.kill();
    let deadline = Instant::now() + Duration::from_secs(10);
    while jvm::process_is_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }

    let error = attach::load_agent(pid, &agent_jar(), None, Duration::from_secs(2)).expect_err("the process is gone");
    assert!(matches!(error, AgentError::ProcessUnavailable { .. }), "expected ProcessUnavailable, got {error:?}");
}

// ---------------------------------------------------------------------------
// Web Start conditions (OpenSpec `java-agent-web-start`)

/// The shape of a Java Web Start target: the application is trusted, everything
/// else in its JVM is sandboxed. Before the agent's classes were bootstrap-
/// defined it died here on its very first `getenv` — while the attach reported
/// success, which is what made the failure silent rather than merely fatal.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn a_sandboxed_application_still_gets_a_working_agent() {
    let fixture = FixtureJvm::launch_with("sandboxed", &[], &FixtureModes { sandboxed: true, ..Default::default() });
    fixture.wait_until_started();
    let pid = fixture.pid();

    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");

    let info = await_agent(pid);
    // Not incidental: the version comes from a resource the agent reads through its own class,
    // and in this shape that class is bootstrap-defined — the one case where reading it through
    // `getClassLoader()` would have dereferenced null.
    assert!(
        !info.agent_version.is_empty() && info.agent_version != "unknown",
        "the sandboxed agent must still know its own version, got {:?}",
        info.agent_version
    );

    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    client.ping().expect("ping");
    assert!(await_toolkit(&mut client).contains(&"swing".to_owned()), "the fixture is a Swing application");

    let answer = client.call("ui/windows", serde_json::json!({})).expect("ui/windows");
    let windows = answer["windows"].as_array().expect("a windows array");
    assert!(!windows.is_empty(), "the sandboxed application's window must be served: {answer}");
}

/// A start that fails must fail **inside** the agent. The failure is induced
/// from outside — the handshake directory is pointed at a path that cannot be
/// created — so the agent is already loaded and running its own code when it
/// goes wrong, which is the case that once threw an `ExceptionInInitializerError`
/// out of `agentmain` into the target's attach listener thread.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn a_failed_start_never_reaches_the_application() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let barrier = workspace.path().join("blocked");
    std::fs::write(&barrier, b"a file where the agent wants a directory").expect("write barrier");
    let agent_dir = barrier.join("agents");

    let mut fixture = FixtureJvm::launch_with(
        "failed-start",
        &[],
        &FixtureModes { agent_dir: Some(agent_dir.clone()), ..Default::default() },
    );
    fixture.wait_until_started();
    let pid = fixture.pid();

    // The attach succeeds: the JVM loaded the agent and `agentmain` returned normally. Whatever
    // went wrong afterwards is the agent's business and must stay there.
    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");
    std::thread::sleep(Duration::from_secs(2));

    assert!(
        handshake::for_pid_in(&agent_dir, pid).expect("handshake lookup").is_none(),
        "the start was supposed to fail, so nothing may have been published"
    );
    assert!(fixture.is_running(), "the target application must survive an agent that could not start");
}

/// "Already tried" is not "already running". A JVM whose agent failed to start
/// must accept another attempt, or one transient cause makes it unreachable for
/// the rest of its life — and the client's retry budget is spent on a target
/// that has quietly decided never to answer.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn a_failed_start_does_not_disable_the_jvm_for_later_attempts() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let barrier = workspace.path().join("blocked");
    std::fs::write(&barrier, b"a file where the agent wants a directory").expect("write barrier");
    let agent_dir = barrier.join("agents");

    let fixture = FixtureJvm::launch_with(
        "retry-after-failure",
        &[],
        &FixtureModes { agent_dir: Some(agent_dir.clone()), ..Default::default() },
    );
    fixture.wait_until_started();
    let pid = fixture.pid();

    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("first attach");
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        handshake::for_pid_in(&agent_dir, pid).expect("handshake lookup").is_none(),
        "the first start was supposed to fail"
    );

    // Remove the obstacle the first start tripped over — same JVM, same agent, a cause that is
    // simply no longer there.
    std::fs::remove_file(&barrier).expect("remove barrier");

    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("second attach");
    let info = await_agent_in(&agent_dir, pid);
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    client.ping().expect("ping");
    assert_eq!(client.info().pid, pid);
}

/// Polls `ui/windows` until the application's window has appeared.
fn await_windows(client: &mut AgentClient) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + HANDSHAKE_DEADLINE;
    while Instant::now() < deadline {
        let answer = client.call("ui/windows", serde_json::json!({})).expect("ui/windows");
        let windows = answer["windows"].as_array().expect("a windows array").clone();
        if !windows.is_empty() {
            return windows;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("the agent never reported a window");
}

/// The accessible name an element carries, under either of the two keys the
/// payload uses: an explicitly set name, or the accessibility API's own.
fn named(element: &serde_json::Value) -> Option<&str> {
    element["name"].as_str().or_else(|| element["accessibleName"].as_str())
}

/// Walks the tree from `id` and returns the first element carrying `name`.
///
/// Depth-first over whole levels, because the interesting question is whether the
/// tree is readable at all from the outside, not how fast.
fn find_named(client: &mut AgentClient, id: i64, name: &str, depth: u32) -> Option<serde_json::Value> {
    if depth == 0 {
        return None;
    }
    let answer = client.call("ui/children", serde_json::json!({ "id": id })).ok()?;
    let children = answer["children"].as_array()?.clone();
    if let Some(hit) = children.iter().find(|child| named(child) == Some(name)) {
        return Some(hit.clone());
    }
    for child in &children {
        if let Some(found) = child["id"].as_i64().and_then(|id| find_named(client, id, name, depth - 1)) {
            return Some(found);
        }
    }
    None
}

/// An application that lives in its own AWT toolkit world — the Web Start shape —
/// must be served like any other, which before this change it was not: the agent's
/// threads are in a different world (or in none), and the window list it could
/// reach from there was empty.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn an_element_outside_the_agents_own_toolkit_world_is_served() {
    let fixture =
        FixtureJvm::launch_with("app-context", &[], &FixtureModes { second_app_context: true, ..Default::default() });
    fixture.wait_until_started();
    let pid = fixture.pid();
    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");

    let info = await_agent(pid);
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    await_toolkit(&mut client);

    let windows = await_windows(&mut client);
    let window = windows
        .iter()
        .find(|window| named(window) == Some(fixture.title()))
        .unwrap_or_else(|| panic!("the application's window is missing from {windows:?}"));

    // Readable to full depth, not merely present: the table sits six levels down, and reaching it
    // means every intermediate call was dispatched to a queue that actually runs.
    let id = window["id"].as_i64().expect("a window id");
    let table = find_named(&mut client, id, "main-table", 12)
        .expect("the tree must be readable down to the table in the application's own world");
    assert!(table["childCount"].as_i64().unwrap_or(0) > 0, "the table must report its children: {table}");
}

/// The launcher's own windows are not part of the application. A Web Start runtime
/// keeps a shared owner frame and its download dialogs in the world it hosts the
/// application from; none of them is showing by the time the application's window
/// is, and the showing filter is what has to keep them out — now that the agent
/// looks into every world rather than only its own.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn the_launchers_own_windows_are_not_part_of_the_application() {
    let fixture =
        FixtureJvm::launch_with("furniture", &[], &FixtureModes { second_app_context: true, ..Default::default() });
    fixture.wait_until_started();
    let pid = fixture.pid();
    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");

    let info = await_agent(pid);
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    await_toolkit(&mut client);

    let windows = await_windows(&mut client);
    assert!(
        windows.iter().any(|window| named(window) == Some(fixture.title())),
        "the application's window must be there: {windows:?}"
    );
    assert!(
        !windows.iter().any(|window| named(window) == Some("launcher-furniture")),
        "the launcher's never-shown window must not be reported: {windows:?}"
    );
}

/// Several toolkit worlds mean several toolkit threads, and one of them stopping
/// must not take the others with it. Before the dispatcher knew about worlds there
/// was only one queue to wedge and the question could not even be asked.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn one_wedged_toolkit_world_does_not_disable_the_others() {
    let fixture = FixtureJvm::launch_with(
        "wedge",
        &[],
        &FixtureModes { second_app_context: true, companion_window: true, wedge: Some((8, 40)), ..Default::default() },
    );
    fixture.wait_until_started();
    let pid = fixture.pid();
    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");

    let info = await_agent(pid);
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    await_toolkit(&mut client);

    let deadline = Instant::now() + Duration::from_secs(30);
    let (application, companion) = loop {
        let windows = await_windows(&mut client);
        let application = windows.iter().find(|window| named(window) == Some(fixture.title())).cloned();
        let companion = windows.iter().find(|window| named(window) == Some("companion-window")).cloned();
        if let (Some(application), Some(companion)) = (application, companion) {
            break (application, companion);
        }
        assert!(Instant::now() < deadline, "both worlds must show a window before the wedge starts");
        std::thread::sleep(Duration::from_millis(200));
    };
    let application_id = application["id"].as_i64().expect("an id");
    let companion_id = companion["id"].as_i64().expect("an id");

    // Wait for the wedge to actually take hold, rather than assuming the schedule.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if client.call("ui/element", serde_json::json!({ "id": application_id })).is_err() {
            break;
        }
        assert!(Instant::now() < deadline, "the application's event queue never stopped answering");
        std::thread::sleep(Duration::from_millis(250));
    }

    // The point: the other world is untouched. Asked repeatedly, because "answered once" could be
    // a stale cache, and there is none — every read goes to that world's own queue.
    for attempt in 0..3 {
        client
            .call("ui/element", serde_json::json!({ "id": companion_id }))
            .unwrap_or_else(|e| panic!("attempt {attempt}: the healthy world must keep answering, got {e}"));
    }
    assert!(
        client.call("ui/element", serde_json::json!({ "id": application_id })).is_err(),
        "the wedged world must still fail at its deadline"
    );
}

/// Hit-testing has to cross toolkit worlds for the same reason enumeration does:
/// a point over a Web Start application is over a window in a world the agent's
/// threads are not in, and a single-world lookup answers "nothing here" — the
/// quietest possible wrong answer for a picker.
#[test]
#[ignore = "needs a JVM and the built fixture"]
fn a_point_over_another_toolkit_world_returns_its_chain() {
    let fixture =
        FixtureJvm::launch_with("at-point", &[], &FixtureModes { second_app_context: true, ..Default::default() });
    fixture.wait_until_started();
    let pid = fixture.pid();
    attach::load_agent(pid, &agent_jar(), None, DEFAULT_ATTACH_TIMEOUT).expect("native attach");

    let info = await_agent(pid);
    let mut client = AgentClient::connect(&info, ClientConfig::default()).expect("connect");
    await_toolkit(&mut client);

    let windows = await_windows(&mut client);
    let window = windows
        .iter()
        .find(|window| named(window) == Some(fixture.title()))
        .unwrap_or_else(|| panic!("the application's window is missing from {windows:?}"));
    let bounds = &window["bounds"];
    let centre_x = bounds["x"].as_f64().expect("x") + bounds["width"].as_f64().expect("width") / 2.0;
    let centre_y = bounds["y"].as_f64().expect("y") + bounds["height"].as_f64().expect("height") / 2.0;

    let answer = client.call("ui/at_point", serde_json::json!({ "x": centre_x, "y": centre_y })).expect("ui/at_point");
    let chain = answer["chain"].as_array().expect("a chain array");
    assert!(!chain.is_empty(), "a point inside the window must hit something: {answer}");
    assert_eq!(
        named(&chain[0]),
        Some(fixture.title()),
        "the chain must start at the window that owns the point, outermost first: {answer}"
    );
    assert!(chain.len() > 1, "the centre of the window is over its content, so the chain goes deeper: {answer}");
}
