//! The JVM attach protocol, spoken natively.
//!
//! Both platforms load the same agent and speak the same request/reply shape;
//! only the transport differs. Unix: a trigger file, `SIGQUIT`, and the JVM's
//! own Unix domain socket. Windows: a remote thread invoking
//! `JVM_EnqueueOperation`, with the reply coming back over a named pipe the
//! client creates.
//!
//! *Why not bundle `jattach`?* It solves the same problem and is Apache-2.0,
//! but it costs a third-party binary per OS × architecture in our wheel, turns
//! failures into exit codes and stderr instead of the structured distinction
//! below, and ships an **unsigned foreign** binary whose Windows leg performs
//! `OpenProcess`/`CreateRemoteThread` — an EDR red flag in enterprise
//! environments, where the same operation inside our own signed binary is at
//! least attributable and reviewable. Its narrow use stays available as an
//! escape hatch: only the Windows leg is genuinely `unsafe`, so if that leg
//! proves worse than expected, vendoring *that* leg beats replacing the
//! transport.
//!
//! # Two failures, two remedies
//!
//! The whole point of parsing the reply rather than collapsing it into a
//! boolean is that [`AgentError::AttachFailed`] and [`AgentError::AgentRefused`]
//! need different things from the operator: the first is something about the
//! connection from PlatynUI's side, the second is a decision inside the target
//! JVM — a sandboxed-JNLP `SecurityManager`, or dynamic agent loading
//! disallowed.

use crate::error::AgentError;
use crate::jvm;
use std::path::Path;
use std::time::Duration;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// How long to wait for the target JVM to answer an attach request.
///
/// The JVM has to start its attach listener first, which on a busy or
/// large-heap application is not instant.
pub const DEFAULT_ATTACH_TIMEOUT: Duration = Duration::from_secs(10);

/// The attach command that loads a Java agent JAR.
///
/// A Java agent is loaded through the JVMTI agent library named `instrument`,
/// which is exactly what `VirtualMachine.loadAgent` does under the hood — the
/// JAR path travels as that library's option string.
const LOAD_COMMAND: &str = "load";
const INSTRUMENT_LIBRARY: &str = "instrument";

/// Loads the PlatynUI agent JAR into the running JVM `pid`.
///
/// `options` is the agent argument string, or `None`.
///
/// # Errors
///
/// [`AgentError::NotAJvm`] if the process is known not to run a JVM,
/// [`AgentError::AttachFailed`] if the attach never reached the target, and
/// [`AgentError::AgentRefused`] if the target rejected the agent.
pub fn load_agent(pid: u32, jar: &Path, options: Option<&str>, timeout: Duration) -> Result<(), AgentError> {
    if !jar.is_file() {
        return Err(AgentError::JarUnavailable {
            details: format!("no such file: {}", jar.display()),
            path: Some(jar.to_path_buf()),
        });
    }
    if !jvm::process_is_alive(pid) {
        return Err(AgentError::ProcessUnavailable { pid, details: "the process is not running".to_owned() });
    }
    // `None` means the probe could not answer (an elevated target, or a
    // platform without a free signal) — that must not block an attach. Only a
    // definite "no JVM here" does.
    if jvm::process_runs_jvm(pid) == Some(false) {
        return Err(AgentError::NotAJvm { pid });
    }

    // The JVM wants an absolute path: it resolves the agent JAR in the target's
    // working directory, which is not ours.
    let jar = jar.canonicalize().map_err(|e| AgentError::JarUnavailable {
        details: format!("cannot resolve {}: {e}", jar.display()),
        path: Some(jar.to_path_buf()),
    })?;
    let jar_path = strip_extended_prefix(&jar.to_string_lossy());
    let argument = match options {
        Some(options) if !options.is_empty() => format!("{jar_path}={options}"),
        _ => jar_path,
    };

    tracing::debug!(pid, jar = %jar.display(), "attaching the PlatynUI agent");
    let reply = execute(pid, LOAD_COMMAND, &[INSTRUMENT_LIBRARY, "false", &argument], timeout)?;
    interpret_load_reply(pid, &reply)
}

/// Windows canonicalisation yields a `\\?\` extended-length path, which the JVM
/// does not accept as an agent path.
#[cfg(windows)]
fn strip_extended_prefix(path: &str) -> String {
    path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
}

#[cfg(not(windows))]
fn strip_extended_prefix(path: &str) -> String {
    path.to_owned()
}

/// Runs one attach command and returns the JVM's raw reply.
fn execute(pid: u32, command: &str, args: &[&str], timeout: Duration) -> Result<String, AgentError> {
    #[cfg(unix)]
    {
        unix::execute(pid, command, args, timeout)
    }
    #[cfg(windows)]
    {
        windows::execute(pid, command, args, timeout)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (command, args, timeout);
        Err(AgentError::AttachFailed { pid, details: "no attach transport on this platform".to_owned() })
    }
}

/// The prefix newer JDKs put in front of `Agent_OnAttach`'s return value.
///
/// Java 8 answers a bare integer on the second line; JDK 21 answers
/// `return code: 0`. Both dialects are live in the field — enterprise Swing on
/// 8, everything modern on 17+ — so both are parsed rather than one being
/// declared canonical.
const RETURN_CODE_PREFIX: &str = "return code:";

/// Turns the JVM's reply to a `load` into a verdict.
///
/// The first line is always the attach-operation status. What follows is either
/// the agent's result (in one of the two dialects above) or, when the VM
/// declined to load the agent at all, its own message.
fn interpret_load_reply(pid: u32, reply: &str) -> Result<(), AgentError> {
    let mut lines = reply.lines();
    let status_line = lines.next().unwrap_or("").trim();
    let status: i32 = status_line.parse().map_err(|_| AgentError::AttachFailed {
        pid,
        details: format!("the target did not answer the attach protocol (got {reply:?})"),
    })?;
    let body = lines.collect::<Vec<_>>().join("\n").trim().to_owned();

    if status != 0 {
        return Err(refusal_or_attach_failure(
            pid,
            if body.is_empty() { format!("attach operation failed with status {status}") } else { body },
        ));
    }
    if body.is_empty() {
        return Err(AgentError::Protocol {
            details: format!("the target accepted the attach but returned no agent result (got {reply:?})"),
        });
    }

    let numeric = body.strip_prefix(RETURN_CODE_PREFIX).unwrap_or(&body).trim().parse::<i32>();
    match numeric {
        Ok(0) => Ok(()),
        Ok(code) => {
            Err(AgentError::AgentRefused { pid, details: format!("the agent's Agent_OnAttach returned {code}") })
        }
        // Not a result at all: the VM refused to load the agent and said why.
        // This is the path a JEP 451 refusal takes on a modern JDK.
        Err(_) => Err(refusal_or_attach_failure(pid, body)),
    }
}

/// Classifies a message from the target.
///
/// A JEP 451 refusal reaches us as a load failure, but it is a decision
/// *inside* the target with a remedy there (`-XX:+EnableDynamicAgentLoading`,
/// settable per environment through `JAVA_TOOL_OPTIONS`). Reporting it as a
/// plain attach failure would send the operator looking on the wrong side.
fn refusal_or_attach_failure(pid: u32, message: String) -> AgentError {
    const REFUSALS: [&str; 3] =
        ["Dynamic agent loading is not enabled", "Failed to load agent library", "agent library failed to init"];
    if REFUSALS.iter().any(|marker| message.contains(marker)) {
        AgentError::AgentRefused { pid, details: message }
    } else {
        AgentError::AttachFailed { pid, details: message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both wire dialects, measured against real JVMs: Java 8 answers a bare
    // integer, JDK 21 answers "return code: 0".
    #[test]
    fn a_clean_load_succeeds_in_both_dialects() {
        assert!(interpret_load_reply(1, "0\n0\n").is_ok(), "Java 8 dialect");
        assert!(interpret_load_reply(1, "0\nreturn code: 0\n").is_ok(), "JDK 17+ dialect");
    }

    #[test]
    fn a_nonzero_return_code_is_reported_as_the_target_refusing() {
        let error = interpret_load_reply(5, "0\nreturn code: 1\n").expect_err("must fail");
        assert!(matches!(error, AgentError::AgentRefused { pid: 5, .. }), "got {error:?}");
    }

    #[test]
    fn a_failed_attach_operation_carries_the_targets_message() {
        let error = interpret_load_reply(1, "101\nAttach agent not supported\n").expect_err("must fail");
        match error {
            AgentError::AttachFailed { pid, details } => {
                assert_eq!(pid, 1);
                assert_eq!(details, "Attach agent not supported");
            }
            other => panic!("expected AttachFailed, got {other:?}"),
        }
    }

    // JEP 451: the sunset path. The JVM reports it as an attach-operation
    // failure, but the remedy is inside the target, so it must not read as a
    // PlatynUI-side attach problem.
    #[test]
    fn a_jep451_refusal_is_reported_as_the_target_refusing() {
        let reply = "1\nFailed to load agent library: Dynamic agent loading is not enabled. \
                     Use -XX:+EnableDynamicAgentLoading to launch target VM.\n";
        let error = interpret_load_reply(7, reply).expect_err("must fail");
        match error {
            AgentError::AgentRefused { pid, details } => {
                assert_eq!(pid, 7);
                assert!(details.contains("EnableDynamicAgentLoading"), "the remedy must survive: {details}");
            }
            other => panic!("expected AgentRefused, got {other:?}"),
        }
    }

    // A SecurityManager or a broken JAR: the attach worked, the agent did not.
    #[test]
    fn a_failed_agent_load_is_reported_as_the_target_refusing() {
        let error = interpret_load_reply(3, "0\nFailed to load agent library: something\n").expect_err("must fail");
        assert!(matches!(error, AgentError::AgentRefused { pid: 3, .. }), "got {error:?}");
    }

    #[test]
    fn a_non_protocol_answer_is_not_mistaken_for_success() {
        assert!(matches!(interpret_load_reply(1, ""), Err(AgentError::AttachFailed { .. })));
        assert!(matches!(interpret_load_reply(1, "hello\n"), Err(AgentError::AttachFailed { .. })));
        // Status parsed, but the agent result never arrived.
        assert!(matches!(interpret_load_reply(1, "0\n"), Err(AgentError::Protocol { .. })));
        // An unrecognised message must not read as a clean load either.
        assert!(matches!(interpret_load_reply(1, "0\nsomething unexpected\n"), Err(AgentError::AttachFailed { .. })));
    }
}
