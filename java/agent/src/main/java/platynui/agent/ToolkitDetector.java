package platynui.agent;

import java.lang.instrument.Instrumentation;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * Which UI toolkits are actually live in this JVM.
 *
 * <p>From inside the process the answer is unambiguous, which is why the agent — not the host — is
 * the authoritative classifier (design decision 1). It reports the <strong>set</strong> of active
 * toolkits: a JVM running more than one is the deferred mixed-toolkit case, and the handshake
 * carries a list from day one so that case needs no format break.
 *
 * <p>Detection reads {@link Instrumentation#getAllLoadedClasses()} rather than probing the toolkits
 * themselves. That matters: {@code Toolkit.getDefaultToolkit()} would <em>initialise</em> AWT and
 * start an event thread in an application that never had one. An agent must not change the
 * behaviour of the application it observes.
 */
final class ToolkitDetector {

    static final String SWING = "swing";
    static final String AWT = "awt";
    static final String JAVAFX = "javafx";
    static final String SWT = "swt";

    /** Marker class per toolkit — loaded if and only if that toolkit is in play. */
    private static final String SWING_MARKER = "javax.swing.JComponent";
    private static final String AWT_MARKER = "java.awt.Window";
    private static final String JAVAFX_MARKER = "javafx.stage.Stage";
    private static final String SWT_MARKER = "org.eclipse.swt.widgets.Display";

    private ToolkitDetector() {
        // Static helper.
    }

    /**
     * The toolkits live in this JVM right now, in a stable order.
     *
     * <p>Empty is a normal answer, not a failure: injected via {@code -javaagent}, the agent runs
     * before the application has loaded a single toolkit class.
     *
     * @param instrumentation the JVM's instrumentation handle, or {@code null} if unavailable
     * @return an unmodifiable set of toolkit names
     */
    static Set<String> detect(Instrumentation instrumentation) {
        Set<String> toolkits = new LinkedHashSet<String>();
        if (instrumentation == null) {
            return Collections.unmodifiableSet(toolkits);
        }
        boolean swing = false;
        boolean awt = false;
        for (Class<?> loaded : instrumentation.getAllLoadedClasses()) {
            String name = loaded.getName();
            if (SWING_MARKER.equals(name)) {
                swing = true;
            } else if (AWT_MARKER.equals(name)) {
                awt = true;
            } else if (JAVAFX_MARKER.equals(name)) {
                toolkits.add(JAVAFX);
            } else if (SWT_MARKER.equals(name)) {
                toolkits.add(SWT);
            }
        }
        if (swing) {
            // Swing is built on AWT, so the AWT marker is always loaded too. Reporting both
            // would suggest two adapters are needed where one serves the whole hierarchy.
            toolkits.add(SWING);
        } else if (awt) {
            toolkits.add(AWT);
        }
        return Collections.unmodifiableSet(toolkits);
    }

    /** The detected set as a JSON-ready list, sorted so the handshake file is stable. */
    static List<String> asSortedList(Set<String> toolkits) {
        List<String> sorted = new java.util.ArrayList<String>(toolkits);
        Collections.sort(sorted);
        return sorted;
    }
}
