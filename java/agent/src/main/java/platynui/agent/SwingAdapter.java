package platynui.agent;

import java.awt.AWTEvent;
import java.awt.Toolkit;
import java.awt.Window;
import java.awt.event.AWTEventListener;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The Swing/AWT adapter: what turns a JVM running Swing into a tree the host can read.
 *
 * <p>Installed by {@link AgentRuntime} once toolkit detection has seen Swing or AWT, and only then —
 * touching {@code Toolkit.getDefaultToolkit()} in a JVM that has no UI would <em>create</em> an AWT
 * event thread in an application that never had one, which is exactly the kind of side effect an
 * agent must not have.
 *
 * <p>What lives here is the wiring: the event queue as the toolkit thread, the toolkit's liveness
 * answer, the structural-change listener that drives the UI-generation counter, and the RPC methods
 * themselves. The tree reading is {@link SwingTree}, the payload is {@link SwingElement}, the
 * coordinates are {@link SwingGeometry} — each testable without the others.
 */
final class SwingAdapter {

    private final AgentRuntime runtime;

    private SwingAdapter(AgentRuntime runtime) {
        this.runtime = runtime;
    }

    /**
     * Installs the adapter if this JVM runs Swing or AWT.
     *
     * @param toolkits the detected toolkit set
     * @return the adapter, or {@code null} when no AWT-based toolkit is in play
     */
    static SwingAdapter installIfPresent(AgentRuntime runtime, Set<String> toolkits) {
        if (!toolkits.contains(ToolkitDetector.SWING) && !toolkits.contains(ToolkitDetector.AWT)) {
            return null;
        }
        SwingAdapter adapter = new SwingAdapter(runtime);
        runtime.setToolkitDispatcher(new SwingDispatcher());
        runtime.registry().setLivenessCheck(SwingTree.LIVENESS);
        adapter.watchStructuralChanges();
        AgentLog.debug("Swing/AWT adapter installed");
        return adapter;
    }

    /**
     * Bumps the UI-generation counter when the structure changes.
     *
     * <p>A global AWT listener rather than per-container listeners: the agent must not add listeners
     * to the application's own components, where they would survive the agent and change what the
     * application holds on to. The counter is only an invalidation <em>hint</em> — per-element
     * validity has its own endpoint — so a coarse signal is exactly the right amount of information.
     */
    private void watchStructuralChanges() {
        try {
            Toolkit.getDefaultToolkit().addAWTEventListener(new AWTEventListener() {
                @Override
                public void eventDispatched(AWTEvent event) {
                    runtime.generation().bump();
                }
            }, AWTEvent.WINDOW_EVENT_MASK | AWTEvent.CONTAINER_EVENT_MASK | AWTEvent.HIERARCHY_EVENT_MASK);
        } catch (RuntimeException e) {
            // `SecurityException` when a security manager refuses
            // `listenToAllAWTEvents`. The tree still works; clients just fall back
            // to polling `ui/generation`.
            AgentLog.debug("structural change listener unavailable: " + e);
        }
    }

    // ------------------------------------------------------------ RPC methods

    /** Registers the adapter's methods into the RPC table under construction. */
    void contributeMethods(Map<String, RpcMethod> methods) {
        methods.put("ui/windows", new WindowsMethod());
        methods.put("ui/children", new ChildrenMethod());
        methods.put("ui/element", new ElementMethod());
        methods.put("ui/at_point", new AtPointMethod());
        methods.put("ui/focus", new FocusMethod());
        methods.put("ui/window_handle", new WindowHandleMethod());
    }

    /** Builds the payload for one element, children count included. */
    private Map<String, Object> describe(Object element) {
        return SwingElement.describe(element, runtime.registry(), SwingTree.childCountOf(element));
    }

    private List<Object> describeAll(List<?> elements) {
        List<Object> payloads = new ArrayList<Object>(elements.size());
        for (Object element : elements) {
            payloads.add(describe(element));
        }
        return payloads;
    }

    /** Resolves an {@code id} parameter to a live element, or fails the call. */
    private Object require(Map<String, Object> params) throws RpcException {
        Object raw = params.get("id");
        if (!(raw instanceof Long)) {
            throw new RpcException(RpcException.INVALID_PARAMS, "'id' must be an element id");
        }
        Object element = runtime.registry().resolve(((Long) raw).longValue());
        if (element == null) {
            // Gone rather than never-registered, from the caller's point of view the
            // same thing: the element it holds is stale and has to be looked up again.
            throw new RpcException(RpcException.INVALID_PARAMS, "element " + raw + " is gone");
        }
        return element;
    }

    private static double requireDouble(Map<String, Object> params, String key) throws RpcException {
        Object raw = params.get(key);
        if (!(raw instanceof Number)) {
            throw new RpcException(RpcException.INVALID_PARAMS, "'" + key + "' must be a number");
        }
        return ((Number) raw).doubleValue();
    }

    /**
     * The JVM's top-level windows — the roots the provider hangs under the desktop.
     *
     * <p>Answered from {@code Window.getWindows()}, which is why the agent needs no platform window
     * enumeration and works the same on every windowing system.
     */
    private final class WindowsMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            List<Object> windows = runtime.onToolkitThread(new java.util.concurrent.Callable<List<Object>>() {
                @Override
                public List<Object> call() {
                    return describeAll(SwingTree.windows());
                }
            });
            Map<String, Object> result = Json.newObject();
            result.put("windows", windows);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /** One level of the tree, each child as a full element — the wire is coarse by design. */
    private final class ChildrenMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            final Object element = require(params);
            List<Object> children = runtime.onToolkitThread(new java.util.concurrent.Callable<List<Object>>() {
                @Override
                public List<Object> call() {
                    return describeAll(SwingTree.childrenOf(element));
                }
            });
            Map<String, Object> result = Json.newObject();
            result.put("children", children);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /** Re-reads one element, for a node whose attributes may have changed. */
    private final class ElementMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            final Object element = require(params);
            Map<String, Object> payload =
                    runtime.onToolkitThread(new java.util.concurrent.Callable<Map<String, Object>>() {
                        @Override
                        public Map<String, Object> call() {
                            return describe(element);
                        }
                    });
            Map<String, Object> result = Json.newObject();
            result.put("element", payload);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /**
     * Hit-test: the element chain at a physical desktop point, outermost first.
     *
     * <p>The chain rather than just the hit, because a picker has to reveal where the element sits,
     * and the ancestors are free here — the walk that found the deepest component passed through all
     * of them.
     */
    private final class AtPointMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            final double x = requireDouble(params, "x");
            final double y = requireDouble(params, "y");
            List<Object> chain = runtime.onToolkitThread(new java.util.concurrent.Callable<List<Object>>() {
                @Override
                public List<Object> call() {
                    return describeAll(SwingTree.chainAt(x, y));
                }
            });
            Map<String, Object> result = Json.newObject();
            result.put("chain", chain);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /**
     * Focus.
     *
     * <p>The one and only state-changing element method the agent exposes. Notably absent: a text
     * write. Per the {@code text-input-policy} capability text is typed with synthesized keyboard
     * input, so what a user cannot type into, automation cannot fill either — a programmatic setter
     * would quietly bypass validation, listeners and input masks that the application relies on.
     */
    private final class FocusMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            final Object element = require(params);
            Boolean requested = runtime.onToolkitThread(new java.util.concurrent.Callable<Boolean>() {
                @Override
                public Boolean call() {
                    return Boolean.valueOf(SwingTree.requestFocus(element));
                }
            });
            Map<String, Object> result = Json.newObject();
            result.put("requested", requested);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /**
     * The native handle of an element's window, re-read on demand.
     *
     * <p>Separate from the window element's own payload because a window gets its peer late: a client
     * that saw {@code null} at enumeration time can ask again once the window is realised, instead of
     * having to re-enumerate the tree to find out.
     */
    private final class WindowHandleMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            final Object element = require(params);
            return runtime.onToolkitThread(new java.util.concurrent.Callable<Map<String, Object>>() {
                @Override
                public Map<String, Object> call() {
                    Map<String, Object> result = Json.newObject();
                    Window window = SwingTree.windowOf(element);
                    SwingWindowHandle.describeInto(result, window);
                    result.put("pid", Long.valueOf(AgentPaths.currentPid()));
                    // The window's own rectangle travels along: it is what the
                    // provider's PID-plus-geometry fallback matches against when no
                    // in-JVM strategy yields a handle on this JDK.
                    Map<String, Object> bounds = SwingGeometry.boundsOf(window);
                    if (bounds != null) {
                        result.put("bounds", bounds);
                    }
                    return result;
                }
            });
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }
}
