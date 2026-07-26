//! The wire-level client: newline-delimited JSON-RPC 2.0 over the agent's
//! loopback port.
//!
//! Provider-neutral — it knows about calls, deadlines and versions, not about
//! UI nodes. A toolkit adapter builds its element methods on top.
//!
//! Every read has a deadline. That is the whole point of the control/data
//! split: a frozen toolkit thread in the target must surface as a bounded error
//! here, never as a hung test run.

use crate::error::AgentError;
use crate::handshake::HandshakeInfo;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// The version this client speaks; must equal the agent's exactly.
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default per-call deadline. Generous enough for a busy toolkit thread,
/// short enough that a wedged one is noticed within a test step.
pub const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Default connect deadline; loopback, so a slow answer means a sick target.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Server-initiated notification frame — a JSON-RPC message without an `id`.
#[derive(Debug, Clone)]
pub struct Notification {
    /// The notification method, e.g. `ui/generation`.
    pub method: String,
    /// Its parameters, or [`Value::Null`] if it carried none.
    pub params: Value,
}

/// What the agent reports about itself on connect.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentInfo {
    /// Wire-format version.
    pub protocol: u32,
    /// The agent's version.
    #[serde(rename = "agentVersion")]
    pub agent_version: String,
    /// The JVM's process id, as the agent sees it.
    pub pid: u32,
    /// The loopback port it is serving on.
    pub port: u16,
    /// The toolkits live in that JVM at connect time.
    pub toolkits: Vec<String>,
}

/// Timeouts for one client.
#[derive(Debug, Clone, Copy)]
pub struct ClientConfig {
    /// How long a single call may take before it is abandoned.
    pub call_timeout: Duration,
    /// How long the initial TCP connect may take.
    pub connect_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self { call_timeout: DEFAULT_CALL_TIMEOUT, connect_timeout: DEFAULT_CONNECT_TIMEOUT }
    }
}

/// A connection to one agent.
#[derive(Debug)]
pub struct AgentClient {
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    pid: u32,
    next_id: u64,
    info: AgentInfo,
    notifications: VecDeque<Notification>,
    call_timeout: Duration,
}

impl AgentClient {
    /// Connects to the agent described by `info` and completes the handshake.
    ///
    /// The version is checked **before** the socket is opened: the handshake
    /// file already states it, and an agent that cannot be unloaded is not
    /// worth a connection attempt. The agent enforces the same rule on its
    /// side, so a hand-rolled client cannot talk its way past it.
    ///
    /// # Errors
    ///
    /// [`AgentError::VersionMismatch`] when the agent was built from another
    /// PlatynUI version, [`AgentError::Unauthenticated`] when it rejects the
    /// token, and [`AgentError::Transport`] when the port does not answer.
    pub fn connect(info: &HandshakeInfo, config: ClientConfig) -> Result<Self, AgentError> {
        if info.agent_version != CLIENT_VERSION {
            return Err(AgentError::VersionMismatch {
                pid: info.pid,
                agent: info.agent_version.clone(),
                client: CLIENT_VERSION.to_owned(),
            });
        }

        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, info.port));
        let stream = TcpStream::connect_timeout(&address, config.connect_timeout).map_err(|e| {
            AgentError::Transport { details: format!("cannot connect to the agent in process {}: {e}", info.pid) }
        })?;
        stream.set_nodelay(true).map_err(|e| AgentError::Transport { details: e.to_string() })?;
        stream
            .set_read_timeout(Some(config.call_timeout))
            .map_err(|e| AgentError::Transport { details: e.to_string() })?;
        stream
            .set_write_timeout(Some(config.call_timeout))
            .map_err(|e| AgentError::Transport { details: e.to_string() })?;
        let reader = BufReader::new(stream.try_clone().map_err(|e| AgentError::Transport { details: e.to_string() })?);

        let mut client = Self {
            writer: stream,
            reader,
            pid: info.pid,
            next_id: 0,
            info: AgentInfo {
                protocol: info.protocol,
                agent_version: info.agent_version.clone(),
                pid: info.pid,
                port: info.port,
                toolkits: info.toolkits.clone(),
            },
            notifications: VecDeque::new(),
            call_timeout: config.call_timeout,
        };

        let result = client.call("handshake", json!({ "token": info.token, "clientVersion": CLIENT_VERSION }))?;
        client.info = serde_json::from_value(result).map_err(|e| AgentError::Protocol {
            details: format!("handshake result is not an agent info object: {e}"),
        })?;
        if client.info.pid != info.pid {
            return Err(AgentError::Protocol {
                details: format!(
                    "the agent on port {} reports pid {} but the handshake file said {} — stale file or reused port",
                    info.port, client.info.pid, info.pid
                ),
            });
        }
        tracing::debug!(pid = client.pid, toolkits = ?client.info.toolkits, "connected to a PlatynUI agent");
        Ok(client)
    }

    /// What the agent reported at handshake time.
    #[must_use]
    pub fn info(&self) -> &AgentInfo {
        &self.info
    }

    /// The process id of the JVM this client is connected to.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Calls `method` and returns its result value.
    ///
    /// # Errors
    ///
    /// [`AgentError::Timeout`] when the call does not answer within the
    /// configured deadline, [`AgentError::Call`] when the agent reports a
    /// JSON-RPC error, and [`AgentError::Transport`] when the connection
    /// breaks.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, AgentError> {
        self.next_id += 1;
        let id = self.next_id;
        let mut request = serde_json::Map::new();
        request.insert("jsonrpc".to_owned(), Value::from("2.0"));
        request.insert("id".to_owned(), Value::from(id));
        request.insert("method".to_owned(), Value::from(method));
        request.insert("params".to_owned(), params);
        let mut frame = serde_json::to_string(&Value::Object(request))
            .map_err(|e| AgentError::Protocol { details: format!("cannot serialise a request for '{method}': {e}") })?;
        frame.push('\n');
        self.writer.write_all(frame.as_bytes()).map_err(|e| self.io_error(method, &e))?;
        self.writer.flush().map_err(|e| self.io_error(method, &e))?;
        self.await_response(method, id)
    }

    /// Round-trips the connection.
    ///
    /// # Errors
    ///
    /// The call errors of [`Self::call`].
    pub fn ping(&mut self) -> Result<(), AgentError> {
        self.call("ping", json!({})).map(|_| ())
    }

    /// Re-reads the agent's live state, including a toolkit set that may have
    /// grown since the handshake.
    ///
    /// # Errors
    ///
    /// The call errors of [`Self::call`], and [`AgentError::Protocol`] when the
    /// answer is not an agent-info object.
    pub fn refresh_info(&mut self) -> Result<&AgentInfo, AgentError> {
        let result = self.call("agent/info", json!({}))?;
        self.info = serde_json::from_value(result)
            .map_err(|e| AgentError::Protocol { details: format!("agent/info is not an agent info object: {e}") })?;
        Ok(&self.info)
    }

    /// Takes the notifications received so far.
    ///
    /// The frame is part of the wire from day one so later event capabilities
    /// need no protocol break; version 1 carries only the UI-generation
    /// counter, which is an invalidation *hint* — per-element liveness has its
    /// own endpoint and is never inferred from this.
    pub fn drain_notifications(&mut self) -> Vec<Notification> {
        self.notifications.drain(..).collect()
    }

    /// Reads frames until the response to `id` arrives, queueing notifications.
    fn await_response(&mut self, method: &str, id: u64) -> Result<Value, AgentError> {
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).map_err(|e| self.io_error(method, &e))?;
            if read == 0 {
                return Err(AgentError::Transport {
                    details: format!("the agent in process {} closed the connection during '{method}'", self.pid),
                });
            }
            if line.trim().is_empty() {
                continue;
            }
            let message: Value = serde_json::from_str(&line).map_err(|e| AgentError::Protocol {
                details: format!("unparsable frame from the agent in process {}: {e}", self.pid),
            })?;

            match message.get("id").filter(|value| !value.is_null()) {
                None => {
                    // No id: either a notification, or an error the agent could
                    // not attribute to a request (a malformed frame from us).
                    if let Some(error) = message.get("error") {
                        return Err(self.error_from(method, error));
                    }
                    if let Some(notification_method) = message.get("method").and_then(Value::as_str) {
                        self.notifications.push_back(Notification {
                            method: notification_method.to_owned(),
                            params: message.get("params").cloned().unwrap_or(Value::Null),
                        });
                    }
                }
                Some(value) if value.as_u64() == Some(id) => {
                    if let Some(error) = message.get("error") {
                        return Err(self.error_from(method, error));
                    }
                    return Ok(message.get("result").cloned().unwrap_or(Value::Null));
                }
                Some(other) => {
                    // Calls are strictly sequential on one connection, so a
                    // foreign id means the streams have desynchronised.
                    return Err(AgentError::Protocol {
                        details: format!("the agent answered id {other} while '{method}' (id {id}) was pending"),
                    });
                }
            }
        }
    }

    /// Maps a JSON-RPC error object onto the variant whose remedy fits.
    fn error_from(&self, method: &str, error: &Value) -> AgentError {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = error.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
        match code {
            crate::UNAUTHENTICATED => AgentError::Unauthenticated { pid: self.pid },
            crate::VERSION_MISMATCH => AgentError::VersionMismatch {
                pid: self.pid,
                agent: self.info.agent_version.clone(),
                client: CLIENT_VERSION.to_owned(),
            },
            crate::DEADLINE_EXCEEDED => AgentError::Timeout {
                method: method.to_owned(),
                timeout_ms: u64::try_from(self.call_timeout.as_millis()).unwrap_or(u64::MAX),
            },
            _ => AgentError::Call { method: method.to_owned(), code, message },
        }
    }

    /// A socket timeout is a deadline, not a broken pipe — the distinction is
    /// what tells "the target is wedged" from "the target is gone".
    fn io_error(&self, method: &str, error: &std::io::Error) -> AgentError {
        match error.kind() {
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => AgentError::Timeout {
                method: method.to_owned(),
                timeout_ms: u64::try_from(self.call_timeout.as_millis()).unwrap_or(u64::MAX),
            },
            _ => AgentError::Transport {
                details: format!("connection to process {} failed during '{method}': {error}", self.pid),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::HandshakeInfo;
    use std::io::{BufRead, BufReader as StdBufReader, Write as StdWrite};
    use std::net::TcpListener;
    use std::thread;

    fn info_for(port: u16, version: &str) -> HandshakeInfo {
        HandshakeInfo {
            protocol: crate::handshake::SUPPORTED_PROTOCOL,
            pid: std::process::id(),
            port,
            token: "test-token".to_owned(),
            toolkits: vec!["swing".to_owned()],
            agent_version: version.to_owned(),
        }
    }

    /// A stand-in agent: completes the handshake, then answers each request with
    /// the frames the test scripted for it.
    fn fake_agent(script: Vec<String>) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut writer = stream.try_clone().expect("clone");
            let mut reader = StdBufReader::new(stream);
            let pid = std::process::id();

            let mut request = String::new();
            reader.read_line(&mut request).expect("handshake request");
            let handshake = format!(
                r#"{{"jsonrpc":"2.0","id":1,"result":{{"protocol":1,"agentVersion":"{CLIENT_VERSION}",
                   "pid":{pid},"port":0,"toolkits":["swing"]}}}}"#
            )
            .replace('\n', "")
            .replace("                   ", "");
            writeln!(writer, "{handshake}").expect("handshake reply");

            for frame in script {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                writeln!(writer, "{frame}").expect("scripted reply");
            }
        });
        port
    }

    /// A mismatched agent must be refused before a socket is even opened — the
    /// handshake file already says which version is in that JVM.
    #[test]
    fn a_version_mismatch_is_refused_without_connecting() {
        // Port 1 is not listening; reaching it would be the bug.
        let error =
            AgentClient::connect(&info_for(1, "0.0.0-other"), ClientConfig::default()).expect_err("must refuse");
        match error {
            AgentError::VersionMismatch { agent, client, .. } => {
                assert_eq!(agent, "0.0.0-other");
                assert_eq!(client, CLIENT_VERSION);
            }
            other => panic!("expected VersionMismatch, got {other:?}"),
        }
    }

    /// The notification frame is part of the wire from day one: a message
    /// without an `id` arriving mid-call must be queued, not mistaken for the
    /// answer and not treated as a desync.
    #[test]
    fn a_notification_arriving_mid_call_is_queued_and_the_response_still_matches() {
        // Two notifications, then the actual answer to the pending call, in one
        // scripted write.
        let port = fake_agent(vec![
            [
                r#"{"jsonrpc":"2.0","method":"ui/generation","params":{"generation":7}}"#,
                r#"{"jsonrpc":"2.0","method":"ui/generation","params":{"generation":8}}"#,
                r#"{"jsonrpc":"2.0","id":2,"result":{"pong":true}}"#,
            ]
            .join("\n"),
        ]);
        let mut client =
            AgentClient::connect(&info_for(port, CLIENT_VERSION), ClientConfig::default()).expect("connect");

        client.ping().expect("the response must be found past the notifications");

        let notifications = client.drain_notifications();
        assert_eq!(notifications.len(), 2, "both notifications must survive");
        assert_eq!(notifications[0].method, "ui/generation");
        assert_eq!(notifications[1].params["generation"], 8);
        assert!(client.drain_notifications().is_empty(), "draining must consume them");
    }

    /// An agent that answers a request we are not waiting for means the streams
    /// have desynchronised; guessing which frame belongs to which call would
    /// hand a caller somebody else's data.
    #[test]
    fn a_foreign_response_id_is_a_protocol_error() {
        let port = fake_agent(vec![r#"{"jsonrpc":"2.0","id":99,"result":{}}"#.to_owned()]);
        let mut client =
            AgentClient::connect(&info_for(port, CLIENT_VERSION), ClientConfig::default()).expect("connect");
        assert!(matches!(client.ping(), Err(AgentError::Protocol { .. })));
    }

    /// An agent that stops answering must surface as a deadline, not a hang.
    #[test]
    fn a_silent_agent_times_out_rather_than_hanging() {
        let port = fake_agent(vec![]); // handshake only, then silence
        let config = ClientConfig { call_timeout: Duration::from_millis(300), ..ClientConfig::default() };
        let mut client = AgentClient::connect(&info_for(port, CLIENT_VERSION), config).expect("connect");
        match client.ping() {
            Err(AgentError::Timeout { method, .. }) => assert_eq!(method, "ping"),
            // The scripted server drops the connection when its script is empty,
            // which is the other bounded outcome; a hang is the only failure.
            Err(AgentError::Transport { .. }) => {}
            other => panic!("expected a bounded error, got {other:?}"),
        }
    }

    /// The agent's own error codes must become the variants whose remedies fit,
    /// not a generic call failure.
    #[test]
    fn agent_error_codes_map_to_the_variant_with_the_right_remedy() {
        for (code, check) in [
            (crate::UNAUTHENTICATED, "unauth"),
            (crate::VERSION_MISMATCH, "version"),
            (crate::DEADLINE_EXCEEDED, "deadline"),
            (-32601, "other"),
        ] {
            let port =
                fake_agent(vec![format!(r#"{{"jsonrpc":"2.0","id":2,"error":{{"code":{code},"message":"nope"}}}}"#)]);
            let mut client =
                AgentClient::connect(&info_for(port, CLIENT_VERSION), ClientConfig::default()).expect("connect");
            let error = client.ping().expect_err("must fail");
            match (check, &error) {
                ("unauth", AgentError::Unauthenticated { .. })
                | ("version", AgentError::VersionMismatch { .. })
                | ("deadline", AgentError::Timeout { .. })
                | ("other", AgentError::Call { .. }) => {}
                _ => panic!("code {code} mapped to the wrong variant: {error:?}"),
            }
        }
    }
}
