package platynui.agent;

import java.io.IOException;
import java.io.InputStream;
import java.lang.instrument.Instrumentation;
import java.util.Properties;

/**
 * Entry point of the PlatynUI agent.
 *
 * <p>One artifact serves both injection paths (design decision 1): {@code premain} when the
 * operator launched the application with {@code -javaagent}, {@code agentmain} when PlatynUI
 * attached to an already-running JVM. Attach is the primary path — Java applications are launched
 * by scripts, installers or Web Start, and the Inspector's core use is looking into an application
 * that is already running.
 *
 * <p>Both paths land in {@link #start}, which is idempotent on a <em>running</em> agent: a JVM that
 * was started with {@code -javaagent} and later attached to keeps the one agent it already has,
 * while a start that failed leaves the JVM open to a later attempt. That distinction lives in
 * {@link AgentRuntime} alone — "an agent is running here" is a fact about the runtime, and a second
 * flag here could only ever disagree with it.
 */
public final class Agent {

    private static final String VERSION_RESOURCE = "platynui/agent/version.properties";

    private Agent() {
        // Entry-point holder.
    }

    /** JVM entry point for {@code -javaagent} at launch. */
    public static void premain(String args, Instrumentation instrumentation) {
        start(args, instrumentation);
    }

    /** JVM entry point for an attach into a running JVM. */
    public static void agentmain(String args, Instrumentation instrumentation) {
        start(args, instrumentation);
    }

    /**
     * Starts the agent, at most once per JVM.
     *
     * <p>Nothing below this point may reach the application. The runtime reports and absorbs its own
     * failures; this catch is the backstop for the one that broke in the field — a failure raised
     * <em>while</em> reporting an earlier one, which arrives as an {@code Error} and escapes
     * everything written to expect exceptions.
     *
     * @param args the agent argument string, or {@code null}
     * @param instrumentation the JVM's instrumentation handle — used only to read the loaded-class
     *     list for toolkit detection; the agent registers no transformer and rewrites no class
     */
    public static synchronized void start(String args, Instrumentation instrumentation) {
        try {
            AgentRuntime.start(args, instrumentation);
        } catch (Throwable e) {
            reportQuietly(e);
        }
    }

    /**
     * Reports a failure the runtime could not report itself, and gives up silently if even that
     * fails. An agent has no diagnostic channel of its own; borrowing the application's failure path
     * is the one thing it must never do.
     */
    private static void reportQuietly(Throwable failure) {
        try {
            AgentLog.error("agent failed to start", failure);
        } catch (Throwable ignored) {
            // Deliberately empty: there is nothing left to report with.
        }
    }

    /**
     * The agent's own version, as built into the artifact.
     *
     * <p>Read from a generated resource rather than from the package's manifest attributes,
     * because which class loader ends up defining the agent package depends on the injection path.
     * The provider compares this string for exact equality (design decision 1a).
     *
     * @return the version string, or {@code "unknown"} if the resource is missing
     */
    public static String version() {
        Properties properties = new Properties();
        // Through the class, not through its loader: `getClassLoader()` is null for a
        // bootstrap-defined class, which is exactly where the agent lands when the JAR
        // carries `Boot-Class-Path`.
        //
        // For such a class this call resolves through the SYSTEM class loader — the JDK
        // offers no public way to read a resource off the bootstrap search path. It
        // answers because the agent mechanism appends the JAR to the system class path as
        // well, which every injection path does. A JAR placed on the bootstrap search
        // ALONE (`-Xbootclasspath/a:`, which is how one would be tempted to simulate this
        // in a test) has no readable resource here and reports "unknown" — a property of
        // the JDK's resource lookup, not a defect of this method.
        try (InputStream stream = Agent.class.getResourceAsStream("/" + VERSION_RESOURCE)) {
            if (stream == null) {
                return "unknown";
            }
            properties.load(stream);
        } catch (IOException e) {
            return "unknown";
        }
        return properties.getProperty("version", "unknown");
    }
}
