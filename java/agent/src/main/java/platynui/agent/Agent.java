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
 * <p>Both paths land in {@link #start}, which is idempotent: a JVM that was started with
 * {@code -javaagent} and later attached to keeps the one agent it already has.
 */
public final class Agent {

    private static final String VERSION_RESOURCE = "platynui/agent/version.properties";

    /** Guards {@link #start} against a second injection; see the class comment. */
    private static boolean started;

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
     * @param args the agent argument string, or {@code null}
     * @param instrumentation the JVM's instrumentation handle — used only to read the loaded-class
     *     list for toolkit detection; the agent registers no transformer and rewrites no class
     */
    public static synchronized void start(String args, Instrumentation instrumentation) {
        if (started) {
            return;
        }
        started = true;
        AgentRuntime.start(args, instrumentation);
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
        try (InputStream stream = Agent.class.getClassLoader().getResourceAsStream(VERSION_RESOURCE)) {
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
