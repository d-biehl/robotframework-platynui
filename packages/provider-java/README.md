# platynui-provider-java

Delivery package for PlatynUI's **in-JVM Java agent** — the artifact PlatynUI loads into a target
JVM to read Swing, JavaFX and SWT models from the inside, past what native accessibility exposes.

It carries exactly one thing: `platynui-agent.jar`. No automation logic, no dependencies.

## Installing it is the consent

Java agent support is opt-in **by installation**. Without this package PlatynUI reports Java agent
support as unavailable and instruments nothing; uninstalling removes the capability. That is why it
is a separate package rather than a flag — an environment either has the artifact or it does not,
and that decision is visible in its own dependency list.

```sh
pip install "robotframework-PlatynUI[java]"     # for a test run
pip install "platynui-inspector[java]"          # for the Inspector
```

Both extras pin this package to the exact matching version. That is not pedantry: an agent cannot
be unloaded from a JVM, so a version mismatch has exactly one remedy — restart the application —
and the connection is refused rather than degraded.

The JAR is platform- and Python-version-neutral, so this ships as a `py3-none-any` wheel. It is not
duplicated into the per-platform native wheels, and building those needs no JDK.

## Finding the JAR

PlatynUI finds it by itself, through the `platynui.providers` entry point this package registers —
in-process when the runtime already runs inside Python, and through the environment's own
interpreter when a standalone binary (Inspector, CLI) asks.

The command line exists for the one case PlatynUI cannot help with: a target JVM where dynamic
attach is blocked, so the agent has to go on the launch command line by hand.

```sh
platynui-provider-java agent-path        # → …/platynui_provider_java/agent/platynui-agent.jar
platynui-provider-java info              # → {"agent_jar": "…", "version": "…"}

java -javaagent:$(platynui-provider-java agent-path) -jar theirapp.jar
```

## Building from the repository

The JAR is built by Gradle and copied in before the wheel is assembled:

```sh
just build-provider-java        # builds the agent JAR and stages it into this package
just build-provider-java-wheel  # → dist/
```
