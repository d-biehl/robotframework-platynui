//! Local PID-namespace harness for the AT-SPI process identity.
//!
//! Builds real PID-namespace topologies with `unshare` around a real bus daemon
//! and asserts what only a real daemon can show: the outcome each topology
//! decides, how every peer is classified, and what the attribute path would be
//! allowed to read. It drives a plain bus without an AT-SPI registry, so it
//! asserts the identity helper's outputs — the values the attribute path
//! consumes — rather than rendered nodes.
//!
//! It lives inside the crate because the identity module is private to it; a
//! test under `tests/` would only see the public API.
//!
//! Run it once per bus implementation:
//!
//! ```sh
//! just test-atspi-pidns dbus-daemon
//! just test-atspi-pidns dbus-broker
//! ```
//!
//! Prerequisites, each of which **fails** the run with a message naming it and
//! never skips: `unshare` with unprivileged user namespaces permitted, control
//! over the next PID inside them (`/proc/sys/kernel/ns_last_pid`, for the forced
//! collision), and the bus daemon the run is parameterised with — `dbus-daemon`, or
//! `dbus-broker-launch` started through `systemd-socket-activate`, which also
//! needs a user session: the launcher talks to systemd over the user bus, as a
//! client, the way every dbus-broker session does. An ignored
//! test only runs when someone asks for exactly this coverage, so a skip would
//! read as a pass in the one run meant to exercise it.
//!
//! The topologies:
//!
//! - **Supported sidecar** — the daemon and two applications in one namespace,
//!   the runtime in a sibling, and one more application in a namespace the
//!   daemon cannot see.
//! - **Runtime beside the daemon** — the daemon and the runtime share a
//!   namespace, an application lives in a sibling. The provider keeps an
//!   identity of its own while that application has none.
//! - **Forced collision** — the supported sidecar with the runtime's own PID
//!   forced onto the application's in-namespace PID.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::identity::{self, Answer, BusDaemon, Credentials};

const ROLE_ENV: &str = "PLATYNUI_PIDNS_ROLE";
const DAEMON_ENV: &str = "PLATYNUI_PIDNS_DAEMON";
const DIR_ENV: &str = "PLATYNUI_PIDNS_DIR";
const LABEL_ENV: &str = "PLATYNUI_PIDNS_LABEL";
const PEERS_ENV: &str = "PLATYNUI_PIDNS_PEERS";

/// The test this binary re-executes itself as, for one member of a topology.
const ROLE_TEST: &str = "pidns_harness::harness_role";

/// A user namespace that keeps our uid — the bus daemon authenticates peers by
/// it — and keeps the capabilities that let the forced-collision topology set
/// the next PID of its own PID namespace.
const UNSHARE: [&str; 7] =
    ["--user", "--map-current-user", "--keep-caps", "--pid", "--fork", "--mount-proc", "--kill-child"];

/// How often the forced collision is attempted before the run fails.
const COLLISION_ATTEMPTS: usize = 5;

// ── Members ─────────────────────────────────────────────────────────────────

/// One member of a topology, when this binary was started as one — and nothing
/// otherwise, so the ignored test is harmless in a run that forces ignored tests.
#[test]
#[ignore = "a member of the PID-namespace harness, started by `identity_across_pid_namespaces`"]
fn harness_role() {
    let Ok(role) = std::env::var(ROLE_ENV) else {
        return;
    };
    let dir = PathBuf::from(std::env::var(DIR_ENV).expect("a member is always given its topology directory"));
    let label = std::env::var(LABEL_ENV).expect("a member is always given a label");
    let address = format!("unix:path={}", dir.join("bus").display());
    match role.as_str() {
        "peer" => run_peer(&dir, &label, &address),
        "probe" => run_probe(&dir, &label, &address),
        other => panic!("unknown harness role `{other}`"),
    }
}

/// An application stand-in: a bus connection whose unique name and in-namespace
/// PID it publishes, held open until the topology is torn down.
fn run_peer(dir: &Path, label: &str, address: &str) {
    let bus = crate::connection::connect_a11y_bus_with(Some(address)).expect("a peer must reach the bus");
    let name = bus.connection().unique_name().expect("a connected peer has a unique name").to_string();
    write_record(
        &dir.join(format!("peer-{label}")),
        &[("name".into(), name), ("pid".into(), std::process::id().to_string())],
    );

    let deadline = Instant::now() + Duration::from_secs(120);
    while !dir.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(bus);
}

/// The runtime: decides the connection's numbering through the production path
/// and classifies every peer it is given, recording what it found.
fn run_probe(dir: &Path, label: &str, address: &str) {
    let bus = crate::connection::connect_a11y_bus_with(Some(address)).expect("the probe must reach the bus");
    let conn = bus.connection();
    let own_name = conn.unique_name().expect("a connected probe has a unique name").to_string();

    // The input the outcome is decided from, recorded on its own so the run
    // names what it exercised.
    let reported_for_self = match BusDaemon::new(conn).process_id(&own_name) {
        Answer::Definitive(reported) => format!("{reported:?}"),
        Answer::Transient => "transient".to_string(),
    };

    // Our own connection first: the counter-control that makes "nothing is ours"
    // a finding rather than a filter that never matches anything.
    let own = identity::peer_of(conn, &own_name);
    let numbering = identity::for_connection(conn).and_then(|state| state.known_numbering());

    let mut record = vec![
        ("own_pid".to_string(), std::process::id().to_string()),
        ("reported_for_self".to_string(), reported_for_self),
        ("numbering".to_string(), format!("{numbering:?}")),
        ("self.is_own".to_string(), own.is_own.to_string()),
    ];

    let peers = std::env::var(PEERS_ENV).unwrap_or_default();
    for peer in peers.split(',').filter(|peer| !peer.is_empty()) {
        let published = read_record(&dir.join(format!("peer-{peer}")));
        let classified = identity::peer_of(conn, &published["name"]);
        // What the attribute path would read with the number it is allowed to
        // use: the command line names the process it was read from.
        let command_line = classified.local_number.and_then(crate::process::query_command_line).unwrap_or_default();
        record.extend([
            (format!("{peer}.number"), format!("{:?}", classified.number)),
            (format!("{peer}.is_own"), classified.is_own.to_string()),
            (format!("{peer}.local_number"), format!("{:?}", classified.local_number)),
            (format!("{peer}.id"), format!("{:?}", crate::node::application_id(classified.number, || None))),
            (format!("{peer}.command_line"), command_line),
        ]);
    }

    write_record(&dir.join(format!("probe-{label}")), &record);
}

// ── The topologies ──────────────────────────────────────────────────────────

#[test]
#[ignore = "needs unprivileged user namespaces and a bus daemon; run with `just test-atspi-pidns <daemon>`"]
fn identity_across_pid_namespaces() {
    let daemon = Daemon::from_env();
    require_user_namespaces();
    require_next_pid_control();
    daemon.require();
    let root = HarnessDir::new();
    let exe = std::env::current_exe().expect("the test binary has a path");

    println!("bus daemon: {}", daemon.version());
    supported_sidecar(&daemon, &root, &exe);
    runtime_beside_the_daemon(&daemon, &root, &exe);
    forced_collision(&daemon, &root, &exe);
}

/// The daemon and two applications in one namespace, the runtime in a sibling,
/// one more application in a namespace the daemon cannot see.
fn supported_sidecar(daemon: &Daemon, root: &HarnessDir, exe: &Path) {
    let dir = root.topology("t1");
    let app = Namespace::start(
        &dir,
        "app",
        &[
            daemon.launch(&dir),
            wait_for_socket(&dir),
            member(exe, &dir, "peer", "a1", &[]),
            member(exe, &dir, "peer", "a2", &[]),
        ],
    );
    let other = Namespace::start(&dir, "other", &[wait_for_socket(&dir), member(exe, &dir, "peer", "c1", &[])]);
    let runtime = Namespace::start(
        &dir,
        "runtime",
        &[
            wait_for_socket(&dir),
            wait_for_peers(&dir, &["a1", "a2", "c1"]),
            member(exe, &dir, "probe", "p", &["a1", "a2", "c1"]),
        ],
    );
    let namespaces = [&app, &other, &runtime];
    let probe = wait_for_record(&dir, "probe-p", &namespaces);
    report("supported sidecar", daemon, &probe, &["a1", "a2", "c1"]);

    assert_eq!(probe["numbering"], "Some(Unknown)", "a daemon that cannot see the runtime gives no identity");
    assert_eq!(probe["self.is_own"], "false", "without identity nothing is ours, not even our own connection");
    for app in ["a1", "a2"] {
        let pid = peer_pid(&dir, app);
        assert_eq!(
            probe[&format!("{app}.number")],
            format!("Some({pid})"),
            "{app}: the number its own namespace knows it by"
        );
        assert_eq!(probe[&format!("{app}.is_own")], "false", "{app}");
        assert_eq!(probe[&format!("{app}.local_number")], "None", "{app}: that number is not ours to read /proc with");
        assert_eq!(probe[&format!("{app}.command_line")], "", "{app}: nothing is read from the process table");
    }
    assert_unresolved(&probe, "c1");
    stop(&dir);
}

/// The daemon and the runtime share a namespace; an application lives in a
/// sibling. The provider keeps its own identity while the application has none.
fn runtime_beside_the_daemon(daemon: &Daemon, root: &HarnessDir, exe: &Path) {
    let dir = root.topology("t2");
    let shared = Namespace::start(
        &dir,
        "daemon+runtime",
        &[
            daemon.launch(&dir),
            wait_for_socket(&dir),
            member(exe, &dir, "peer", "a1", &[]),
            wait_for_peers(&dir, &["a1", "c1"]),
            member(exe, &dir, "probe", "p", &["a1", "c1"]),
        ],
    );
    let other = Namespace::start(&dir, "other", &[wait_for_socket(&dir), member(exe, &dir, "peer", "c1", &[])]);
    let namespaces = [&shared, &other];
    let probe = wait_for_record(&dir, "probe-p", &namespaces);
    report("runtime beside the daemon", daemon, &probe, &["a1", "c1"]);

    assert_eq!(probe["numbering"], "Some(Local)", "a daemon that sees the runtime numbers it as it numbers itself");
    assert_eq!(probe["self.is_own"], "true", "our own connection is recognised as ours");

    let pid = peer_pid(&dir, "a1");
    assert_eq!(probe["a1.number"], format!("Some({pid})"));
    assert_eq!(probe["a1.is_own"], "false", "a co-located application is not ours");
    assert_eq!(probe["a1.local_number"], format!("Some({pid})"), "under local numbering its number is ours to use");
    let command_line = &probe["a1.command_line"];
    assert!(
        command_line.contains("marker-a1") && !command_line.contains("marker-p"),
        "the process table must describe that application, not the harness's probe: {command_line}"
    );

    // The combination in which a filter trusting a peer's bare number would
    // claim a foreign application as its own.
    assert_unresolved(&probe, "c1");
    stop(&dir);
}

/// The supported sidecar with the runtime's own PID forced onto the
/// application's in-namespace PID.
fn forced_collision(daemon: &Daemon, root: &HarnessDir, exe: &Path) {
    let dir = root.topology("t3");
    let app = Namespace::start(
        &dir,
        "app",
        &[daemon.launch(&dir), wait_for_socket(&dir), member(exe, &dir, "peer", "a1", &[])],
    );
    let target = peer_pid_waiting(&dir, "a1", &[&app]);
    assert!(target > 1, "the application cannot be PID 1 of its namespace: that is the shell");

    for attempt in 1..=COLLISION_ATTEMPTS {
        let label = format!("p{attempt}");
        // Set the next PID of the fresh namespace immediately before forking the
        // probe; everything before it — the waits — has already taken its PIDs.
        let runtime = Namespace::start(
            &dir,
            &format!("runtime-{attempt}"),
            &[
                wait_for_socket(&dir),
                wait_for_peers(&dir, &["a1"]),
                // A failed write must not fork the probe anyway: it would land
                // on an arbitrary PID, and the attempt would be spent unseen.
                format!("echo {} > /proc/sys/kernel/ns_last_pid || exit 1", target - 1),
                member(exe, &dir, "probe", &label, &["a1"]),
            ],
        );
        let probe = wait_for_record(&dir, &format!("probe-{label}"), &[&app, &runtime]);
        if probe["own_pid"] != target.to_string() {
            println!("forced collision: attempt {attempt} landed on PID {} instead of {target}", probe["own_pid"]);
            continue;
        }
        report("forced collision", daemon, &probe, &["a1"]);

        assert_eq!(probe["a1.number"], format!("Some({target})"), "the application reports the runtime's own PID");
        assert_eq!(probe["numbering"], "Some(Unknown)");
        assert_eq!(probe["a1.is_own"], "false", "a collision must not make the application ours: nothing is dropped");
        assert_eq!(probe["a1.local_number"], "None", "the colliding number is not ours to read /proc with");
        assert_eq!(
            probe["a1.command_line"], "",
            "neither the harness binary's own data nor any other process's comes back"
        );
        stop(&dir);
        return;
    }
    panic!("the forced collision never took effect in {COLLISION_ATTEMPTS} attempts; the topology was not set up");
}

/// A peer the daemon cannot see: no number, never `0`, not ours, and no node
/// identifier derived from a process ID.
fn assert_unresolved(probe: &HashMap<String, String>, peer: &str) {
    assert_eq!(probe[&format!("{peer}.number")], "None", "{peer}: the daemon cannot see it");
    assert_eq!(probe[&format!("{peer}.is_own")], "false", "{peer}: an unresolved identity never matches");
    assert_eq!(probe[&format!("{peer}.local_number")], "None", "{peer}");
    assert_eq!(probe[&format!("{peer}.id")], "None", "{peer}: no identifier from a process ID, and never \"0\"");
}

fn report(topology: &str, daemon: &Daemon, probe: &HashMap<String, String>, peers: &[&str]) {
    println!(
        "[{topology}] {} — outcome {} from reported {} for own PID {}",
        daemon.name(),
        probe["numbering"],
        probe["reported_for_self"],
        probe["own_pid"],
    );
    for peer in peers {
        println!(
            "    {peer}: number {} · ours {} · local {}",
            probe[&format!("{peer}.number")],
            probe[&format!("{peer}.is_own")],
            probe[&format!("{peer}.local_number")],
        );
    }
}

// ── Prerequisites ───────────────────────────────────────────────────────────

fn require_user_namespaces() {
    match Command::new("unshare").args(UNSHARE).arg("true").output() {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            panic!("prerequisite missing: `unshare` is not installed (util-linux)")
        }
        Err(err) => panic!("prerequisite missing: cannot run `unshare`: {err}"),
        Ok(output) if !output.status.success() => panic!(
            "prerequisite missing: unprivileged user namespaces are unavailable (`unshare {}` exited with {}: {}). \
             Allow them with `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`, and check that \
             `user.max_user_namespaces` is not 0.",
            UNSHARE.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
        ),
        Ok(_) => {}
    }
}

/// The forced collision sets the next PID of its own namespace. That takes a
/// kernel with checkpoint/restore support and the capability `--keep-caps` keeps.
fn require_next_pid_control() {
    let output = Command::new("unshare")
        .args(UNSHARE)
        .args(["sh", "-c", "echo 100 > /proc/sys/kernel/ns_last_pid"])
        .output()
        .unwrap_or_else(|err| panic!("prerequisite missing: cannot run `unshare`: {err}"));
    assert!(
        output.status.success(),
        "prerequisite missing: the forced-collision topology cannot set the next PID of its own PID namespace \
         (writing /proc/sys/kernel/ns_last_pid inside `unshare {}` failed: {}). It needs a kernel built with \
         CONFIG_CHECKPOINT_RESTORE, and CAP_SYS_ADMIN or CAP_CHECKPOINT_RESTORE kept into the namespace.",
        UNSHARE.join(" "),
        String::from_utf8_lossy(&output.stderr).trim(),
    );
}

/// The bus implementation a run uses; the recipe passes it.
enum Daemon {
    DbusDaemon,
    DbusBroker,
}

impl Daemon {
    fn from_env() -> Self {
        match std::env::var(DAEMON_ENV).as_deref() {
            Ok("dbus-daemon") => Self::DbusDaemon,
            Ok("dbus-broker") => Self::DbusBroker,
            Ok(other) => panic!("unknown bus daemon `{other}`: {DAEMON_ENV} takes `dbus-daemon` or `dbus-broker`"),
            Err(_) => panic!(
                "prerequisite missing: {DAEMON_ENV} names the bus daemon this run uses — run the harness through \
                 `just test-atspi-pidns dbus-daemon` or `just test-atspi-pidns dbus-broker`"
            ),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::DbusDaemon => "dbus-daemon",
            Self::DbusBroker => "dbus-broker",
        }
    }

    fn binaries(&self) -> &'static [&'static str] {
        match self {
            Self::DbusDaemon => &["dbus-daemon"],
            Self::DbusBroker => &["systemd-socket-activate", "dbus-broker-launch"],
        }
    }

    /// Fail unless every binary this daemon needs can be started, and — for
    /// dbus-broker — unless its launcher can reach the user bus it talks to
    /// systemd over.
    fn require(&self) {
        for binary in self.binaries() {
            if let Err(err) = Command::new(binary).arg("--version").output() {
                panic!("prerequisite missing: `{binary}` cannot be started ({err}), and this run uses {}", self.name());
            }
        }
        if matches!(self, Self::DbusBroker)
            && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
            && std::env::var_os("XDG_RUNTIME_DIR").is_none()
        {
            panic!(
                "prerequisite missing: `dbus-broker-launch` reaches systemd over the user bus, and neither \
                 DBUS_SESSION_BUS_ADDRESS nor XDG_RUNTIME_DIR is set — run it from a user session"
            );
        }
    }

    fn version(&self) -> String {
        let binary = self.binaries().last().copied().unwrap_or("?");
        Command::new(binary)
            .arg("--version")
            .output()
            .ok()
            .and_then(|output| String::from_utf8_lossy(&output.stdout).lines().next().map(str::to_owned))
            .unwrap_or_else(|| self.name().to_string())
    }

    /// The shell line that starts this daemon on the topology's socket, in the
    /// background of the namespace's shell.
    fn launch(&self, dir: &Path) -> String {
        let socket = dir.join("bus");
        match self {
            Self::DbusDaemon => {
                format!(
                    "dbus-daemon --session --nofork --address={} &",
                    quoted(&format!("unix:path={}", socket.display()))
                )
            }
            // `systemd-socket-activate` starts its child with an empty environment,
            // and `dbus-broker-launch` needs the user bus to reach systemd; without
            // it the launcher exits with ENOMEDIUM moments after the first answer.
            Self::DbusBroker => format!(
                "systemd-socket-activate -E DBUS_SESSION_BUS_ADDRESS -E XDG_RUNTIME_DIR -l {} \
                 dbus-broker-launch --scope user &",
                quoted(&socket.display().to_string())
            ),
        }
    }
}

// ── Namespaces and their members ────────────────────────────────────────────

/// One PID namespace of a topology. Dropping it kills the `unshare` parent, and
/// `--kill-child` takes the whole namespace down with it.
struct Namespace {
    child: Child,
    log: PathBuf,
    label: String,
}

impl Namespace {
    fn start(dir: &Path, label: &str, lines: &[String]) -> Self {
        let mut script = String::from("set -u\n");
        for line in lines {
            script.push_str(line);
            script.push('\n');
        }
        // Wait for the background members instead of letting the shell exit, and
        // keep every member a real fork — a trailing command would be `exec`ed.
        script.push_str("wait\n");

        let log = dir.join(format!("ns-{}.log", label.replace('+', "-")));
        let out = std::fs::File::create(&log).expect("the topology directory is writable");
        let err = out.try_clone().expect("a log file handle can be duplicated");
        let child = Command::new("unshare")
            .args(UNSHARE)
            .args(["sh", "-c", &script])
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .unwrap_or_else(|err| panic!("prerequisite missing: cannot run `unshare`: {err}"));
        Self { child, log, label: label.to_string() }
    }

    fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

impl Drop for Namespace {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A member of a namespace: this test binary, re-executed in a role. The
/// `--skip marker-<label>` filter matches nothing; it only puts the label on the
/// member's command line, so a process-table read can be told apart.
fn member(exe: &Path, dir: &Path, role: &str, label: &str, peers: &[&str]) -> String {
    format!(
        "{ROLE_ENV}={role} {LABEL_ENV}={label} {DIR_ENV}={dir} {PEERS_ENV}={peers} {exe} --exact {ROLE_TEST} \
         --include-ignored --nocapture --skip marker-{label} &",
        dir = quoted(&dir.display().to_string()),
        peers = peers.join(","),
        exe = quoted(&exe.display().to_string()),
    )
}

/// `text` as one shell word: the test binary lives wherever the checkout or
/// `CARGO_TARGET_DIR` is, and that path may hold anything.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn wait_for_socket(dir: &Path) -> String {
    format!("while [ ! -S {} ]; do sleep 0.05; done", quoted(&dir.join("bus").display().to_string()))
}

fn wait_for_peers(dir: &Path, peers: &[&str]) -> String {
    peers
        .iter()
        .map(|peer| {
            let record = dir.join(format!("peer-{peer}"));
            format!("while [ ! -f {} ]; do sleep 0.05; done", quoted(&record.display().to_string()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for_record(dir: &Path, name: &str, namespaces: &[&Namespace]) -> HashMap<String, String> {
    let path = dir.join(name);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        if Instant::now() > deadline {
            let logs: String = namespaces
                .iter()
                .map(|namespace| format!("--- namespace {} ---\n{}\n", namespace.label, namespace.log()))
                .collect();
            panic!("`{name}` never appeared in {}\n{logs}", dir.display());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    read_record(&path)
}

fn peer_pid(dir: &Path, peer: &str) -> u32 {
    read_record(&dir.join(format!("peer-{peer}")))["pid"].parse().expect("a peer publishes its PID as a number")
}

fn peer_pid_waiting(dir: &Path, peer: &str, namespaces: &[&Namespace]) -> u32 {
    wait_for_record(dir, &format!("peer-{peer}"), namespaces)["pid"].parse().expect("a peer publishes its PID")
}

/// End a topology: the peers see the stop file and leave the bus.
fn stop(dir: &Path) {
    let _ = std::fs::write(dir.join("stop"), "");
}

// ── Records ─────────────────────────────────────────────────────────────────

/// Write `key=value` lines, through a rename so a reader never sees half a file.
fn write_record(path: &Path, fields: &[(String, String)]) {
    let text: String = fields.iter().map(|(key, value)| format!("{key}={value}\n")).collect();
    let partial = path.with_extension("partial");
    std::fs::write(&partial, text).expect("the topology directory is writable");
    std::fs::rename(&partial, path).expect("a rename within one directory succeeds");
}

fn read_record(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// The harness's directory: short, because the bus socket lives in it and a
/// unix socket path holds at most 107 bytes. Removed when the harness ends.
struct HarnessDir(PathBuf);

impl HarnessDir {
    fn new() -> Self {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.is_dir())
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let dir = base.join(format!("pidns-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .unwrap_or_else(|err| panic!("prerequisite missing: cannot create {}: {err}", dir.display()));
        assert!(
            dir.as_os_str().len() < 90,
            "the harness directory {} is too long for the bus socket inside it",
            dir.display()
        );
        // The bus address names the socket in it, and an address holds these
        // characters unescaped.
        assert!(
            dir.to_string_lossy().chars().all(|c| c.is_ascii_alphanumeric() || "-_/.".contains(c)),
            "the harness directory {} must be usable in a bus address without escaping",
            dir.display()
        );
        Self(dir)
    }

    fn topology(&self, name: &str) -> PathBuf {
        let dir = self.0.join(name);
        std::fs::create_dir_all(&dir).expect("the harness directory is writable");
        dir
    }
}

impl Drop for HarnessDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
