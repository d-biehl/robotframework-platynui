package platynui.agent;

/**
 * The agent's diagnostic channel.
 *
 * <p>The agent runs inside somebody else's process, so its default noise budget is zero: only
 * failures the operator must see are printed unconditionally. Everything else needs
 * {@code -Dplatynui.agent.debug=true}. No logging framework is used — pulling one onto a foreign
 * application's classpath is exactly the kind of side effect an agent must not have.
 */
final class AgentLog {

    private static final String PREFIX = "[PlatynUI agent] ";

    private static final boolean DEBUG = debugRequested();

    /**
     * Reading a system property is itself permission-gated, and a target running under a
     * {@code SecurityManager} may deny it. The diagnostic channel must survive that: throwing here
     * would take out the class initializer, and with it every error message the agent still had to
     * report.
     */
    private static boolean debugRequested() {
        try {
            return Boolean.getBoolean("platynui.agent.debug");
        } catch (SecurityException e) {
            return false;
        }
    }

    private AgentLog() {
        // Static helper.
    }

    static boolean isDebugEnabled() {
        return DEBUG;
    }

    static void debug(String message) {
        if (DEBUG) {
            System.err.println(PREFIX + message);
        }
    }

    static void error(String message, Throwable cause) {
        System.err.println(PREFIX + message);
        if (cause != null) {
            if (DEBUG) {
                cause.printStackTrace(System.err);
            } else {
                System.err.println(PREFIX + "  cause: " + cause);
            }
        }
    }
}
