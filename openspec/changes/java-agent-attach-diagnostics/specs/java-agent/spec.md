## MODIFIED Requirements

### Requirement: Delivery as an opt-in package
The agent artifact SHALL be delivered in a separate installable package, discovered through the `platynui.providers` entry-point group. Discovery SHALL ask a Python interpreter of the environment the runtime belongs to: where the runtime runs inside Python, the interpreter it runs in, which the Python extension names once, when it is imported; for a standalone binary, the co-located environment interpreter. A host whose executable is not a Python interpreter — a frozen application, or an application that embeds Python — SHALL NOT be named, and discovery then infers the environment as a standalone binary does. Discovery SHALL NOT call into the Python interpreter of the process that hosts the runtime, so that it cannot deadlock a runtime call, and naming the interpreter SHALL neither read package metadata nor import any package. An explicit configuration setting, and after it the `PLATYNUI_JAVA_AGENT_JAR` environment variable, SHALL override discovery; a path that either of them names and that is not a file SHALL be reported as such and SHALL NOT be replaced by a discovered artifact.

Installing that package SHALL be the consent for in-JVM instrumentation: when it is absent, Java agent support SHALL be reported unavailable with an actionable diagnostic that names the install, and nothing SHALL be injected. Every other reason why the artifact cannot be resolved — an explicit path that is not a file, an installed package that is broken or of another version, an environment interpreter that cannot be run or does not give a discovery answer — SHALL be reported with its own cause and remedy, carrying the package's or the interpreter's own message where there is one, and SHALL NOT name installing the package as the remedy. (Whether a detected JVM is then attached *automatically*, and how loudly an unresolved artifact is reported, is the consuming provider's policy — see `java-provider`.)

#### Scenario: Missing package yields an actionable diagnostic
- **GIVEN** no agent package installed, and neither the setting nor `PLATYNUI_JAVA_AGENT_JAR` given
- **WHEN** a Java application is encountered
- **THEN** nothing is injected, the application is served as it would be without Java agent support, and the diagnostic names the install as the remedy

#### Scenario: The agent artifact is found from a standalone binary
- **GIVEN** a standalone binary installed in a virtual environment that has the agent package, and no explicit configuration
- **WHEN** the binary resolves the agent artifact
- **THEN** it finds the artifact belonging to that environment, without embedding a Python interpreter

#### Scenario: A test run finds the artifact of the interpreter it runs in
- **GIVEN** the agent package installed in the environment of the Python interpreter that imported the extension, `VIRTUAL_ENV` unset, and neither the setting nor `PLATYNUI_JAVA_AGENT_JAR` given
- **WHEN** the agent artifact is resolved
- **THEN** the artifact of that interpreter's environment is found, exactly as it would be with the environment activated
- **NOTE** Verified in a subprocess with a stand-in package on any operating system. A virtual environment's interpreter started directly on Windows, and a conda environment where one is available, are checked by hand.

#### Scenario: Importing the extension resolves nothing
- **GIVEN** a stand-in agent package whose import leaves a trace in a file, first on the module search path
- **WHEN** a Python process imports the extension and creates a runtime
- **THEN** the interpreter named for discovery is the process's `sys.executable`
- **AND** the stand-in package has been imported neither in that process nor in an interpreter started for discovery
- **NOTE** Verified with the mock build, whose runtime never needs the artifact; on Windows the same holds for a real runtime until a Java window without an agent is met (see *Quiescence when inactive*).

#### Scenario: A host that is not a Python interpreter is not taken for one
- **GIVEN** a process that marks itself as a frozen application (`sys.frozen`), or whose `sys.executable` names a program that is not a Python interpreter, before it imports the extension
- **WHEN** the import completes
- **THEN** no interpreter is named for discovery, because starting that program would start the application instead of asking an interpreter
- **NOTE** Verified in subprocesses that set `sys.frozen`, or assign `sys.executable`, before the import.

#### Scenario: A broken installation reports its own cause
- **GIVEN** the agent package installed, but its entry point raises because the artifact is missing from the installation
- **WHEN** the agent artifact is resolved
- **THEN** the diagnostic carries the entry point's own exception type and message, including its remedy, reinstalling `platynui-provider-java`
- **AND** it does not name installing `robotframework-platynui[java]` as the remedy
- **NOTE** Verified with a stand-in package, and with the real wheel whose artifact was deleted after installation (the delivery check).

#### Scenario: An installed package of another version is named
- **GIVEN** an installed agent package that reports another version than this PlatynUI
- **WHEN** the agent artifact is resolved
- **THEN** the diagnostic names both versions and reinstalling the extra as the remedy
- **AND** it does not name installing the package as if it were absent
- **NOTE** Unit test of the resolution, and a stand-in package in a subprocess.

#### Scenario: An explicit path that is not a file is named, not replaced
- **GIVEN** `providers.java.agent.jar`, or else `PLATYNUI_JAVA_AGENT_JAR`, naming a path that is not a file, and an installed agent package
- **WHEN** the agent artifact is resolved
- **THEN** the diagnostic names the setting or the variable, and the path
- **AND** the installed package's artifact is not used in its place
- **NOTE** Unit test of the resolution.

#### Scenario: An interpreter that cannot be asked is not a missing package
- **GIVEN** an environment interpreter that exits with an error, or answers with something that is not a discovery answer
- **WHEN** the agent artifact is resolved
- **THEN** the diagnostic says that the environment could not be asked, carries the interpreter's own message where it gave one, and names the explicit setting as the way around it
- **AND** it does not name installing the package as the remedy
- **NOTE** Unit test of how the interpreter's answer is read.
