package platynui.agent;

import java.awt.Component;
import java.awt.EventQueue;
import java.awt.Window;
import java.lang.ref.WeakReference;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.Vector;

/**
 * The JVM's AWT toolkit worlds.
 *
 * <p>AWT partitions a JVM's UI into {@code sun.awt.AppContext}s, each with its own window list and
 * its own event queue. A plain application has one, so the distinction is invisible — and the two
 * APIs that matter, {@code Window.getWindows()} and {@code EventQueue.invokeLater}, quietly resolve
 * against the <em>calling thread's</em> context, which makes the single-context case look like the
 * only case.
 *
 * <p>It is not. Java Web Start and applet runtimes create one context per hosted application, so an
 * agent whose threads live elsewhere sees no windows and posts work to a queue that will never run
 * it. Measured in a real OpenWebStart target: two contexts, the application's {@code JFrame} in the
 * one the agent's threads are not in. Measured in the fixture's Web Start mode, which is harsher:
 * the agent's thread has <em>no</em> context at all and {@code Window.getWindows()} throws, because
 * once a JVM has more than one context {@code AppContext.getAppContext()} loses its single-context
 * shortcut and walks the calling thread's group chain instead.
 *
 * <p>Everything here is reflective: the agent compiles against Java 8's public API and must not link
 * a JDK internal. Only two accessors are used — {@code getAppContexts} and {@code get} — both stable
 * since 1.4 and both what AWT's own code uses. Every failure degrades to {@link #available()}
 * reporting {@code false}, which leaves callers on today's single-context behaviour with a
 * diagnostic, rather than taking the adapter down.
 */
final class AppContexts {

    private static final Class<?> APP_CONTEXT = resolveClass();
    private static final Method GET_APP_CONTEXTS = resolveMethod("getAppContexts");
    private static final Method GET_APP_CONTEXT = resolveMethod("getAppContext");
    private static final Method GET = resolveGet();
    private static final Object EVENT_QUEUE_KEY = resolveEventQueueKey();
    /** {@code java.awt.Component.appContext}, when it is reachable; see {@link #of}. */
    private static final Field COMPONENT_APP_CONTEXT = resolveComponentField();

    private AppContexts() {
        // Static helper.
    }

    /** Whether this JVM's toolkit worlds can be enumerated at all. */
    static boolean available() {
        return GET_APP_CONTEXTS != null && GET != null && EVENT_QUEUE_KEY != null;
    }

    /**
     * Every toolkit world in this JVM.
     *
     * <p>Empty when the internals are unavailable, which callers read as "fall back to whatever the
     * calling thread can see" — the behaviour before any of this existed.
     */
    static List<Object> all() {
        if (!available()) {
            return Collections.emptyList();
        }
        try {
            Object contexts = GET_APP_CONTEXTS.invoke(null);
            if (!(contexts instanceof Set)) {
                return Collections.emptyList();
            }
            return new ArrayList<Object>((Set<?>) contexts);
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("could not enumerate AppContexts: " + e);
            return Collections.emptyList();
        }
    }

    /**
     * The calling thread's own world, or {@code null} — which an agent thread genuinely is in a
     * multi-context JVM. Diagnostics only: it is never an answer about an element.
     */
    static Object current() {
        if (GET_APP_CONTEXT == null) {
            return null;
        }
        try {
            return GET_APP_CONTEXT.invoke(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return null;
        }
    }

    /** The event queue of one world, or {@code null} if it has none (yet). */
    static EventQueue eventQueueOf(Object context) {
        Object queue = valueOf(context, EVENT_QUEUE_KEY);
        return queue instanceof EventQueue ? (EventQueue) queue : null;
    }

    /**
     * The windows of one world.
     *
     * <p>Read from the same place {@code Window.getWindows()} reads — the context's own
     * {@code Vector<WeakReference<Window>>} under the {@code Window.class} key — so this is the same
     * source of truth, consulted once per world instead of once for whichever world the caller
     * happens to be in.
     */
    static List<Window> windowsOf(Object context) {
        Object value = valueOf(context, Window.class);
        if (!(value instanceof Vector)) {
            return Collections.emptyList();
        }
        List<Window> windows = new ArrayList<Window>();
        // Copied under the vector's own lock: the application may add or remove a window while this
        // runs, and AWT mutates this very list to do it.
        Vector<?> vector = (Vector<?>) value;
        synchronized (vector) {
            for (Object entry : vector) {
                Object window = entry instanceof WeakReference ? ((WeakReference<?>) entry).get() : entry;
                if (window instanceof Window) {
                    windows.add((Window) window);
                }
            }
        }
        return windows;
    }

    /**
     * The world an element belongs to, derived from the element itself.
     *
     * <p>Never from the calling thread: that is the bug this class exists to fix, and it would come
     * back silently — an id registered by one thread and re-read from another would answer
     * differently depending on who asked.
     *
     * <p>{@code Component.appContext} is read directly when the package is open (it is, through the
     * same {@code ModuleAccess} opening the native window handle needs). The fallback searches the
     * worlds' window lists for the component's own top-level, which needs no field access at all.
     *
     * @return the owning context, or {@code null} when it cannot be determined
     */
    static Object of(Component component) {
        if (component == null || !available()) {
            return null;
        }
        if (COMPONENT_APP_CONTEXT != null) {
            try {
                Object context = COMPONENT_APP_CONTEXT.get(component);
                if (context != null) {
                    return context;
                }
            } catch (ReflectiveOperationException | RuntimeException e) {
                // Fall through to the search.
            }
        }
        Window window = component instanceof Window ? (Window) component : SwingTree.windowOf(component);
        if (window == null) {
            return null;
        }
        for (Object context : all()) {
            for (Window candidate : windowsOf(context)) {
                if (candidate == window) {
                    return context;
                }
            }
        }
        return null;
    }

    private static Object valueOf(Object context, Object key) {
        if (context == null || key == null || GET == null) {
            return null;
        }
        try {
            return GET.invoke(context, key);
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("could not read " + key + " from an AppContext: " + e);
            return null;
        }
    }

    private static Class<?> resolveClass() {
        try {
            return Class.forName("sun.awt.AppContext");
        } catch (ClassNotFoundException | RuntimeException e) {
            AgentLog.debug("sun.awt.AppContext unavailable: " + e);
            return null;
        }
    }

    private static Method resolveMethod(String name) {
        if (APP_CONTEXT == null) {
            return null;
        }
        try {
            return APP_CONTEXT.getMethod(name);
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("sun.awt.AppContext." + name + "() unavailable: " + e);
            return null;
        }
    }

    private static Method resolveGet() {
        if (APP_CONTEXT == null) {
            return null;
        }
        try {
            return APP_CONTEXT.getMethod("get", Object.class);
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("sun.awt.AppContext.get(Object) unavailable: " + e);
            return null;
        }
    }

    private static Object resolveEventQueueKey() {
        if (APP_CONTEXT == null) {
            return null;
        }
        try {
            return APP_CONTEXT.getField("EVENT_QUEUE_KEY").get(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("sun.awt.AppContext.EVENT_QUEUE_KEY unavailable: " + e);
            return null;
        }
    }

    private static Field resolveComponentField() {
        try {
            Field field = Component.class.getDeclaredField("appContext");
            field.setAccessible(true);
            return field;
        } catch (ReflectiveOperationException | RuntimeException e) {
            // Not fatal: `of` falls back to searching the window lists.
            AgentLog.debug("java.awt.Component.appContext unavailable, using the window-list search: " + e);
            return null;
        }
    }
}
