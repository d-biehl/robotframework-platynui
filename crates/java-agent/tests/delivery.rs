//! Delivery checks: does installing `platynui-provider-java` actually make the
//! agent findable, and does *not* installing it produce something a user can
//! act on?
//!
//! This builds a throwaway virtual environment and installs the real wheel into
//! it, because the thing under test is the packaging — an entry point that
//! resolves in the source tree but not from an installed wheel would pass any
//! cheaper test and fail every user.
//!
//! `#[ignore]`d: needs `uv` and a network-free wheel build. The lane runs them
//! explicitly:
//!
//! ```text
//! just test-provider-java-delivery
//! ```

#![allow(clippy::doc_markdown)]

use platynui_java_agent::discovery::{self, AgentPackage};
use std::path::{Path, PathBuf};
use std::process::Command;

// Dependencies of the library that this test target does not use directly.
#[cfg(unix)]
use rustix as _;
use serde as _;
use serde_json as _;
use tempfile as _;
use thiserror as _;
use tracing as _;
#[cfg(windows)]
use windows as _;

const EXPECTED_VERSION: &str = env!("CARGO_PKG_VERSION");

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn interpreter_in(venv: &Path) -> PathBuf {
    if cfg!(windows) { venv.join("Scripts").join("python.exe") } else { venv.join("bin").join("python") }
}

fn script_in(venv: &Path, name: &str) -> PathBuf {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() };
    if cfg!(windows) { venv.join("Scripts").join(file) } else { venv.join("bin").join(file) }
}

fn run(command: &mut Command, what: &str) -> String {
    let output = command.output().unwrap_or_else(|e| panic!("{what} could not start: {e}"));
    assert!(
        output.status.success(),
        "{what} failed ({}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Creates an empty virtual environment.
fn new_venv(at: &Path) {
    run(Command::new("uv").arg("venv").arg(at).current_dir(repo_root()), "uv venv");
}

/// Builds the wheel exactly as the release recipe does, and returns its path.
fn build_wheel(into: &Path) -> PathBuf {
    // The JAR has to be staged first — a wheel without it installs as a working
    // package that silently provides no Java support, which is the one failure
    // mode worth building a whole environment to rule out.
    let jar = repo_root().join("java").join("agent").join("build").join("libs").join("platynui-agent.jar");
    assert!(jar.is_file(), "agent JAR not found at {} — run `just build-java-agent` first", jar.display());
    let staged = repo_root()
        .join("packages")
        .join("provider-java")
        .join("src")
        .join("platynui_provider_java")
        .join("agent")
        .join("platynui-agent.jar");
    std::fs::create_dir_all(staged.parent().expect("agent dir")).expect("create the staging directory");
    std::fs::copy(&jar, &staged).expect("stage the agent JAR");

    run(
        Command::new("uv")
            .arg("build")
            .arg("--wheel")
            .arg(repo_root().join("packages").join("provider-java"))
            .arg("-o")
            .arg(into)
            .current_dir(repo_root()),
        "uv build",
    );
    std::fs::read_dir(into)
        .expect("read the wheel output directory")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "whl"))
        .expect("a wheel must have been built")
}

/// With the package installed, discovery finds it through the real transport —
/// the environment's own interpreter, no configuration, no embedded Python.
#[test]
#[ignore = "needs uv and builds a wheel"]
fn the_installed_package_is_discovered_through_the_environment_interpreter() {
    let workspace = tempfile::tempdir().expect("temp dir");
    let wheel = build_wheel(&workspace.path().join("dist"));
    let venv = workspace.path().join("venv");
    new_venv(&venv);
    run(
        Command::new("uv").arg("pip").arg("install").arg("--python").arg(interpreter_in(&venv)).arg(&wheel),
        "uv pip install",
    );

    let package: AgentPackage =
        discovery::query_interpreter(&interpreter_in(&venv)).expect("the installed package must be discovered");
    assert_eq!(package.version, EXPECTED_VERSION, "the wheel must carry this build's version");
    assert!(package.agent_jar.is_file(), "the reported JAR must exist at {}", package.agent_jar.display());
    assert!(
        package.agent_jar.starts_with(&venv),
        "discovery must find the JAR of THAT environment, not one lying around: {}",
        package.agent_jar.display()
    );

    // The operator-facing command must agree with what discovery resolved —
    // it is what a user pastes into a `-javaagent:` line when attach is blocked.
    let printed = run(Command::new(script_in(&venv, "platynui-provider-java")).arg("agent-path"), "agent-path");
    assert_eq!(Path::new(printed.trim()), package.agent_jar);
}

/// Without the package there is simply nothing to find — which is what makes
/// the install diagnostic the right answer rather than a crash.
#[test]
#[ignore = "needs uv"]
fn an_environment_without_the_package_reports_nothing() {
    let workspace = tempfile::tempdir().expect("temp dir");
    let venv = workspace.path().join("venv");
    new_venv(&venv);
    assert!(
        discovery::query_interpreter(&interpreter_in(&venv)).is_none(),
        "a bare environment must not resolve a Java provider"
    );
}
