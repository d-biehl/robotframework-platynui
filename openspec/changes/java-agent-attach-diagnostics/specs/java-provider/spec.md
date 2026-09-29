## ADDED Requirements

### Requirement: Automatic attachment that cannot happen says why
When automatic attachment is on and the Java provider cannot inject the agent into a JVM, the provider SHALL report why, at the level the cause deserves. Every such report SHALL state its consequence without presuming that another backend serves the application, because on some platforms none does: the application is *not served through the agent*.

- **A configuration or installation mistake.** An agent artifact that cannot be resolved because of a mistake — an explicit path that is not a file, an installed package that is broken or of another version, an environment interpreter that cannot be asked — SHALL be reported as a warning once per runtime for each distinct mistake, naming the cause and its remedy. While the mistake persists, each further occurrence SHALL be recorded at debug level. When the artifact resolves again, that SHALL be recorded once, at debug level.
- **The absent package.** An absent agent package is the consented default. It SHALL be recorded once per runtime at debug level, naming the install, and SHALL NOT produce a warning.
- **Attempts.** An artifact that cannot be resolved SHALL NOT use up a JVM's attach attempts: once it resolves, every JVM seen meanwhile still has all of its attempts.
- **A refusing JVM.** A JVM that refuses the agent — dynamic agent loading disallowed (JEP 451), or the agent library declined by the JVM — SHALL be reported as a warning the first time it refuses in a runtime. The warning SHALL name the process and carry the JVM's own message. It SHALL name the remedy inside the target — `-XX:+EnableDynamicAgentLoading`, which `JAVA_TOOL_OPTIONS` can carry, or `-javaagent` at launch — and `providers.java.agent.auto_attach = false` as the way to stop the attempts. Further refusals of the same process SHALL be recorded at debug level.
- **Other attach failures.** An attach that does not reach the target, a process that is not a JVM, and a process that is gone SHALL be recorded at debug level only.

Nothing SHALL be resolved or reported while automatic attachment is off or the agent backend is disabled.

#### Scenario: An explicit JAR path that is not a file warns once
- **GIVEN** automatic attachment on, `providers.java.agent.jar` naming a path that is not a file, and a Java window whose JVM carries no agent
- **WHEN** the desktop is enumerated three times
- **THEN** exactly one warning SHALL name the setting and the path, and say that the application is not served through the agent
- **AND** the second and third enumerations SHALL add debug records only
- **AND** no attach SHALL be attempted, and no attempt counted against the JVM
- **NOTE** Unit test with log capture, on a Windows host like the whole Java provider today.

#### Scenario: Each runtime says it once
- **GIVEN** a process whose first runtime has already warned about an explicit JAR path that is not a file
- **WHEN** a second runtime with the same setting meets a JVM without an agent, three times
- **THEN** the second runtime SHALL warn exactly once as well, so that its own log — a Robot Framework suite's, for example — says why its Java applications are not served through the agent
- **AND** within one runtime, a second, different mistake SHALL be warned about once on its own
- **NOTE** Unit test with two backends, on a Windows host.

#### Scenario: A broken installation warns once, with its own cause
- **GIVEN** automatic attachment on, and an installed agent package whose entry point fails
- **WHEN** the desktop is enumerated repeatedly while a JVM without an agent is present
- **THEN** exactly one warning SHALL carry the entry point's own message and remedy
- **AND** no record SHALL recommend installing the package
- **NOTE** Unit test with the resolution's outcome injected, on a Windows host; the message itself is verified under `java-agent`.

#### Scenario: An absent package produces no warning
- **GIVEN** automatic attachment on, no agent package, and neither the setting nor `PLATYNUI_JAVA_AGENT_JAR` given
- **WHEN** the desktop is enumerated repeatedly while a JVM without an agent is present
- **THEN** no warning about the agent SHALL be logged
- **AND** exactly one debug record SHALL name the install
- **NOTE** Unit test on a Windows host.

#### Scenario: A JAR that resolves later still gets every attempt
- **GIVEN** an explicit JAR path that is not a file, and a JVM without an agent seen by more enumerations than a JVM has attach attempts
- **WHEN** the file appears and the desktop is enumerated again
- **THEN** an attach SHALL be attempted for that JVM
- **AND** one debug record SHALL say that the artifact resolves again
- **NOTE** Unit test on a Windows host.

#### Scenario: A JVM that refuses the agent is named once, with the remedy
- **GIVEN** automatic attachment on, a resolvable JAR, and a JVM without an agent started with `-XX:-EnableDynamicAgentLoading`
- **WHEN** the desktop is enumerated until that JVM's attach attempts are spent
- **THEN** exactly one warning SHALL name the process, carry the JVM's refusal, and name `-XX:+EnableDynamicAgentLoading`, `JAVA_TOOL_OPTIONS`, `-javaagent` and `providers.java.agent.auto_attach`
- **AND** the further refusals of that process SHALL be recorded at debug level
- **AND** the warning SHALL NOT say that the Access Bridge or any other backend serves the application
- **NOTE** The refusal itself needs a real JVM of JDK 21 or later: `java-agent`'s live check proves it on Linux and Windows. The level is proven by a unit test with the attach outcome injected, on a Windows host; the whole path is checked by hand on Windows.

#### Scenario: An attach that fails for other reasons stays at debug
- **GIVEN** automatic attachment on, a resolvable JAR, and a candidate process that is not a JVM, or that is gone
- **WHEN** the desktop is enumerated
- **THEN** no warning SHALL be logged, and the failure SHALL be recorded at debug level
- **NOTE** Unit test on a Windows host, against the test's own process, which is not a JVM, and a process ID that no process has; no attach reaches any process.

#### Scenario: Nothing is reported while automatic attachment is off
- **GIVEN** `providers.java.agent.auto_attach` set to `false`, `providers.java.agent.jar` naming a path that is not a file, and a JVM without an agent
- **WHEN** the desktop is enumerated
- **THEN** the path SHALL NOT be checked, and no record about the artifact SHALL be logged
- **NOTE** Unit test on a Windows host.
