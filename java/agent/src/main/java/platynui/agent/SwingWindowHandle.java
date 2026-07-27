package platynui.agent;

import java.awt.Component;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Map;

/**
 * The native window handle of an AWT window, read from inside the JVM (design decision 5).
 *
 * <p>Why the agent and not the host: the handle is what lets PlatynUI's window patterns
 * (activate/move/resize/state) delegate to the platform's own window manager, and the only place
 * where the mapping "this Java window ↔ that OS window" is unambiguous is inside the process that
 * owns both. The host's fallback — match by PID plus geometry against the native window list — is
 * a guess by comparison, and it needs a native window list to guess against, which not every
 * platform has.
 *
 * <p>Why reflection: the handle lives behind {@code sun.awt} peers, and none of the paths to it
 * survived Java 8 unchanged. {@code Component.getPeer()} was public-and-deprecated on 8 and removed
 * on 9; the peer field went private; the peer classes moved behind module boundaries. So this walks
 * a chain of strategies, cheapest first, and <strong>reports which one answered</strong> — the
 * source string is the diagnostic that turns "no handle on this JDK" from a mystery into a known
 * cell of the matrix. Nothing here throws: an unavailable handle is a normal answer that hands the
 * question to the provider's fallback.
 *
 * <p>On 9+ every path needs {@code --add-opens}; which ones is recorded in the change's design.md,
 * measured rather than assumed. Absent them the result is simply {@code null}, never a failure in
 * the target application.
 */
final class SwingWindowHandle {

    /** Where a handle came from, or why there is none. Part of the wire payload. */
    static final String SOURCE_NONE = "none";

    private SwingWindowHandle() {
        // Static helper.
    }

    /** One resolved handle plus the strategy that produced it. */
    static final class Resolved {

        final long handle;
        final String source;

        Resolved(long handle, String source) {
            this.handle = handle;
            this.source = source;
        }
    }

    /**
     * The native handle of the window {@code component} lives in.
     *
     * @return the handle and its source; {@code handle == 0} with {@link #SOURCE_NONE} when no
     *     strategy worked on this JDK
     */
    static Resolved of(Component component) {
        if (component == null) {
            return new Resolved(0L, SOURCE_NONE);
        }
        Object peer = peerOf(component);
        if (peer == null) {
            return new Resolved(0L, SOURCE_NONE);
        }
        // Windows: `sun.awt.windows.WComponentPeer#getHWnd`.
        Resolved windows = invokeLongGetter(peer, "getHWnd", "sun.awt.windows.WComponentPeer#getHWnd");
        if (windows != null) {
            return windows;
        }
        // X11: `sun.awt.X11.XBaseWindow#getWindow`. Read here rather than in a
        // Linux-only branch because the agent is one artifact for every platform,
        // and `java-provider-linux` measures this exact call.
        Resolved x11 = invokeLongGetter(peer, "getWindow", "sun.awt.X11.XBaseWindow#getWindow");
        if (x11 != null) {
            return x11;
        }
        return new Resolved(0L, SOURCE_NONE);
    }

    /** The handle as a JSON-ready pair, for embedding in a window element. */
    static void describeInto(Map<String, Object> target, Component component) {
        Resolved resolved = of(component);
        target.put("handle", resolved.handle == 0L ? null : Long.valueOf(resolved.handle));
        target.put("handleSource", resolved.source);
    }

    /**
     * The component's AWT peer.
     *
     * <p>Two strategies, in the order that costs least: the public-on-8 accessor first, the private
     * field second. The field path is what carries 9+, and it is the one that needs
     * {@code --add-opens java.desktop/java.awt=ALL-UNNAMED}.
     */
    private static Object peerOf(Component component) {
        try {
            // Java 8: `public ComponentPeer getPeer()` — deprecated, but no
            // `--add-opens` and no private access.
            Method getPeer = Component.class.getMethod("getPeer");
            Object peer = getPeer.invoke(component);
            if (peer != null) {
                return peer;
            }
        } catch (ReflectiveOperationException | RuntimeException e) {
            AgentLog.debug("Component.getPeer() unavailable: " + e);
        }
        try {
            Field field = Component.class.getDeclaredField("peer");
            field.setAccessible(true);
            return field.get(component);
        } catch (ReflectiveOperationException | RuntimeException e) {
            // `InaccessibleObjectException` on 9+ without the add-opens — a
            // RuntimeException, so it lands here and stays a null answer.
            AgentLog.debug("Component.peer field unavailable: " + e);
            return null;
        }
    }

    /**
     * Calls a no-argument getter that returns a native handle.
     *
     * <p>Walks the peer's class hierarchy: the method is declared on a base peer class
     * ({@code WComponentPeer}, {@code XBaseWindow}) while the instance is a concrete subclass, and
     * {@code getMethod} would not see it once the declaring class stopped being exported.
     *
     * @return the resolved handle, or {@code null} when this peer has no such getter
     */
    private static Resolved invokeLongGetter(Object peer, String getter, String source) {
        for (Class<?> type = peer.getClass(); type != null; type = type.getSuperclass()) {
            Method method;
            try {
                method = type.getDeclaredMethod(getter);
            } catch (NoSuchMethodException e) {
                continue;
            }
            try {
                method.setAccessible(true);
                Object value = method.invoke(peer);
                if (value instanceof Number) {
                    long handle = ((Number) value).longValue();
                    if (handle != 0L) {
                        return new Resolved(handle, source);
                    }
                }
                return null;
            } catch (ReflectiveOperationException | RuntimeException e) {
                AgentLog.debug(source + " not accessible: " + e);
                return null;
            }
        }
        return null;
    }
}
