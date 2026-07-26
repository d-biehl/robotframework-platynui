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
        let classes = swing_classes_dir();
        assert!(
            classes.is_dir(),
            "Swing fixture classes not found at {} — run `just build-test-app-swing` first",
            classes.display()
        );
        let title = format!("PlatynUI Agent Live {} {}", std::process::id(), title_suffix);
        let child = Command::new(swing_java_launcher())
            .args(jvm_args)
            .arg("-cp")
            .arg(&classes)
            .arg("platynui.testapp.Main")
            .arg("--title")
            .arg(&title)
            .arg("--auto-close")
            .arg("180")
            .spawn()
            .expect("failed to launch the fixture JVM — set PLATYNUI_TEST_APP_SWING_JAVA or put `java` on PATH");
        Self { child }
    }

    fn pid(&self) -> u32 {
        self.child.id()
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
