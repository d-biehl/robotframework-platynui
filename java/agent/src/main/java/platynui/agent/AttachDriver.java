package platynui.agent;

import java.io.File;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

/**
 * Convenience attach driver, shipped inside the agent JAR (design decision 1).
 *
 * <pre>java -jar platynui-agent.jar &lt;pid&gt; [agent-args]</pre>
 *
 * <p>This is <em>not</em> the normal path. PlatynUI attaches through its own native implementation
 * of the JVM attach protocol (design decision 5), which needs no JDK on the test host. This driver
 * exists for hosts that happen to have a JDK — manual diagnosis, and a second opinion when the
 * native transport misbehaves.
 *
 * <p>{@code com.sun.tools.attach.VirtualMachine} lives in the {@code jdk.attach} module and is not
 * part of the Java 8 API surface this artifact compiles against, so it is reached reflectively.
 * That also keeps the driver working on both 8 (where it lived in {@code tools.jar}) and 9+.
 *
 * <p>Exit codes distinguish the two failures that need different remedies: the attach never
 * reached the target, versus the target refused the agent (a sandboxed-JNLP {@code SecurityManager},
 * for instance).
 */
public final class AttachDriver {

    /** The agent was loaded. */
    static final int EXIT_OK = 0;

    /** Wrong command line. */
    static final int EXIT_USAGE = 2;

    /** No attach mechanism on this host — a JRE without {@code jdk.attach}. */
    static final int EXIT_NO_ATTACH_MECHANISM = 3;

    /** The attach itself failed: no such process, not a JVM, denied, or attach disabled. */
    static final int EXIT_ATTACH_FAILED = 4;

    /** The attach succeeded but the target refused to initialise the agent. */
    static final int EXIT_AGENT_REFUSED = 5;

    private AttachDriver() {
        // Entry-point holder.
    }

    public static void main(String[] args) {
        System.exit(run(args));
    }

    static int run(String[] args) {
        if (args.length < 1 || args[0].isEmpty()) {
            System.err.println("usage: java -jar platynui-agent.jar <pid> [agent-args]");
            return EXIT_USAGE;
        }
        String pid = args[0];
        String agentArgs = args.length > 1 ? args[1] : null;
        String jarPath = ownJarPath();
        if (jarPath == null) {
            System.err.println("cannot locate the agent JAR — run this driver with `java -jar`");
            return EXIT_USAGE;
        }

        Class<?> virtualMachine;
        try {
            virtualMachine = Class.forName("com.sun.tools.attach.VirtualMachine");
        } catch (ClassNotFoundException e) {
            System.err.println("no attach mechanism on this host (jdk.attach missing — this is a JRE, not a JDK)");
            return EXIT_NO_ATTACH_MECHANISM;
        }

        Object vm;
        try {
            Method attach = virtualMachine.getMethod("attach", String.class);
            vm = attach.invoke(null, pid);
        } catch (ReflectiveOperationException | RuntimeException e) {
            System.err.println("attach to pid " + pid + " failed: " + rootCause(e));
            return EXIT_ATTACH_FAILED;
        }

        try {
            Method loadAgent = virtualMachine.getMethod("loadAgent", String.class, String.class);
            loadAgent.invoke(vm, jarPath, agentArgs);
        } catch (InvocationTargetException e) {
            Throwable cause = e.getCause() == null ? e : e.getCause();
            String causeName = cause.getClass().getName();
            if (causeName.equals("com.sun.tools.attach.AgentInitializationException")
                    || causeName.equals("com.sun.tools.attach.AgentLoadException")) {
                System.err.println("target refused the agent: " + cause);
                return EXIT_AGENT_REFUSED;
            }
            System.err.println("loading the agent into pid " + pid + " failed: " + cause);
            return EXIT_ATTACH_FAILED;
        } catch (ReflectiveOperationException | RuntimeException e) {
            System.err.println("loading the agent into pid " + pid + " failed: " + rootCause(e));
            return EXIT_ATTACH_FAILED;
        } finally {
            detachQuietly(virtualMachine, vm);
        }
        return EXIT_OK;
    }

    private static void detachQuietly(Class<?> virtualMachine, Object vm) {
        try {
            virtualMachine.getMethod("detach").invoke(vm);
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("detach failed: " + e);
        }
    }

    private static String ownJarPath() {
        try {
            File location = new File(
                    AttachDriver.class.getProtectionDomain().getCodeSource().getLocation().toURI());
            return location.isFile() ? location.getAbsolutePath() : null;
        } catch (Exception e) {
            return null;
        }
    }

    private static Throwable rootCause(Throwable error) {
        Throwable cause = error;
        while (cause.getCause() != null && cause.getCause() != cause) {
            cause = cause.getCause();
        }
        return cause;
    }
}
