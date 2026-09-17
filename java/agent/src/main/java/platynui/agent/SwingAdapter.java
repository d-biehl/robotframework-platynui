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
        runtime.registry().setWorldCheck(WORLD_OF_ELEMENT);
        // So the runtime's own element-scoped endpoints — `element/live` — reach the same thread
        // the adapter's do, instead of whichever queue the RPC handler could see.
        runtime.setToolkitDispatcherResolver(new ToolkitDispatcher.Resolver() {
            @Override
            public ToolkitDispatcher forWorld(Object world) {
                return world == null ? null : new SwingDispatcher(world);
            }
        });
        adapter.watchStructuralChanges();
        AgentLog.debug("Swing/AWT adapter installed; toolkit worlds: "
                + (AppContexts.available() ? String.valueOf(AppContexts.all().size()) : "not enumerable"));
        return adapter;
    }

    /**
     * Which toolkit world an element belongs to, asked of the element rather than of the thread.
     *
     * <p>A {@code VirtualChild} (a table row or cell) has no context of its own; it belongs to
     * wherever its owning component does.
     */
    private static final ElementRegistry.WorldCheck WORLD_OF_ELEMENT = new ElementRegistry.WorldCheck() {
        @Override
        public Object worldOf(Object element) {
            return AppContexts.of(SwingTree.componentOf(element));
        }
    };

    /**
     * Bumps the UI-generation counter when the structure changes.
     *
     * <p>A global AWT listener rather than per-container listeners: the agent must not add listeners
     * to the application's own components, where they would survive the agent and change what the
     * application holds on to. The counter is only an invalidation <em>hint</em> — per-element
     * validity has its own endpoint — so a coarse signal is exactly the right amount of information.
     *
     * <p>"Global" here really is per JVM, across toolkit worlds — measured, because the opposite was
     * the obvious guess and it is wrong: the listener list hangs off the single {@code Toolkit}
     * instance, not off an {@code AppContext}, so one registration hears every world's events
     * ({@code AppContextEventsTest}). Registering once per world would install N listeners in a
     * foreign process to learn the same thing N times.
     *
     * <p>Where it registers from still matters. An agent thread in a multi-world JVM has no
     * {@code AppContext} of its own, and AWT calls that resolve one then fail — so the registration
     * is posted to a world's event thread when there are worlds to ask, and only done inline when
     * this JVM has no enumerable ones (the single-world case, where the calling thread's context is
     * always found).
     */
    private void watchStructuralChanges() {
        List<Object> worlds = AppContexts.all();
        if (worlds.isEmpty()) {
            listenHere();
            return;
        }
        new SwingDispatcher(worlds.get(0)).submit(new Runnable() {
            @Override
            public void run() {
                listenHere();
            }
        });
    }

    /** Registers the structural listener from the calling thread's toolkit world. */
    private void listenHere() {
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

    /**
     * An element and the toolkit thread that may touch it.
     *
     * <p>The two travel together on purpose: every read of a component has to happen on the event
     * queue of the world that component belongs to, and separating the pair is how a call ends up
     * dispatched to whichever queue the agent thread could reach.
     */
    private static final class Target {

        private final Object element;
        private final ToolkitDispatcher dispatcher;

        Target(Object element, ToolkitDispatcher dispatcher) {
            this.element = element;
            this.dispatcher = dispatcher;
        }
    }

    /** Resolves an {@code id} parameter to a live element and its toolkit thread, or fails. */
    private Target require(Map<String, Object> params) throws RpcException {
        Object raw = params.get("id");
        if (!(raw instanceof Long)) {
            throw new RpcException(RpcException.INVALID_PARAMS, "'id' must be an element id");
        }
        long id = ((Long) raw).longValue();
        Object element = runtime.registry().resolve(id);
        if (element == null) {
            // Gone rather than never-registered, from the caller's point of view the
            // same thing: the element it holds is stale and has to be looked up again.
            throw new RpcException(RpcException.INVALID_PARAMS, "element " + raw + " is gone");
        }
        // The world recorded when the id was handed out — never re-derived from this thread, whose
        // own world is either the wrong one or none at all.
        return new Target(element, dispatcherFor(runtime.registry().worldOf(id)));
    }

    /**
     * The dispatcher for one toolkit world, or the installed default when the world is unknown.
     *
     * <p>Not cached: a dispatcher is two fields, and a map keyed by {@code AppContext} would hold
     * disposed worlds alive for the life of the agent — a weak map cannot help, because the value
     * references the key.
     */
    private ToolkitDispatcher dispatcherFor(Object world) {
        return runtime.dispatcherForWorld(world);
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
            Map<String, Object> result = Json.newObject();
            result.put("windows", allWindows());
            return result;
        }

        /**
         * The union across every toolkit world, each world asked on its own event thread.
         *
         * <p>A world that misses its deadline contributes nothing and does not fail the call. That
         * is a deliberate asymmetry with the per-element endpoints, which do fail: a frozen Web
         * Start application must not make a healthy one in the same JVM disappear, and "this world
         * did not answer" is indistinguishable from "this world has no windows" to a caller either
         * way. When *no* world answers, the failure is propagated — that is the frozen-JVM case the
         * transport's containment promise is about.
         */
        private List<Object> allWindows() throws RpcException {
            List<Object> worlds = AppContexts.all();
            if (worlds.isEmpty()) {
                return runtime.onToolkitThread(new java.util.concurrent.Callable<List<Object>>() {
                    @Override
                    public List<Object> call() {
                        return describeAll(SwingTree.windows());
                    }
                });
            }
            List<Object> windows = new ArrayList<Object>();
            RpcException lastFailure = null;
            int answered = 0;
            for (final Object world : worlds) {
                try {
                    windows.addAll(runtime.onToolkitThread(
                            dispatcherFor(world), new java.util.concurrent.Callable<List<Object>>() {
                                @Override
                                public List<Object> call() {
                                    return describeAll(SwingTree.windowsOf(world));
                                }
                            }));
                    answered++;
                } catch (RpcException e) {
                    AgentLog.debug("toolkit world " + world + " did not answer ui/windows: " + e.getMessage());
                    lastFailure = e;
                }
            }
            if (answered == 0 && lastFailure != null) {
                throw lastFailure;
            }
            return windows;
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
            final Target target = require(params);
            List<Object> children =
                    runtime.onToolkitThread(target.dispatcher, new java.util.concurrent.Callable<List<Object>>() {
                        @Override
                        public List<Object> call() {
                            return describeAll(SwingTree.childrenOf(target.element));
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
            final Target target = require(params);
            Map<String, Object> payload = runtime.onToolkitThread(
                    target.dispatcher, new java.util.concurrent.Callable<Map<String, Object>>() {
                        @Override
                        public Map<String, Object> call() {
                            return describe(target.element);
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
            Map<String, Object> result = Json.newObject();
            result.put("chain", chainAt(x, y));
            return result;
        }

        /**
         * Asks each toolkit world in turn, on its own event thread, and takes the first hit.
         *
         * <p>A point over a Web Start application is in a world the agent's threads are not in, so
         * a single lookup would answer "nothing here" rather than fail — the quietest possible
         * wrong answer for a picker. A world that misses its deadline is skipped for the same
         * reason {@code ui/windows} skips it: a frozen application must not make the window under
         * the pointer unpickable in a healthy one.
         */
        private List<Object> chainAt(final double x, final double y) throws RpcException {
            List<Object> worlds = AppContexts.all();
            if (worlds.isEmpty()) {
                return runtime.onToolkitThread(new java.util.concurrent.Callable<List<Object>>() {
                    @Override
                    public List<Object> call() {
                        return describeAll(SwingTree.chainAt(x, y));
                    }
                });
            }
            RpcException lastFailure = null;
            int answered = 0;
            for (final Object world : worlds) {
                try {
                    List<Object> chain = runtime.onToolkitThread(
                            dispatcherFor(world), new java.util.concurrent.Callable<List<Object>>() {
                                @Override
                                public List<Object> call() {
                                    return describeAll(SwingTree.chainAt(SwingTree.windowsOf(world), x, y));
                                }
                            });
                    answered++;
                    if (!chain.isEmpty()) {
                        return chain;
                    }
                } catch (RpcException e) {
                    AgentLog.debug("toolkit world " + world + " did not answer ui/at_point: " + e.getMessage());
                    lastFailure = e;
                }
            }
            if (answered == 0 && lastFailure != null) {
                throw lastFailure;
            }
            return new ArrayList<Object>();
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
            final Target target = require(params);
            Boolean requested =
                    runtime.onToolkitThread(target.dispatcher, new java.util.concurrent.Callable<Boolean>() {
                        @Override
                        public Boolean call() {
                            return Boolean.valueOf(SwingTree.requestFocus(target.element));
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
            final Target target = require(params);
            return runtime.onToolkitThread(
                    target.dispatcher, new java.util.concurrent.Callable<Map<String, Object>>() {
                        @Override
                        public Map<String, Object> call() {
                            Map<String, Object> result = Json.newObject();
                            Window window = SwingTree.windowOf(target.element);
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
