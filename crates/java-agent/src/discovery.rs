//! Finding the agent JAR — the artifact that has to exist before anything can
//! be injected.
//!
//! **Opt-in by installation** (design decision 9): the JAR ships in its own
//! package, `platynui-provider-java`, pulled in by the `[java]` extras. Nobody
//! gets in-JVM instrumentation without having asked for Java support, and
//! uninstalling removes the capability. When the package is absent the answer
//! is not a crash and not silence — it is a diagnostic naming the install.
//!
//! # One concept, two transports
//!
//! Discovery is always the `platynui.providers` entry-point group. How it is
//! read depends on where the runtime lives:
//!
//! - **In-process** — the provider runs inside the `platynui_native` extension
//!   module, so it already has an interpreter and the GIL. It installs a
//!   resolver through [`set_in_process_resolver`]; that is the normal test-run
//!   path and costs nothing.
//! - **Co-located interpreter** — the Inspector and CLI are maturin
//!   `bindings = "bin"` console scripts with **no Python wrapper process**, so
//!   nothing hands them an environment. They derive it from their own
//!   executable path and ask it once. Embedding libpython was rejected: maturin
//!   ignores `abi3` for binary targets and builds one wheel per Python version,
//!   because a binary links a concrete libpython at build time — those wheels
//!   are Python-version-independent today and would stop being so.
//!
//! # Quiescence
//!
//! Nothing here runs on its own. The first call resolves and caches; a session
//! with agent support inactive never makes that call, so no interpreter is
//! spawned and no package metadata is read.

use crate::client::CLIENT_VERSION;
use crate::error::AgentError;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Config key that decides whether agent support is usable at all.
///
/// Named here so the consuming provider and this crate cannot drift on the
/// spelling. Reading it is the provider's job — this crate stays free of
/// PlatynUI dependencies.
pub const ENABLED_KEY: &str = "providers.java.agent.enabled";

/// Config key that overrides discovery with an explicit JAR path.
pub const JAR_KEY: &str = "providers.java.agent.jar";

/// Environment override, and the `cargo run` development leg.
pub const JAR_ENV: &str = "PLATYNUI_JAVA_AGENT_JAR";

/// The entry-point group a provider package registers itself in.
pub const ENTRY_POINT_GROUP: &str = "platynui.providers";

/// The entry-point name of the Java provider package.
pub const ENTRY_POINT_NAME: &str = "java";

/// What the installed delivery package reports about itself.
#[derive(Debug, Clone)]
pub struct AgentPackage {
    /// Absolute path of the bundled agent JAR.
    pub agent_jar: PathBuf,
    /// The package's version, which must equal this client's exactly.
    pub version: String,
}

type InProcessResolver = fn() -> Option<AgentPackage>;

static IN_PROCESS_RESOLVER: Mutex<Option<InProcessResolver>> = Mutex::new(None);
static CACHED_PACKAGE: OnceLock<Option<AgentPackage>> = OnceLock::new();

/// Installs the in-process entry-point resolver.
///
/// Called by the Python extension module, which already holds an interpreter —
/// it can ask `importlib.metadata` directly instead of spawning one.
///
/// # Panics
///
/// If the resolver registry was poisoned by a panic in another thread.
pub fn set_in_process_resolver(resolver: InProcessResolver) {
    *IN_PROCESS_RESOLVER.lock().expect("in-process resolver mutex poisoned") = Some(resolver);
}

/// Resolves the agent JAR.
///
/// Order: explicit configuration ([`JAR_KEY`]) → environment ([`JAR_ENV`]) →
/// the installed package → an actionable diagnostic. Explicit settings win so
/// an operator can point at a JAR PlatynUI did not ship — for a bisect, or a
/// locally built agent.
///
/// # Errors
///
/// [`AgentError::JarUnavailable`] when nothing resolves, when a configured path
/// does not exist, or when the installed package's version differs from this
/// client's — an agent cannot be unloaded, so injecting a mismatched one only
/// moves the failure into the middle of a test.
pub fn resolve_agent_jar(configured: Option<&Path>) -> Result<PathBuf, AgentError> {
    if let Some(path) = configured {
        return require_file(path, JAR_KEY);
    }
    if let Some(path) = std::env::var_os(JAR_ENV).filter(|value| !value.is_empty()) {
        return require_file(Path::new(&path), JAR_ENV);
    }

    let package = installed_package().ok_or_else(missing_package_error)?;

    if package.version != CLIENT_VERSION {
        return Err(AgentError::JarUnavailable {
            details: format!(
                "the installed platynui-provider-java is version {} but this PlatynUI is {CLIENT_VERSION} — \
                 they must match exactly; reinstall the `[java]` extra so the resolver pins them together",
                package.version
            ),
            path: Some(package.agent_jar.clone()),
        });
    }
    require_file(&package.agent_jar, "the installed platynui-provider-java package")
}

/// The diagnostic for "no Java agent support here".
///
/// It has one job: name the remedy. "Java support unavailable" without the
/// install line is the difference between a user who fixes it in ten seconds
/// and one who files a bug.
fn missing_package_error() -> AgentError {
    AgentError::JarUnavailable {
        details: format!(
            "Java agent support needs the `platynui-provider-java` package — \
             install `robotframework-platynui[java]` (or `platynui-inspector[java]` for the Inspector), \
             or point `{JAR_KEY}` / `{JAR_ENV}` at a JAR"
        ),
        path: None,
    }
}

fn require_file(path: &Path, source: &str) -> Result<PathBuf, AgentError> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    Err(AgentError::JarUnavailable {
        details: format!("{source} points at {}, which is not a file", path.display()),
        path: Some(path.to_path_buf()),
    })
}

/// The installed delivery package, resolved once per process.
///
/// Caching is not only for speed: the co-located transport spawns an
/// interpreter, and doing that per window enumeration would be absurd.
#[must_use]
pub fn installed_package() -> Option<AgentPackage> {
    CACHED_PACKAGE.get_or_init(discover_package).clone()
}

fn discover_package() -> Option<AgentPackage> {
    let resolver = *IN_PROCESS_RESOLVER.lock().expect("in-process resolver mutex poisoned");
    if let Some(resolver) = resolver
        && let Some(package) = resolver()
    {
        tracing::debug!(jar = %package.agent_jar.display(), "resolved the agent package in process");
        return Some(package);
    }
    let package = query_colocated_interpreter();
    if let Some(package) = &package {
        tracing::debug!(jar = %package.agent_jar.display(), "resolved the agent package via the environment interpreter");
    }
    package
}

// ------------------------------------------------- co-located interpreter

/// The one-shot query. Kept deliberately small and total: a broken third-party
/// entry point in the same group must not stop us finding ours.
const QUERY_SNIPPET: &str = r#"
import json, sys
try:
    from importlib.metadata import entry_points
    found = {}
    for ep in entry_points(group="platynui.providers"):
        try:
            found[ep.name] = ep.load()()
        except Exception:
            pass
    print(json.dumps(found))
except Exception:
    print("{}")
"#;

fn query_colocated_interpreter() -> Option<AgentPackage> {
    query_interpreter(&environment_interpreter()?)
}

/// Keeps the interpreter's console off the screen.
///
/// `python.exe` is a console-subsystem program, and Windows gives such a child a
/// **fresh console window** when the parent has none — which is exactly the
/// Inspector's situation. The output is captured either way, so the window is
/// pure visual noise: a black box that flashes over the application the user is
/// inspecting, once per process. `CREATE_NO_WINDOW` suppresses it without
/// changing anything about the query.
///
/// Not solved by using `pythonw.exe`: that name only exists for CPython on
/// Windows, and the interpreter here is whatever the environment happens to
/// provide.
#[cfg(windows)]
fn hide_console(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    /// `CREATE_NO_WINDOW` from `processthreadsapi.h` — spelled out rather than
    /// pulled from a Windows binding, because this crate deliberately depends on
    /// nothing PlatynUI and carries only the bindings its attach transport needs.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut std::process::Command) {
    // No console is ever allocated for a child on Unix.
}

/// Asks one specific interpreter what it has registered in the group.
///
/// Public because "which environment?" is a legitimate question for a caller to
/// answer itself — and because it makes the transport testable against a real
/// environment without reaching through process-wide state.
#[must_use]
pub fn query_interpreter(interpreter: &Path) -> Option<AgentPackage> {
    let mut command = std::process::Command::new(interpreter);
    command.arg("-c").arg(QUERY_SNIPPET);
    hide_console(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() {
        tracing::debug!(interpreter = %interpreter.display(), "the environment interpreter refused the query");
        return None;
    }
    let manifest: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let java = manifest.get(ENTRY_POINT_NAME)?;
    Some(AgentPackage {
        agent_jar: PathBuf::from(java.get("agent_jar")?.as_str()?),
        version: java.get("version")?.as_str()?.to_owned(),
    })
}

/// The interpreter of the environment this binary was installed into.
///
/// A console script sits next to its interpreter — `Scripts\` on Windows,
/// `bin/` elsewhere — and a real environment root carries a `pyvenv.cfg`. That
/// marker is what distinguishes "installed into an environment" from "a binary
/// that happens to live next to some python", which is why an interpreter
/// without it is only accepted through the explicit `VIRTUAL_ENV`.
fn environment_interpreter() -> Option<PathBuf> {
    if let Some(executable) = std::env::current_exe().ok().and_then(|exe| {
        let directory = exe.parent()?.to_path_buf();
        interpreter_in(&directory).filter(|_| directory.parent().is_some_and(is_environment_root))
    }) {
        return Some(executable);
    }
    let virtual_env = std::env::var_os("VIRTUAL_ENV").filter(|value| !value.is_empty())?;
    interpreter_in(&PathBuf::from(virtual_env).join(SCRIPT_DIR))
}

#[cfg(windows)]
const SCRIPT_DIR: &str = "Scripts";
#[cfg(not(windows))]
const SCRIPT_DIR: &str = "bin";

#[cfg(windows)]
const INTERPRETER_NAMES: [&str; 1] = ["python.exe"];
#[cfg(not(windows))]
const INTERPRETER_NAMES: [&str; 2] = ["python3", "python"];

fn interpreter_in(directory: &Path) -> Option<PathBuf> {
    INTERPRETER_NAMES.iter().map(|name| directory.join(name)).find(|candidate| candidate.is_file())
}

fn is_environment_root(directory: &Path) -> bool {
    directory.join("pyvenv.cfg").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_path_wins_and_must_exist() {
        let dir = tempfile::tempdir().expect("temp dir");
        let jar = dir.path().join("platynui-agent.jar");
        std::fs::write(&jar, b"not really a jar").expect("write");
        assert_eq!(resolve_agent_jar(Some(&jar)).expect("resolves"), jar);

        let missing = dir.path().join("nope.jar");
        let error = resolve_agent_jar(Some(&missing)).expect_err("must fail");
        match error {
            AgentError::JarUnavailable { details, path } => {
                assert!(details.contains(JAR_KEY), "the diagnostic must name the setting: {details}");
                assert_eq!(path.as_deref(), Some(missing.as_path()));
            }
            other => panic!("expected JarUnavailable, got {other:?}"),
        }
    }

    /// The diagnostic is the whole value of the "no package" path: it must name
    /// the install, both extras, and the two overrides.
    #[test]
    fn the_missing_package_diagnostic_names_the_remedy() {
        let AgentError::JarUnavailable { details, path } = missing_package_error() else {
            panic!("the missing-package case must be a JarUnavailable");
        };
        assert!(path.is_none(), "there is no path to report when nothing is installed");
        for expected in
            ["platynui-provider-java", "robotframework-platynui[java]", "platynui-inspector[java]", JAR_KEY, JAR_ENV]
        {
            assert!(details.contains(expected), "the diagnostic must mention {expected}: {details}");
        }
    }

    #[test]
    fn the_config_keys_are_spelled_once() {
        // The provider reads these; a silent rename here would turn a
        // configured setting into an ignored one.
        assert_eq!(ENABLED_KEY, "providers.java.agent.enabled");
        assert_eq!(JAR_KEY, "providers.java.agent.jar");
        assert_eq!(JAR_ENV, "PLATYNUI_JAVA_AGENT_JAR");
        assert_eq!(ENTRY_POINT_GROUP, "platynui.providers");
    }

    #[test]
    fn an_interpreter_is_only_accepted_from_a_real_environment_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let scripts = dir.path().join(SCRIPT_DIR);
        std::fs::create_dir_all(&scripts).expect("scripts dir");
        std::fs::write(scripts.join(INTERPRETER_NAMES[0]), b"").expect("fake interpreter");

        assert!(interpreter_in(&scripts).is_some(), "the interpreter must be found by name");
        assert!(!is_environment_root(dir.path()), "without pyvenv.cfg this is not an environment");

        std::fs::write(dir.path().join("pyvenv.cfg"), b"home = /usr").expect("marker");
        assert!(is_environment_root(dir.path()));
    }
}
