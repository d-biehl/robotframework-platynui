//! Process identity on an accessibility bus connection.
//!
//! Every process ID on this bus is a value in the *bus daemon's* PID namespace.
//! Whether that namespace is the runtime's own is decided once per connection,
//! by asking the daemon what it reports for **our own** connection and comparing
//! that with `getpid()` — the one number we can check an answer against. A peer's
//! number on its own says nothing: a `0` for a peer the daemon cannot see looks
//! exactly like a `0` from a daemon that cannot see anybody, and a real number
//! from a daemon in an ancestor namespace belongs to somebody else.
//!
//! The decision is a comparison of two numbers the provider already has: no
//! syscall, no `/proc` read, no file descriptor. See the `sidecar-deployment`
//! capability for the behaviour each outcome costs.

/// Whether the process IDs a bus daemon reports are values in the runtime's own
/// namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Numbering {
    /// **Local numbering** — the daemon reported our own process ID for our own
    /// connection. Its numbers may be compared with ours and read through
    /// `/proc`.
    Local,
    /// **No identity** — the daemon has no process ID for us, or one that is not
    /// ours. No own-process check is possible, and none of its numbers is valid
    /// in the runtime's namespace.
    Unknown,
}

/// A process ID the daemon reported, as an identity: `0` is not a process, and
/// every shape of "cannot tell" — an omitted field, an explicit
/// `UnixProcessIdUnknown`, a failed lookup — reaches this module as `None`.
fn usable(reported: Option<u32>) -> Option<u32> {
    reported.filter(|pid| *pid > 0)
}

/// Decide a connection's numbering from what the daemon reports for **our own**
/// connection.
pub(crate) fn decide(reported_for_self: Option<u32>, own_pid: u32) -> Numbering {
    match usable(reported_for_self) {
        Some(reported) if reported == own_pid && own_pid > 0 => Numbering::Local,
        _ => Numbering::Unknown,
    }
}

/// Whether a peer is the runtime's own process — only ever on a positive match
/// of two numbers in one namespace, never on an unresolved one.
pub(crate) fn peer_is_own(numbering: Numbering, reported_for_peer: Option<u32>, own_pid: u32) -> bool {
    match numbering {
        Numbering::Local => usable(reported_for_peer) == usable(Some(own_pid)),
        Numbering::Unknown => false,
    }
}

/// The process ID to report for a peer: what the daemon said, never `0`. It is
/// the number the application's own environment knows it by, reported whether or
/// not it is valid in the runtime's namespace.
pub(crate) fn peer_number(reported_for_peer: Option<u32>) -> Option<u32> {
    usable(reported_for_peer)
}

/// The process ID the local process table may be read with: a peer's number, but
/// only where the daemon numbers processes the way the runtime does.
pub(crate) fn local_number(numbering: Numbering, reported_for_peer: Option<u32>) -> Option<u32> {
    match numbering {
        Numbering::Local => usable(reported_for_peer),
        Numbering::Unknown => None,
    }
}

/// What a credentials lookup produced.
///
/// The distinction is the caching rule: an answer is remembered, a call that did
/// not complete is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    /// The daemon answered. `Some(pid)` is the process ID it reported, `None`
    /// means it cannot tell — an omitted field, an explicit
    /// `UnixProcessIdUnknown`, or a name it does not know. Both are definitive.
    Definitive(Option<u32>),
    /// The call did not complete: a timeout, an I/O error, a closed connection.
    /// Not an answer, so nothing is remembered and the next occasion asks again.
    Transient,
}

/// The bus daemon, as this module needs it: one question, asked about one name.
pub(crate) trait Credentials {
    /// The process ID the daemon reports for the connection of `bus_name`.
    fn process_id(&self, bus_name: &str) -> Answer;
}

/// What the provider knows about one peer on a bus connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PeerIdentity {
    /// The process ID the daemon reported, never `0` — the number the peer's own
    /// environment knows it by, whether or not it is valid here.
    pub(crate) number: Option<u32>,
    /// Whether this peer is the runtime's own process. Only ever true on a
    /// positive match of two numbers in one namespace.
    pub(crate) is_own: bool,
    /// The process ID the local process table may be read with, if any.
    pub(crate) local_number: Option<u32>,
}

impl PeerIdentity {
    /// What a peer is while nothing is known about it: not ours, no number, and
    /// nothing `/proc` may be asked about.
    pub(crate) const fn unknown() -> Self {
        Self { number: None, is_own: false, local_number: None }
    }
}

/// The identity state of one bus connection: its numbering, decided once on
/// first use, and what the daemon has answered about each peer since.
///
/// The provider holds more than one connection (its own and the popup watcher's
/// two), and each decides and remembers for itself.
pub(crate) struct ConnectionIdentity {
    own_pid: u32,
    own_name: String,
    numbering: std::sync::Mutex<Option<Numbering>>,
    /// The daemon's definitive answer per peer — the reported number, not a
    /// classification. A peer can be asked about while the numbering is still
    /// undecided, and a classification stored then would outlive the decision.
    peers: std::sync::Mutex<std::collections::HashMap<String, Option<u32>>>,
}

impl ConnectionIdentity {
    pub(crate) fn new(own_pid: u32, own_name: String) -> Self {
        Self {
            own_pid,
            own_name,
            numbering: std::sync::Mutex::new(None),
            peers: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// The unique name of the connection this state belongs to — the name whose
    /// credentials decide the numbering.
    pub(crate) fn own_name(&self) -> &str {
        &self.own_name
    }

    /// The numbering, if it has already been decided. No lookup, so a caller
    /// that must not block can ask first.
    pub(crate) fn known_numbering(&self) -> Option<Numbering> {
        *self.numbering.lock().expect("identity numbering mutex poisoned")
    }

    /// Record what the daemon answered about our own connection. A call that did
    /// not complete is not remembered, so the next occasion decides again rather
    /// than freezing the connection into *no identity*.
    ///
    /// The decision is recorded once per connection, with the inputs it was taken
    /// from, so a reader can redo it: at `info` for local numbering, and as a
    /// warning for *no identity*, the outcome that loses a behaviour.
    pub(crate) fn record_numbering(&self, answer: Answer) -> Numbering {
        let Answer::Definitive(reported) = answer else {
            return Numbering::Unknown;
        };
        let mut decided = self.numbering.lock().expect("identity numbering mutex poisoned");
        if let Some(numbering) = *decided {
            // Another caller decided first; the record exists already.
            return numbering;
        }
        let numbering = decide(reported, self.own_pid);
        *decided = Some(numbering);
        match numbering {
            Numbering::Local => tracing::info!(
                connection = %self.own_name,
                reported = ?reported,
                own_pid = self.own_pid,
                "AT-SPI process identity: local numbering",
            ),
            Numbering::Unknown => tracing::warn!(
                connection = %self.own_name,
                reported = ?reported,
                own_pid = self.own_pid,
                "AT-SPI process identity: no identity, own-process exclusion is inactive on this connection",
            ),
        }
        numbering
    }

    /// The peer behind `bus_name`, if the daemon has already answered about it.
    pub(crate) fn known_peer(&self, bus_name: &str) -> Option<PeerIdentity> {
        let reported = *self.peers.lock().expect("identity peer mutex poisoned").get(bus_name)?;
        Some(self.classify(reported))
    }

    /// Record what the daemon answered about `bus_name`. Only a definitive answer
    /// is remembered.
    pub(crate) fn record_peer(&self, bus_name: &str, answer: Answer) -> PeerIdentity {
        match answer {
            Answer::Definitive(reported) => {
                self.peers.lock().expect("identity peer mutex poisoned").insert(bus_name.to_string(), reported);
                self.classify(reported)
            }
            Answer::Transient => PeerIdentity::unknown(),
        }
    }

    /// Classify a peer's reported number under the numbering decided so far.
    ///
    /// While the numbering is undecided — the lookup for our own connection did
    /// not complete — the peer is classified as under *no identity*: nothing is
    /// excluded and nothing is read. That holds for this occasion only, because
    /// the classification is derived on every read and never stored.
    fn classify(&self, reported: Option<u32>) -> PeerIdentity {
        let numbering = self.known_numbering().unwrap_or(Numbering::Unknown);
        PeerIdentity {
            number: peer_number(reported),
            is_own: peer_is_own(numbering, reported, self.own_pid),
            local_number: local_number(numbering, reported),
        }
    }

    /// The connection's numbering, decided on first use from `creds`.
    pub(crate) fn numbering(&self, creds: &dyn Credentials) -> Numbering {
        match self.known_numbering() {
            Some(numbering) => numbering,
            None => self.record_numbering(creds.process_id(&self.own_name)),
        }
    }

    /// Classify the peer behind `bus_name`, asking `creds` only when the answer
    /// is not already known.
    pub(crate) fn peer(&self, creds: &dyn Credentials, bus_name: &str) -> PeerIdentity {
        self.numbering(creds);
        match self.known_peer(bus_name) {
            Some(peer) => peer,
            None => self.record_peer(bus_name, creds.process_id(bus_name)),
        }
    }

    /// Forget everything: the connection's numbering and every peer answer.
    pub(crate) fn clear(&self) {
        *self.numbering.lock().expect("identity numbering mutex poisoned") = None;
        self.peers.lock().expect("identity peer mutex poisoned").clear();
    }
}

/// The bus daemon behind a live connection.
pub(crate) struct BusDaemon<'a> {
    conn: &'a zbus::Connection,
}

impl<'a> BusDaemon<'a> {
    pub(crate) fn new(conn: &'a zbus::Connection) -> Self {
        Self { conn }
    }
}

impl Credentials for BusDaemon<'_> {
    fn process_id(&self, bus_name: &str) -> Answer {
        let Ok(name) = zbus::names::BusName::try_from(bus_name) else {
            // Not a bus name at all; asking again would not change that.
            return Answer::Definitive(None);
        };
        let replied = crate::timeout::block_on_timeout_call(async {
            let dbus = zbus::fdo::DBusProxy::new(self.conn).await?;
            dbus.get_connection_credentials(name).await
        });
        match replied {
            // The daemon replied. For a peer it cannot see, it reports `0` or
            // leaves the process ID out, depending on the implementation, and
            // `usable` reads both as "cannot tell". The dedicated
            // `GetConnectionUnixProcessID` would add a third shape, an error.
            Some(Ok(credentials)) => Answer::Definitive(credentials.process_id()),
            Some(Err(err)) => classify_error(&err),
            // No reply within the timeout.
            None => Answer::Transient,
        }
    }
}

/// Classify a failed credentials call: did the daemon answer, or did the call not
/// complete? Shared by the blocking path and the popup watcher's async one.
pub(crate) fn classify_error(err: &zbus::fdo::Error) -> Answer {
    use zbus::fdo::Error as Fdo;
    match err {
        // A reply carrying an error name we do not model is still a reply.
        Fdo::ZBus(zbus::Error::MethodError(..)) => Answer::Definitive(None),
        // The call itself did not complete, however it was reported (any other
        // zbus-level failure included) — or the daemon ran out of memory or hit
        // a quota, which a later call may not.
        Fdo::NoReply(_)
        | Fdo::IOError(_)
        | Fdo::Timeout(_)
        | Fdo::TimedOut(_)
        | Fdo::Disconnected(_)
        | Fdo::NoMemory(_)
        | Fdo::LimitsExceeded(_)
        | Fdo::ZBus(_) => Answer::Transient,
        // Every other D-Bus error is a reply: the daemon knows nothing about that
        // name, or declines to say. Asking again says the same.
        _ => Answer::Definitive(None),
    }
}

/// One bus connection: the GUID of the bus instance it is connected to, and its
/// unique name there. A unique name alone is unique only within one daemon, and
/// every daemon hands them out from the same counter — while a process can run
/// several providers, each bound to a bus of its own (`providers.atspi.bus_address`).
type ConnectionKey = (String, String);

/// The identity state of every bus connection in this process.
///
/// Keyed rather than threaded through every node: "once per connection" is a
/// property of the connection, and everything that needs the answer already
/// holds one. A provider has three (its own and the popup watcher's two).
static CONNECTIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<ConnectionKey, std::sync::Arc<ConnectionIdentity>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// The key of `conn`. `None` while the connection has no unique name of its own
/// — there is nothing to ask about yet.
fn key_of(conn: &zbus::Connection) -> Option<ConnectionKey> {
    Some((conn.server_guid().to_string(), conn.unique_name()?.to_string()))
}

/// The identity state of the connection behind `key`, created on first use.
fn identity_for(key: ConnectionKey) -> std::sync::Arc<ConnectionIdentity> {
    let mut connections = CONNECTIONS.lock().expect("identity registry mutex poisoned");
    let own_name = key.1.clone();
    let identity = connections
        .entry(key)
        .or_insert_with(move || std::sync::Arc::new(ConnectionIdentity::new(std::process::id(), own_name)));
    std::sync::Arc::clone(identity)
}

/// The identity state of `conn`, created on first use. `None` while the
/// connection has no unique name of its own.
pub(crate) fn for_connection(conn: &zbus::Connection) -> Option<std::sync::Arc<ConnectionIdentity>> {
    key_of(conn).map(identity_for)
}

/// Classify the peer behind `bus_name` on `conn`, deciding that connection's
/// numbering first if that has not happened yet.
pub(crate) fn peer_of(conn: &zbus::Connection, bus_name: &str) -> PeerIdentity {
    let Some(identity) = for_connection(conn) else {
        return PeerIdentity::unknown();
    };
    identity.peer(&BusDaemon::new(conn), bus_name)
}

/// Forget the identity state of `conn`. Called for each of a provider's
/// connections when it shuts down, which is also when they go away; every other
/// provider in the process keeps its own.
pub(crate) fn forget(conn: &zbus::Connection) {
    if let Some(key) = key_of(conn) {
        forget_key(&key);
    }
}

/// The state is cleared as well as dropped from the registry: a caller that took
/// an `Arc` of it earlier would otherwise keep answering from state this
/// shutdown was meant to discard.
fn forget_key(key: &ConnectionKey) {
    if let Some(identity) = CONNECTIONS.lock().expect("identity registry mutex poisoned").remove(key) {
        identity.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: u32 = 4242;
    const SOMEBODY_ELSE: u32 = 99;

    // ── The per-connection decision (spec: Process identity is decided once
    //    per bus connection from the daemon's view of that connection) ───────

    #[test]
    fn a_daemon_that_reports_our_own_number_for_us_numbers_locally() {
        assert_eq!(decide(Some(OURS), OURS), Numbering::Local);
    }

    #[test]
    fn a_reported_zero_for_our_own_connection_gives_no_identity() {
        // dbus-daemon 1.12/1.14 and dbus-broker 29/33 answer `0` for a peer
        // they cannot see — including us, across a namespace boundary.
        assert_eq!(decide(Some(0), OURS), Numbering::Unknown);
    }

    #[test]
    fn an_omitted_number_for_our_own_connection_gives_no_identity() {
        // dbus-broker 35/37 and dbus-daemon >= 1.15.10 omit the field instead,
        // and an explicit "unknown" answer reaches this function the same way.
        assert_eq!(decide(None, OURS), Numbering::Unknown);
    }

    #[test]
    fn a_number_that_is_not_ours_gives_no_identity() {
        // A daemon in an ancestor namespace numbers us with a real number that
        // is somebody else's here. The self-check is what catches it.
        assert_eq!(decide(Some(SOMEBODY_ELSE), OURS), Numbering::Unknown);
    }

    #[test]
    fn zero_is_never_an_identity_on_either_side() {
        assert_eq!(decide(Some(0), 0), Numbering::Unknown);
    }

    // ── Peer classification (spec: An unresolved identity never matches) ────

    #[test]
    fn under_local_numbering_a_peer_reporting_our_number_is_ours() {
        assert!(peer_is_own(Numbering::Local, Some(OURS), OURS));
    }

    #[test]
    fn under_local_numbering_an_unresolved_peer_is_not_ours() {
        assert!(!peer_is_own(Numbering::Local, Some(0), OURS));
        assert!(!peer_is_own(Numbering::Local, None, OURS));
        assert!(!peer_is_own(Numbering::Local, Some(SOMEBODY_ELSE), OURS));
    }

    #[test]
    fn without_identity_nothing_is_ours_not_even_our_own_number() {
        assert!(!peer_is_own(Numbering::Unknown, Some(OURS), OURS));
        assert!(!peer_is_own(Numbering::Unknown, Some(0), OURS));
        assert!(!peer_is_own(Numbering::Unknown, None, OURS));
    }

    // ── The caching rule (spec: Only definitive identity answers are
    //    remembered; design D6) ───────────────────────────────────────────────

    /// A daemon whose answers the test dictates, counting every call.
    struct FakeDaemon {
        answers: std::collections::HashMap<String, Vec<Answer>>,
        calls: std::cell::RefCell<Vec<String>>,
    }

    impl FakeDaemon {
        fn new(answers: &[(&str, Vec<Answer>)]) -> Self {
            Self {
                answers: answers.iter().map(|(name, a)| ((*name).to_string(), a.clone())).collect(),
                calls: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn calls_for(&self, bus_name: &str) -> usize {
            self.calls.borrow().iter().filter(|name| *name == bus_name).count()
        }
    }

    impl Credentials for FakeDaemon {
        fn process_id(&self, bus_name: &str) -> Answer {
            let nth = self.calls_for(bus_name);
            self.calls.borrow_mut().push(bus_name.to_string());
            let answers = self.answers.get(bus_name).expect("the test dictates an answer for every name asked");
            answers.get(nth).or_else(|| answers.last()).copied().expect("at least one answer")
        }
    }

    const OWN_NAME: &str = ":1.7";
    const PEER: &str = ":1.9";

    fn identity() -> ConnectionIdentity {
        ConnectionIdentity::new(OURS, OWN_NAME.to_string())
    }

    #[test]
    fn a_resolved_identity_is_asked_for_once() {
        let daemon = FakeDaemon::new(&[
            (OWN_NAME, vec![Answer::Definitive(Some(OURS))]),
            (PEER, vec![Answer::Definitive(Some(SOMEBODY_ELSE))]),
        ]);
        let id = identity();

        for _ in 0..3 {
            let peer = id.peer(&daemon, PEER);
            assert_eq!(peer.number, Some(SOMEBODY_ELSE));
            assert!(!peer.is_own);
        }

        assert_eq!(daemon.calls_for(OWN_NAME), 1, "the connection is decided once");
        assert_eq!(daemon.calls_for(PEER), 1, "a resolved peer is asked for once");
    }

    #[test]
    fn a_definitive_cannot_resolve_is_remembered_and_still_means_not_ours() {
        let daemon = FakeDaemon::new(&[
            (OWN_NAME, vec![Answer::Definitive(Some(OURS))]),
            (PEER, vec![Answer::Definitive(None)]),
        ]);
        let id = identity();

        for _ in 0..3 {
            let peer = id.peer(&daemon, PEER);
            assert_eq!(peer.number, None);
            assert!(!peer.is_own);
            assert_eq!(peer.local_number, None);
        }

        assert_eq!(daemon.calls_for(PEER), 1, "a definitive 'cannot tell' is remembered");
    }

    #[test]
    fn a_transient_failure_is_not_remembered_and_is_retried() {
        let daemon = FakeDaemon::new(&[
            (OWN_NAME, vec![Answer::Definitive(Some(OURS))]),
            (PEER, vec![Answer::Transient, Answer::Definitive(Some(SOMEBODY_ELSE))]),
        ]);
        let id = identity();

        let first = id.peer(&daemon, PEER);
        assert_eq!(first.number, None, "a call that did not complete is not an answer");
        assert!(!first.is_own);

        let second = id.peer(&daemon, PEER);
        assert_eq!(second.number, Some(SOMEBODY_ELSE), "the next occasion asks again");
        assert_eq!(daemon.calls_for(PEER), 2);
    }

    #[test]
    fn a_failed_decision_is_retried_rather_than_freezing_the_connection() {
        let daemon = FakeDaemon::new(&[
            (OWN_NAME, vec![Answer::Transient, Answer::Definitive(Some(OURS))]),
            (PEER, vec![Answer::Definitive(Some(OURS))]),
        ]);
        let id = identity();

        assert_eq!(id.numbering(&daemon), Numbering::Unknown, "no answer yet, so no identity yet");
        assert_eq!(id.numbering(&daemon), Numbering::Local, "the decision is taken again, not frozen");
        assert_eq!(daemon.calls_for(OWN_NAME), 2);
    }

    #[test]
    fn a_peer_asked_about_before_the_decision_is_classified_again_after_it() {
        // Our own lookup times out while the peer's succeeds: the peer's answer
        // is definitive and remembered, but the classification taken under the
        // undecided numbering must not be.
        let daemon = FakeDaemon::new(&[
            (OWN_NAME, vec![Answer::Transient, Answer::Definitive(Some(OURS))]),
            (PEER, vec![Answer::Definitive(Some(OURS))]),
        ]);
        let id = identity();

        let before = id.peer(&daemon, PEER);
        assert!(!before.is_own, "nothing is excluded while the numbering is undecided");
        assert_eq!(before.local_number, None, "nothing is read while the numbering is undecided");

        let after = id.peer(&daemon, PEER);
        assert!(after.is_own, "once decided, the remembered answer is classified under the decision");
        assert_eq!(after.local_number, Some(OURS));
        assert_eq!(daemon.calls_for(PEER), 1, "the peer's definitive answer is still asked for once");
    }

    #[test]
    fn shutdown_discards_everything_cached() {
        let daemon = FakeDaemon::new(&[
            (OWN_NAME, vec![Answer::Definitive(Some(OURS))]),
            (PEER, vec![Answer::Definitive(Some(SOMEBODY_ELSE))]),
        ]);
        let id = identity();

        id.peer(&daemon, PEER);
        id.clear();
        id.peer(&daemon, PEER);

        assert_eq!(daemon.calls_for(OWN_NAME), 2, "the connection is decided again after a clear");
        assert_eq!(daemon.calls_for(PEER), 2, "no peer answer survives a clear");
    }

    // ── Classifying a failed credentials call ────────────────────────────────

    #[test]
    fn an_error_reply_from_the_daemon_is_a_definitive_cannot_tell() {
        use zbus::fdo::Error as Fdo;
        // dbus-daemon >= 1.15.10 for a peer it cannot see.
        assert_eq!(classify_error(&Fdo::UnixProcessIdUnknown(String::new())), Answer::Definitive(None));
        assert_eq!(classify_error(&Fdo::NameHasNoOwner(String::new())), Answer::Definitive(None));
    }

    #[test]
    fn a_call_that_did_not_complete_is_transient() {
        use zbus::fdo::Error as Fdo;
        assert_eq!(classify_error(&Fdo::NoReply(String::new())), Answer::Transient);
        assert_eq!(classify_error(&Fdo::Timeout(String::new())), Answer::Transient);
        assert_eq!(classify_error(&Fdo::Disconnected(String::new())), Answer::Transient);
        assert_eq!(classify_error(&Fdo::ZBus(zbus::Error::InvalidReply)), Answer::Transient);
    }

    #[test]
    fn a_daemon_short_of_resources_is_transient() {
        use zbus::fdo::Error as Fdo;
        // An error reply, but not an answer about the name: a later call may succeed.
        assert_eq!(classify_error(&Fdo::NoMemory(String::new())), Answer::Transient);
        assert_eq!(classify_error(&Fdo::LimitsExceeded(String::new())), Answer::Transient);
    }

    // ── The registry of connections ──────────────────────────────────────────

    /// A key of a bus instance only this test uses, so tests sharing the
    /// process-wide registry cannot see each other's state.
    fn key(test: &str, bus: &str, name: &str) -> ConnectionKey {
        (format!("{test}-{bus}"), name.to_string())
    }

    #[test]
    fn the_same_unique_name_on_two_buses_is_two_connections() {
        // Every daemon counts unique names from the same start, so two freshly
        // started buses hand the same name to their first clients.
        let on_a = identity_for(key("two-buses", "a", ":1.5"));
        let on_b = identity_for(key("two-buses", "b", ":1.5"));
        assert!(!std::sync::Arc::ptr_eq(&on_a, &on_b));
        assert!(std::sync::Arc::ptr_eq(&on_a, &identity_for(key("two-buses", "a", ":1.5"))));

        on_a.record_numbering(Answer::Definitive(Some(std::process::id())));
        assert_eq!(on_a.known_numbering(), Some(Numbering::Local));
        assert_eq!(on_b.known_numbering(), None, "a decision on one bus is not taken for the other");

        forget_key(&key("two-buses", "a", ":1.5"));
        forget_key(&key("two-buses", "b", ":1.5"));
    }

    #[test]
    fn forgetting_one_connection_leaves_every_other_one_alone() {
        let mine = identity_for(key("forget-one", "a", ":1.5"));
        let theirs = identity_for(key("forget-one", "b", ":1.9"));
        mine.record_numbering(Answer::Definitive(Some(std::process::id())));
        theirs.record_numbering(Answer::Definitive(Some(std::process::id())));

        forget_key(&key("forget-one", "a", ":1.5"));

        assert_eq!(mine.known_numbering(), None, "a holder of the forgotten state sees it discarded");
        assert!(!std::sync::Arc::ptr_eq(&mine, &identity_for(key("forget-one", "a", ":1.5"))));
        assert_eq!(theirs.known_numbering(), Some(Numbering::Local), "another provider's decision survives");
        assert!(std::sync::Arc::ptr_eq(&theirs, &identity_for(key("forget-one", "b", ":1.9"))));

        forget_key(&key("forget-one", "a", ":1.5"));
        forget_key(&key("forget-one", "b", ":1.9"));
    }

    // ── The number a peer carries, and whether `/proc` may see it ───────────

    #[test]
    fn a_peers_number_is_what_the_daemon_said_and_never_zero() {
        assert_eq!(peer_number(Some(1234)), Some(1234));
        assert_eq!(peer_number(Some(0)), None);
        assert_eq!(peer_number(None), None);
    }

    #[test]
    fn a_number_reaches_proc_only_under_local_numbering() {
        assert_eq!(local_number(Numbering::Local, Some(1234)), Some(1234));
        assert_eq!(local_number(Numbering::Local, Some(0)), None);
        assert_eq!(local_number(Numbering::Unknown, Some(1234)), None);
        assert_eq!(local_number(Numbering::Unknown, None), None);
    }
}
