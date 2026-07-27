package platynui.agent;

import java.io.File;
import java.lang.reflect.Method;
import java.util.Map;

/**
 * What the JVM knows about its own process, for the provider's {@code app:Application} node.
 *
 * <p>Answered from inside rather than queried from outside, and that is the point: the host would
 * need a per-platform process API to learn the same things (on Windows a WMI or ToolHelp query), and
 * would still not know the <em>main class</em> — which is the only one of these facts a user
 * recognises their application by. A JVM's executable is always {@code java}; its identity is the
 * command it was given.
 *
 * <p>Everything is a system property, so this works on every platform and on Java 8. The one
 * exception is the start time, which needs {@code ProcessHandle} (Java 9+) and is simply absent
 * below that.
 */
final class ProcessFacts {

    private ProcessFacts() {
        // Static helper.
    }

    /** The process facts as a JSON-ready object. Absent fields mean "not knowable here". */
    static Map<String, Object> collect() {
        Map<String, Object> facts = Json.newObject();
        facts.put("pid", Long.valueOf(AgentPaths.currentPid()));
        put(facts, "name", applicationName());
        put(facts, "executablePath", executablePath());
        put(facts, "commandLine", System.getProperty("sun.java.command"));
        put(facts, "userName", System.getProperty("user.name"));
        put(facts, "architecture", System.getProperty("os.arch"));
        put(facts, "vmName", System.getProperty("java.vm.name"));
        put(facts, "javaVersion", System.getProperty("java.version"));
        Long startTime = startTimeMillis();
        if (startTime != null) {
            facts.put("startTimeMillis", startTime);
        }
        return facts;
    }

    /**
     * A name a user would recognise: the main class's simple name.
     *
     * <p>{@code sun.java.command} holds the main class followed by the application's arguments, or
     * the jar path when launched with {@code -jar}. Taking the first token and its last dotted
     * segment turns {@code "platynui.testapp.Main --title X"} into {@code "Main"}, which beats both
     * the whole command line and {@code "java"}.
     */
    private static String applicationName() {
        String command = System.getProperty("sun.java.command");
        if (command == null || command.isEmpty()) {
            return null;
        }
        int space = command.indexOf(' ');
        String first = space < 0 ? command : command.substring(0, space);
        if (first.endsWith(".jar")) {
            return new File(first).getName();
        }
        int dot = first.lastIndexOf('.');
        return dot < 0 || dot == first.length() - 1 ? first : first.substring(dot + 1);
    }

    private static String executablePath() {
        String home = System.getProperty("java.home");
        if (home == null || home.isEmpty()) {
            return null;
        }
        boolean windows = String.valueOf(System.getProperty("os.name")).toLowerCase(java.util.Locale.ENGLISH)
                .contains("windows");
        File executable = new File(new File(home, "bin"), windows ? "java.exe" : "java");
        return executable.getPath();
    }

    /** Process start time in epoch millis, via {@code ProcessHandle} on 9+; {@code null} on 8. */
    private static Long startTimeMillis() {
        try {
            Class<?> processHandle = Class.forName("java.lang.ProcessHandle");
            Object current = processHandle.getMethod("current").invoke(null);
            Object info = processHandle.getMethod("info").invoke(current);
            Object startInstant = info.getClass().getMethod("startInstant").invoke(info);
            Method isPresent = startInstant.getClass().getMethod("isPresent");
            if (!Boolean.TRUE.equals(isPresent.invoke(startInstant))) {
                return null;
            }
            Object instant = startInstant.getClass().getMethod("get").invoke(startInstant);
            Object millis = instant.getClass().getMethod("toEpochMilli").invoke(instant);
            return millis instanceof Number ? Long.valueOf(((Number) millis).longValue()) : null;
        } catch (ReflectiveOperationException | RuntimeException e) {
            // Java 8, or a JVM that will not answer. Either way the field is absent
            // rather than wrong.
            return null;
        }
    }

    private static void put(Map<String, Object> target, String key, String value) {
        if (value != null && !value.isEmpty()) {
            target.put(key, value);
        }
    }
}
